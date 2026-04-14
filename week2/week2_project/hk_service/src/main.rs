//! HK Service — PUS Service 3 Housekeeping daemon.
//!
//! On TC(3,129) request: reads /proc/uptime and /proc/self/status,
//! packs TM(3,25) Housekeeping Parameter Report, sends it back to the
//! router for downlinking.

use obc_core::SpacePacket;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info};

const HK_SOCKET: &str = "/tmp/obc_hk.sock";
const ROUTER_SOCKET: &str = "/tmp/obc_router.sock";

/// Reads system uptime from /proc/uptime (first number, in seconds).
async fn read_uptime_s() -> f64 {
    match tokio::fs::read_to_string("/proc/uptime").await {
        Ok(s) => s.split_whitespace().next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0),
        Err(_) => 0.0,
    }
}

/// Reads VmRSS (resident set size in kB) from /proc/self/status.
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
        0x003,              // APID: HK service TM
        request.seq_count,  // mirror the TC sequence count
        3,                  // PUS service 3
        25,                 // subservice 25: HK parameter report
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
    info!("HK service listening on {HK_SOCKET}");

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
                            Err(e) => { error!("parse error: {e}"); break; }
                        };

                        if pkt.service == 3 && pkt.subservice == 129 {
                            info!("received TC(3,129): building HK report");
                            let report = build_hk_report(&pkt).await;
                            if let Err(e) = send_to_router(&report).await {
                                error!("failed to send TM to router: {e}");
                            } else {
                                info!("sent TM(3,25) HK report ({} bytes)", report.len());
                            }
                        }
                    }
                });
            }
            Err(e) => error!("accept: {e}"),
        }
    }
}
