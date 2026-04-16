//! Ejemplo 01 — Servidor Unix Domain Socket
//!
//! Los Unix Domain Sockets (UDS) son el caballo de trabajo del IPC entre daemons de Linux.
//! Comparado con los sockets TCP:
//!  ✓  Sin sobrecarga de pila de red — los datos permanecen en el kernel
//!  ✓  Los permisos de archivo controlan el acceso (no se necesitan firewalls de red)
//!  ✓  Se pueden pasar credenciales (SO_PEERCRED)
//!  ✓  ~2× más rápido que TCP en loopback
//!
//! Este servidor usa LengthDelimitedCodec de tokio-util para enmarcar mensajes.
//! Los streams crudos no tienen límites de mensajes — el enmarcado es tu responsabilidad.
//!
//! Ejecutar el servidor primero:  cargo run --example 01_uds_server
//! Luego el cliente:              cargo run --example 02_uds_client

use bincode::config::standard;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio::net::UnixListener;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use futures::{SinkExt, StreamExt};

const SOCKET_PATH: &str = "/tmp/day4_ipc_demo.sock";

/// Solicitud del cliente al servidor.
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Ping,
    GetStatus,
    Echo { message: String },
}

/// Respuesta del servidor al cliente.
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
    // Eliminar el archivo de socket obsoleto (de la ejecución anterior)
    let _ = std::fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH)?;
    println!("Servidor escuchando en {SOCKET_PATH}");
    println!("Iniciar el cliente con: cargo run --example 02_uds_client\n");

    let mut total_requests = 0u64;

    loop {
        let (stream, _addr) = listener.accept().await?;
        println!("[servidor] nueva conexión");

        // LengthDelimitedCodec antepone una longitud de 4 bytes en big-endian a cada mensaje.
        // Esto resuelve el problema del enmarcado: sabemos exactamente dónde termina cada mensaje.
        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        while let Some(frame) = framed.next().await {
            match frame {
                Ok(bytes) => {
                    let (req, _): (Request, _) = bincode::serde::decode_from_slice(&bytes, standard())?;
                    println!("[servidor] recibido: {req:?}");
                    total_requests += 1;

                    let resp = handle_request(&req, total_requests);
                    let resp_bytes = bincode::serde::encode_to_vec(&resp, standard())?;
                    framed.send(BytesMut::from(resp_bytes.as_slice()).freeze()).await?;
                    println!("[servidor] enviado: {resp:?}");
                }
                Err(e) => {
                    eprintln!("[servidor] error de enmarcado: {e}");
                    break;
                }
            }
        }
        println!("[servidor] conexión cerrada");
    }
}
