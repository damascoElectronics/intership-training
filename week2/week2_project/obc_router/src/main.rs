//! Router OBC — despachador central de TC/TM.
//!
//! Recibe TCs verificados desde tc_receiver, los enruta a sockets de subsistemas
//! por APID. Recibe TM de los subsistemas y los reenvía al socket de enlace descendente.
//!
//! # Tabla de enrutamiento por APID
//! ```text
//! TC APID 0x017 (Servicio 17) → manejado internamente (ping/pong)
//! TC APID 0x003 (Servicio 3)  → /tmp/obc_hk.sock
//! TC APID 0x002 (Servicio 2)  → /tmp/obc_sensor.sock
//! TM de cualquier subsistema  → /tmp/obc_downlink.sock
//! ```

use obc_core::SpacePacket;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info, warn};

const ROUTER_SOCKET: &str = "/tmp/obc_router.sock";
const HK_SOCKET: &str = "/tmp/obc_hk.sock";
const SENSOR_SOCKET: &str = "/tmp/obc_sensor.sock";
const DOWNLINK_SOCKET: &str = "/tmp/obc_downlink.sock";

async fn send_to_socket(socket_path: &str, payload: &[u8]) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(socket_path).await?;
    stream.write_all(&(payload.len() as u32).to_be_bytes()).await?;
    stream.write_all(payload).await?;
    Ok(())
}

async fn build_pong(ping: &SpacePacket) -> Vec<u8> {
    // Respuesta TM(17,2) "Estoy Vivo"
    let pong = SpacePacket::new_tm(
        0x017,
        ping.seq_count,
        17, // servicio
        2,  // subservicio: Estoy Vivo
        vec![],
    );
    bincode::serde::encode_to_vec(&pong, bincode::config::standard())
        .unwrap_or_default()
}

async fn route_packet(payload: &[u8]) {
    let pkt: SpacePacket = match bincode::serde::decode_from_slice(payload, bincode::config::standard()) {
        Ok((p, _)) => p,
        Err(e) => { warn!("router: error de análisis: {e}"); return; }
    };

    let bare_apid = pkt.apid & 0x0FFF;
    info!("enrutando TC svc={}/{} apid=0x{:03X}", pkt.service, pkt.subservice, bare_apid);

    match pkt.service {
        17 if pkt.subservice == 1 => {
            // Ping — responder inmediatamente mediante enlace descendente
            info!("respondiendo a TC(17,1) ping con TM(17,2)");
            let pong = build_pong(&pkt).await;
            if let Err(e) = send_to_socket(DOWNLINK_SOCKET, &pong).await {
                warn!("no se pudo enviar pong al enlace descendente: {e}");
            }
        }
        3 => {
            if let Err(e) = send_to_socket(HK_SOCKET, payload).await {
                warn!("no se pudo reenviar al servicio HK: {e}");
            }
        }
        _ => {
            if let Err(e) = send_to_socket(SENSOR_SOCKET, payload).await {
                warn!("no hay ruta para svc={}, intentado socket sensor: {e}", pkt.service);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_target(false).with_env_filter("info").init();

    let _ = std::fs::remove_file(ROUTER_SOCKET);
    let listener = UnixListener::bind(ROUTER_SOCKET)?;
    info!("Router OBC escuchando en {ROUTER_SOCKET}");

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
                        route_packet(&buf).await;
                    }
                });
            }
            Err(e) => error!("aceptación: {e}"),
        }
    }
}
