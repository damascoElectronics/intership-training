//! Example 02 — Unix Domain Socket client
//!
//! Connects to the server from example 01 and sends a sequence of requests.
//!
//! Run server first:  cargo run --example 01_uds_server
//! Then this:         cargo run --example 02_uds_client

use bincode::config::standard;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use futures::{SinkExt, StreamExt};

const SOCKET_PATH: &str = "/tmp/day4_ipc_demo.sock";

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Ping,
    GetStatus,
    Echo { message: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Pong,
    Status { uptime_s: f64, requests_handled: u64 },
    Echoed { message: String },
    Error(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream = match UnixStream::connect(SOCKET_PATH).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Cannot connect to {SOCKET_PATH}: {e}");
            eprintln!("Start the server first: cargo run --example 01_uds_server");
            return Ok(());
        }
    };
    println!("Connected to {SOCKET_PATH}\n");

    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    let requests = vec![
        Request::Ping,
        Request::Echo { message: "Hello, spacecraft!".into() },
        Request::GetStatus,
        Request::Echo { message: "Another message".into() },
        Request::Ping,
    ];

    for req in &requests {
        println!("→ Sending: {req:?}");
        let req_bytes = bincode::serde::encode_to_vec(req, standard())?;
        framed.send(BytesMut::from(req_bytes.as_slice()).freeze()).await?;

        if let Some(Ok(resp_bytes)) = framed.next().await {
            let (resp, _): (Response, _) = bincode::serde::decode_from_slice(&resp_bytes, standard())?;
            println!("← Received: {resp:?}");
        }
        println!();
    }

    println!("Done.");
    Ok(())
}
