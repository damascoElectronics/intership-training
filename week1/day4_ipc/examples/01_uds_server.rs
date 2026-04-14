//! Example 01 — Unix Domain Socket server
//!
//! Unix Domain Sockets (UDS) are the workhorse of Linux daemon IPC.
//! Compared to TCP sockets:
//!  ✓  No network stack overhead — data stays in kernel
//!  ✓  File permissions control access (no network firewalls needed)
//!  ✓  Credentials can be passed (SO_PEERCRED)
//!  ✓  ~2× faster than loopback TCP
//!
//! This server uses LengthDelimitedCodec from tokio-util to frame messages.
//! Raw streams have no message boundaries — framing is your job.
//!
//! Run server first:  cargo run --example 01_uds_server
//! Then client:       cargo run --example 02_uds_client

use bincode::config::standard;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio::net::UnixListener;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use futures::{SinkExt, StreamExt};

const SOCKET_PATH: &str = "/tmp/day4_ipc_demo.sock";

/// Request from client to server.
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Ping,
    GetStatus,
    Echo { message: String },
}

/// Response from server to client.
#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Pong,
    Status { uptime_s: f64, requests_handled: u64 },
    Echoed { message: String },
    Error(String),
}

fn handle_request(req: &Request, requests: u64) -> Response {
    match req {
        Request::Ping => Response::Pong,
        Request::GetStatus => {
            let uptime_s = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            Response::Status { uptime_s, requests_handled: requests }
        }
        Request::Echo { message } => Response::Echoed { message: message.clone() },
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Remove stale socket file (from previous run)
    let _ = std::fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH)?;
    println!("Server listening on {SOCKET_PATH}");
    println!("Start client with: cargo run --example 02_uds_client\n");

    let mut total_requests = 0u64;

    loop {
        let (stream, _addr) = listener.accept().await?;
        println!("[server] new connection");

        // LengthDelimitedCodec prepends a 4-byte big-endian length to each message.
        // This solves the framing problem: we know exactly where each message ends.
        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        while let Some(frame) = framed.next().await {
            match frame {
                Ok(bytes) => {
                    let (req, _): (Request, _) = bincode::serde::decode_from_slice(&bytes, standard())?;
                    println!("[server] received: {req:?}");
                    total_requests += 1;

                    let resp = handle_request(&req, total_requests);
                    let resp_bytes = bincode::serde::encode_to_vec(&resp, standard())?;
                    framed.send(BytesMut::from(resp_bytes.as_slice()).freeze()).await?;
                    println!("[server] sent: {resp:?}");
                }
                Err(e) => {
                    eprintln!("[server] framing error: {e}");
                    break;
                }
            }
        }
        println!("[server] connection closed");
    }
}
