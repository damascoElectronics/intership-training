//! Servicio HK — daemon de Housekeeping PUS Servicio 3.
//!
//! Ante una solicitud TC(3,129): lee /proc/uptime y /proc/self/status,
//! empaqueta TM(3,25) Informe de Parámetros de Housekeeping, y lo envía de vuelta al
//! router para el enlace descendente.

use obc_core::SpacePacket;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info};

const HK_SOCKET: &str = "/tmp/obc_hk.sock";
const ROUTER_SOCKET: &str = "/tmp/obc_router.sock";

/// Lee el tiempo de actividad del sistema desde /proc/uptime (primer número, en segundos).
async fn read_uptime_s() -> f64 {
    match tokio::fs::read_to_string("/proc/uptime").await {
        Ok(s) => s.split_whitespace().next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0),
        Err(_) => 0.0,
    }
}

/// Lee VmRSS (tamaño del conjunto residente en kB) desde /proc/self/status.
async fn read_rss_kb() -> u64 {
    match tokio::fs::read_to_string("/proc/self/status").await {
        Ok(s) => {
            for line in s.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(v) = line.split_whitespace().nth(1) {
                        return v.parse().unwrap_or(0);
                    }
                }
            }
            0
        }
        Err(_) => 0,
    }
}

async fn build_hk_report(request: &SpacePacket) -> Vec<u8> {
    let uptime_s = read_uptime_s().await;
    let rss_kb = read_rss_kb().await;

    let report = serde_json::json!({
        "uptime_s": uptime_s,
        "rss_kb": rss_kb,
        "obc_mode": "NOMINAL",
    });

    let app_data = report.to_string().into_bytes();
    let tm = SpacePacket::new_tm(
        0x003,              // APID: TM del servicio HK
        request.seq_count,  // reflejar el contador de secuencia del TC
        3,                  // PUS servicio 3
        25,                 // subservicio 25: informe de parámetros HK
        app_data,
    );

    bincode::serde::encode_to_vec(&tm, bincode::config::standard()).unwrap_or_default()
}

async fn send_to_router(payload: &[u8]) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(ROUTER_SOCKET).await?;
    stream.write_all(&(payload.len() as u32).to_be_bytes()).await?;
    stream.write_all(payload).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_target(false).with_env_filter("info").init();

    let _ = std::fs::remove_file(HK_SOCKET);
    let listener = UnixListener::bind(HK_SOCKET)?;
    info!("Servicio HK escuchando en {HK_SOCKET}");

    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                tokio::spawn(async move {
                    loop {
                        let mut len_buf = [0u8; 4];
                        if stream.read_exact(&mut len_buf).await.is_err() { break; }
                        let len = u32::from_be_bytes(len_buf) as usize;
                        if len > 65536 { break; }
                        let mut buf = vec![0u8; len];
                        if stream.read_exact(&mut buf).await.is_err() { break; }

                        let pkt: SpacePacket = match bincode::serde::decode_from_slice(
                            &buf, bincode::config::standard()) {
                            Ok((p, _)) => p,
                            Err(e) => { error!("error de análisis: {e}"); break; }
                        };

                        if pkt.service == 3 && pkt.subservice == 129 {
                            info!("recibido TC(3,129): construyendo informe HK");
                            let report = build_hk_report(&pkt).await;
                            if let Err(e) = send_to_router(&report).await {
                                error!("fallo al enviar TM al router: {e}");
                            } else {
                                info!("enviado TM(3,25) informe HK ({} bytes)", report.len());
                            }
                        }
                    }
                });
            }
            Err(e) => error!("aceptación: {e}"),
        }
    }
}
