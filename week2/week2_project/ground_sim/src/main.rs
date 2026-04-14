//! Ground Station Simulator — test harness for the OBC software stack.
//!
//! Sends a sequence of TCs (some valid, some intentionally invalid) to the
//! tc_receiver and collects TM responses from the downlink socket.
//! Prints a PASS/FAIL test report at the end.
//!
//! # How to run the full stack
//! See `tools/run_obc_stack.sh`, or manually:
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
    let bad_mac = [0xFFu8; 32]; // definitely wrong
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
    println!("║    Ground Station Simulator — OBC Test   ║");
    println!("╚══════════════════════════════════════════╝");
    println!();

    // Set up downlink listener BEFORE sending any TCs
    let _ = std::fs::remove_file(DOWNLINK_SOCKET);
    let mut downlink = UnixListener::bind(DOWNLINK_SOCKET)?;
    info!("Downlink listener ready on {DOWNLINK_SOCKET}");

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut results: Vec<TestResult> = Vec::new();
    let mut seq: u16 = 0;

    // Connect to TC uplink
    let mut uplink = match UnixStream::connect(TC_UPLINK_SOCKET).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ERROR: cannot connect to {TC_UPLINK_SOCKET}: {e}");
            eprintln!("Is the tc-receiver running?");
            std::process::exit(1);
        }
    };

    // ── Test 1: TC(17,1) ping ──────────────────────────────────────────────
    println!("[TEST 1] TC(17,1) Are-You-Alive ping");
    let ping = SpacePacket::new_tc(0x017, seq, 17, 1, vec![]);
    seq += 1;
    uplink.write_all(&sign_and_frame(&ping)).await?;
    let tms = collect_tm(&mut downlink, 500).await;
    let pong_received = tms.iter().any(|p| p.service == 17 && p.subservice == 2);
    results.push(TestResult {
        name: "TC(17,1) → TM(17,2) pong".into(),
        passed: pong_received,
        detail: if pong_received { "pong received".into() } else { "no TM(17,2) received".into() },
    });

    // ── Test 2: TC(3,129) HK request ──────────────────────────────────────
    println!("[TEST 2] TC(3,129) Housekeeping report request");
    let hk_req = SpacePacket::new_tc(0x003, seq, 3, 129, vec![]);
    seq += 1;
    uplink.write_all(&sign_and_frame(&hk_req)).await?;
    let tms = collect_tm(&mut downlink, 1000).await;
    let hk_received = tms.iter().any(|p| p.service == 3 && p.subservice == 25);
    results.push(TestResult {
        name: "TC(3,129) → TM(3,25) HK report".into(),
        passed: hk_received,
        detail: if hk_received {
            let hk = tms.iter().find(|p| p.service == 3).unwrap();
            format!("HK data: {}", String::from_utf8_lossy(&hk.data))
        } else {
            "no TM(3,25) received".into()
        },
    });

    // ── Test 3: Bad HMAC should be rejected (no TM comes back) ────────────
    println!("[TEST 3] TC with bad HMAC should be silently rejected");
    let bad_tc = SpacePacket::new_tc(0x017, seq, 17, 1, vec![]);
    seq += 1;
    uplink.write_all(&frame_with_bad_hmac(&bad_tc)).await?;
    let tms = collect_tm(&mut downlink, 300).await;
    let no_response = tms.is_empty();
    results.push(TestResult {
        name: "bad HMAC → no TM response".into(),
        passed: no_response,
        detail: if no_response { "correctly rejected".into() } else { "unexpectedly got TM!".into() },
    });

    // ── Test 4: Replay — send the same seq_count again ────────────────────
    println!("[TEST 4] Replay attack should be rejected");
    // Re-send test 1's ping with the same seq_count (0)
    let replay_ping = SpacePacket::new_tc(0x017, 0 /* same seq as test 1 */, 17, 1, vec![]);
    uplink.write_all(&sign_and_frame(&replay_ping)).await?;
    let tms = collect_tm(&mut downlink, 300).await;
    let replay_rejected = tms.is_empty();
    results.push(TestResult {
        name: "replay (dup seq) → rejected".into(),
        passed: replay_rejected,
        detail: if replay_rejected { "correctly rejected".into() } else { "replay was accepted!".into() },
    });

    // ── Print summary ──────────────────────────────────────────────────────
    println!();
    println!("══════════════════════════════════════════");
    println!("  Test Results");
    println!("══════════════════════════════════════════");
    let pass_count = results.iter().filter(|r| r.passed).count();
    for r in &results {
        let status = if r.passed { "PASS" } else { "FAIL" };
        println!("  [{status}] {}", r.name);
        println!("         {}", r.detail);
    }
    println!();
    println!("  {pass_count}/{} tests passed", results.len());
    println!("══════════════════════════════════════════");

    if pass_count < results.len() {
        std::process::exit(1);
    }
    Ok(())
}
