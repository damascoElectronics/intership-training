//! TC Receiver daemon — verifies HMAC authentication and replay protection
//! before forwarding telecommands to the OBC router.
//!
//! Security boundary: this process accepts bytes from an untrusted source
//! (simulated RF uplink) and only forwards packets that pass authentication.
//! It has NO knowledge of actuators or subsystems — it can only forward to
//! the router socket.
//!
//! # Architecture
//! ```text
//! [Ground Sim] ──(UnixSocket)──► [tc_receiver] ──(UnixSocket)──► [obc_router]
//!   TC bytes with HMAC            verify HMAC                     parse & route
//!                                 check replay window
//!                                 forward only valid TCs
//! ```

use hmac::{Hmac, Mac};
use obc_core::{IpcMessage, SpacePacket};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info, warn};

const TC_UPLINK_SOCKET: &str = "/tmp/obc_tc_uplink.sock";
const ROUTER_SOCKET: &str = "/tmp/obc_router.sock";

/// HMAC-SHA256 key shared between ground station and OBC.
/// In a real system this would be loaded from secure storage, not hardcoded.
const HMAC_KEY: &[u8] = b"spacecraft-tc-key-change-in-prod";

type HmacSha256 = Hmac<Sha256>;

/// Sliding window replay protection (64-packet window).
struct ReplayWindow {
    last_seq: u16,
    /// Bitmask: bit N set means seq (last_seq − N) was already seen.
    window: u64,
    initialized: bool,
}

impl ReplayWindow {
    fn new() -> Self {
        Self { last_seq: 0, window: 0, initialized: false }
    }

    fn check_and_advance(&mut self, seq: u16) -> Result<(), &'static str> {
        if !self.initialized {
            self.last_seq = seq;
            self.window = 1;
            self.initialized = true;
            return Ok(());
        }

        // Compute distance from last_seq (14-bit arithmetic)
        let diff = (seq as i32 - self.last_seq as i32).rem_euclid(0x4000);

        if diff == 0 {
            return Err("exact replay");
        } else if diff < 64 {
            // New packet in the window ahead of us — advance
            self.window = (self.window << diff) | 1;
            self.last_seq = seq;
        } else if diff > 0x3FC0 {
            // Old packet (behind us in window)
            let back = (0x4000 - diff) as usize;
            if back >= 64 {
                return Err("too old");
            }
            if self.window & (1u64 << back) != 0 {
                return Err("replay within window");
            }
            self.window |= 1u64 << back;
        } else {
            // Far ahead — gap in sequence (acceptable, advance window)
            self.window = 1;
            self.last_seq = seq;
        }
        Ok(())
    }
}

fn verify_hmac(packet_bytes: &[u8]) -> Result<&[u8], &'static str> {
    if packet_bytes.len() < 32 {
        return Err("packet too short for HMAC");
    }
    let (payload, claimed_mac) = packet_bytes.split_at(packet_bytes.len() - 32);
    let mut mac = HmacSha256::new_from_slice(HMAC_KEY).expect("HMAC accepts any key size");
    mac.update(payload);
    let computed: [u8; 32] = mac.finalize().into_bytes().into();
    if computed.ct_eq(claimed_mac).into() {
        Ok(payload)
    } else {
        Err("HMAC mismatch")
    }
}

async fn forward_to_router(payload: &[u8]) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(ROUTER_SOCKET).await?;
    let len = payload.len() as u32;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(payload).await?;
    Ok(())
}

async fn handle_connection(
    mut stream: UnixStream,
    replay: &mut ReplayWindow,
    accepted: &mut u64,
    rejected: &mut u64,
) {
    loop {
        // Read length-prefixed message
        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).await.is_err() {
            break; // connection closed
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 65536 {
            warn!("oversized packet ({len} bytes), dropping connection");
            break;
        }

        let mut buf = vec![0u8; len];
        if stream.read_exact(&mut buf).await.is_err() {
            break;
        }

        // 1. Verify HMAC
        let payload = match verify_hmac(&buf) {
            Ok(p) => p,
            Err(reason) => {
                warn!("rejected: {reason}");
                *rejected += 1;
                continue;
            }
        };

        // 2. Deserialize to get sequence count for replay check
        let pkt: SpacePacket = match bincode::serde::decode_from_slice(payload, bincode::config::standard()) {
            Ok((p, _)) => p,
            Err(e) => {
                warn!("rejected: deserialize failed: {e}");
                *rejected += 1;
                continue;
            }
        };

        // 3. Replay protection
        if let Err(reason) = replay.check_and_advance(pkt.seq_count) {
            warn!("rejected: {reason} (seq={})", pkt.seq_count);
            *rejected += 1;
            continue;
        }

        // 4. Forward verified payload to router
        match forward_to_router(payload).await {
            Ok(()) => {
                info!("forwarded TC apid=0x{:03X} seq={} svc={}/{}",
                      pkt.apid, pkt.seq_count, pkt.service, pkt.subservice);
                *accepted += 1;
            }
            Err(e) => {
                error!("failed to forward to router: {e}");
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter("info")
        .init();

    // Remove stale socket file
    let _ = std::fs::remove_file(TC_UPLINK_SOCKET);
    let listener = UnixListener::bind(TC_UPLINK_SOCKET)?;
    info!("TC receiver listening on {TC_UPLINK_SOCKET}");
    info!("Forwarding verified TCs to {ROUTER_SOCKET}");

    let mut replay = ReplayWindow::new();
    let mut accepted = 0u64;
    let mut rejected = 0u64;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                info!("new connection from ground station");
                handle_connection(stream, &mut replay, &mut accepted, &mut rejected).await;
                info!("connection closed: accepted={accepted} rejected={rejected}");
            }
            Err(e) => error!("accept error: {e}"),
        }
    }
}
