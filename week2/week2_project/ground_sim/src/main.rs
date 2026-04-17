//! Simulador de Estación Terrestre — arnés de prueba para la pila de software OBC.
//!
//! Envía una secuencia de TCs (algunos válidos, algunos intencionalmente inválidos) al
//! tc_receiver y recopila respuestas TM desde el socket de enlace descendente.
//! Imprime un informe de prueba PASS/FAIL al final.
//!
//! # Cómo ejecutar la pila completa
//! Ver `tools/run_obc_stack.sh`, o manualmente:
//! ```sh
//! ./target/debug/tc-receiver &
//! ./target/debug/obc-router &
//! ./target/debug/hk-service &
//! ./target/debug/sensor-daemon &
//! sleep 1
//! ./target/debug/ground-sim
//! ```

use hmac::{Hmac, Mac};
use obc_core::SpacePacket;
use sha2::Sha256;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::time::{timeout, Duration};
use tracing::info;

const TC_UPLINK_SOCKET: &str = "/tmp/obc_tc_uplink.sock";
const DOWNLINK_SOCKET: &str  = "/tmp/obc_downlink.sock";
const HMAC_KEY: &[u8]        = b"spacecraft-tc-key-change-in-prod";

type HmacSha256 = Hmac<Sha256>;

fn sign_and_frame(pkt: &SpacePacket) -> Vec<u8> {
    let payload = bincode::serde::encode_to_vec(pkt, bincode::config::standard()).unwrap();
    let mut mac = HmacSha256::new_from_slice(HMAC_KEY).unwrap();
    mac.update(&payload);
    let tag: [u8; 32] = mac.finalize().into_bytes().into();

    let mut framed = Vec::with_capacity(4 + payload.len() + 32);
    framed.extend_from_slice(&((payload.len() + 32) as u32).to_be_bytes());
    framed.extend_from_slice(&payload);
    framed.extend_from_slice(&tag);
    framed
}

fn frame_with_bad_hmac(pkt: &SpacePacket) -> Vec<u8> {
    let payload = bincode::serde::encode_to_vec(pkt, bincode::config::standard()).unwrap();
    let bad_mac = [0xFFu8; 32]; // definitivamente incorrecto
    let mut framed = Vec::with_capacity(4 + payload.len() + 32);
    framed.extend_from_slice(&((payload.len() + 32) as u32).to_be_bytes());
    framed.extend_from_slice(&payload);
    framed.extend_from_slice(&bad_mac);
    framed
}

async fn collect_tm(rx: &mut UnixListener, wait_ms: u64) -> Vec<SpacePacket> {
    let mut packets = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_millis(wait_ms);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() { break; }
        match timeout(remaining, rx.accept()).await {
            Ok(Ok((mut stream, _))) => {
                let mut len_buf = [0u8; 4];
                if stream.read_exact(&mut len_buf).await.is_err() { continue; }
                let len = u32::from_be_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                if stream.read_exact(&mut buf).await.is_err() { continue; }
                if let Ok((pkt, _)) = bincode::serde::decode_from_slice::<SpacePacket, _>(
                    &buf, bincode::config::standard()) {
                    packets.push(pkt);
                }
            }
            _ => break,
        }
    }
    packets
}

struct TestResult {
    name: String,
    passed: bool,
    detail: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_target(false).with_env_filter("warn").init();

    println!("╔══════════════════════════════════════════╗");
    println!("║  Simulador Estación Terrestre — Prueba OBC ║");
    println!("╚══════════════════════════════════════════╝");
    println!();

    // Configurar el listener de enlace descendente ANTES de enviar cualquier TC
    let _ = std::fs::remove_file(DOWNLINK_SOCKET);
    let mut downlink = UnixListener::bind(DOWNLINK_SOCKET)?;
    info!("Listener de enlace descendente listo en {DOWNLINK_SOCKET}");

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut results: Vec<TestResult> = Vec::new();
    let mut seq: u16 = 0;

    // Conectar al enlace ascendente TC
    let mut uplink = match UnixStream::connect(TC_UPLINK_SOCKET).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ERROR: no se puede conectar a {TC_UPLINK_SOCKET}: {e}");
            eprintln!("¿Está ejecutándose el tc-receiver?");
            std::process::exit(1);
        }
    };

    // ── Prueba 1: TC(17,1) ping ──────────────────────────────────────────────
    println!("[PRUEBA 1] TC(17,1) ping ¿Estás-Vivo?");
    let ping = SpacePacket::new_tc(0x017, seq, 17, 1, vec![]);
    seq += 1;
    uplink.write_all(&sign_and_frame(&ping)).await?;
    let tms = collect_tm(&mut downlink, 500).await;
    let pong_received = tms.iter().any(|p| p.service == 17 && p.subservice == 2);
    results.push(TestResult {
        name: "TC(17,1) → TM(17,2) pong".into(),
        passed: pong_received,
        detail: if pong_received { "pong recibido".into() } else { "no se recibió TM(17,2)".into() },
    });

    // ── Prueba 2: TC(3,129) solicitud HK ──────────────────────────────────
    println!("[PRUEBA 2] TC(3,129) solicitud de informe de Housekeeping");
    let hk_req = SpacePacket::new_tc(0x003, seq, 3, 129, vec![]);
    seq += 1;
    uplink.write_all(&sign_and_frame(&hk_req)).await?;
    let tms = collect_tm(&mut downlink, 1000).await;
    let hk_received = tms.iter().any(|p| p.service == 3 && p.subservice == 25);
    results.push(TestResult {
        name: "TC(3,129) → TM(3,25) informe HK".into(),
        passed: hk_received,
        detail: if hk_received {
            let hk = tms.iter().find(|p| p.service == 3).unwrap();
            format!("datos HK: {}", String::from_utf8_lossy(&hk.data))
        } else {
            "no se recibió TM(3,25)".into()
        },
    });

    // ── Prueba 3: HMAC incorrecto debe ser rechazado (no vuelve ningún TM) ────────────
    println!("[PRUEBA 3] TC con HMAC incorrecto debe ser silenciosamente rechazado");
    let bad_tc = SpacePacket::new_tc(0x017, seq, 17, 1, vec![]);
    seq += 1;
    uplink.write_all(&frame_with_bad_hmac(&bad_tc)).await?;
    let tms = collect_tm(&mut downlink, 300).await;
    let no_response = tms.is_empty();
    results.push(TestResult {
        name: "HMAC incorrecto → sin respuesta TM".into(),
        passed: no_response,
        detail: if no_response { "rechazado correctamente".into() } else { "¡se recibió TM inesperadamente!".into() },
    });

    // ── Prueba 4: Repetición — enviar el mismo seq_count de nuevo ────────────────────
    println!("[PRUEBA 4] El ataque de repetición debe ser rechazado");
    // Reenviar el ping de la prueba 1 con el mismo seq_count (0)
    let replay_ping = SpacePacket::new_tc(0x017, 0 /* mismo seq que prueba 1 */, 17, 1, vec![]);
    uplink.write_all(&sign_and_frame(&replay_ping)).await?;
    let tms = collect_tm(&mut downlink, 300).await;
    let replay_rejected = tms.is_empty();
    results.push(TestResult {
        name: "repetición (seq duplicado) → rechazado".into(),
        passed: replay_rejected,
        detail: if replay_rejected { "rechazado correctamente".into() } else { "¡la repetición fue aceptada!".into() },
    });

    // ── Imprimir resumen ──────────────────────────────────────────────────────
    println!();
    println!("══════════════════════════════════════════");
    println!("  Resultados de las Pruebas");
    println!("══════════════════════════════════════════");
    let pass_count = results.iter().filter(|r| r.passed).count();
    for r in &results {
        let status = if r.passed { "PASS" } else { "FAIL" };
        println!("  [{status}] {}", r.name);
        println!("         {}", r.detail);
    }
    println!();
    println!("  {pass_count}/{} pruebas pasadas", results.len());
    println!("══════════════════════════════════════════");

    if pass_count < results.len() {
        std::process::exit(1);
    }
    Ok(())
}
