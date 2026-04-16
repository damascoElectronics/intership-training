//! Ejemplo 02 — Cliente Unix Domain Socket
//!
//! Se conecta al servidor del ejemplo 01 y envía una secuencia de solicitudes.
//!
//! Ejecutar el servidor primero:  cargo run --example 01_uds_server
//! Luego este:                    cargo run --example 02_uds_client

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
            eprintln!("No se puede conectar a {SOCKET_PATH}: {e}");
            eprintln!("Iniciar el servidor primero: cargo run --example 01_uds_server");
            return Ok(());
        }
    };
    println!("Conectado a {SOCKET_PATH}\n");

    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    let requests = vec![
        Request::Ping,
        Request::Echo { message: "Hola, nave espacial!".into() },
        Request::GetStatus,
        Request::Echo { message: "Otro mensaje".into() },
        Request::Ping,
    ];

    for req in &requests {
        println!("→ Enviando: {req:?}");
        let req_bytes = bincode::serde::encode_to_vec(req, standard())?;
        framed.send(BytesMut::from(req_bytes.as_slice()).freeze()).await?;

        if let Some(Ok(resp_bytes)) = framed.next().await {
            let (resp, _): (Response, _) = bincode::serde::decode_from_slice(&resp_bytes, standard())?;
            println!("← Recibido: {resp:?}");
        }
        println!();
    }

    println!("Listo.");
    Ok(())
}
