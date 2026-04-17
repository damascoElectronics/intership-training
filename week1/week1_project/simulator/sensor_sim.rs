//! Simulador de sensor — binario complementario para el daemon sensor.
//!
//! Escucha en un socket Unix, acepta una conexión y transmite
//! mensajes SensorFrame cada 200ms. Cada 10ª trama inyecta un fallo.
//!
//! Uso:
//!   cargo run --bin sensor-sim        # modo normal
//!   cargo run --bin sensor-sim crash  # entra en pánico tras 20 tramas (prueba el reinicio)

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;

const SOCKET_PATH: &str = "/tmp/sensor_sim.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorFrame {
    pub timestamp_ms: u64,
    pub temperature_mc: i32,
    pub pressure_pa: u32,
    pub fault_injected: bool,
}

#[tokio::main]
async fn main() {
    let crash_mode = std::env::args().any(|a| a == "crash");
    let _ = std::fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH).expect("bind");
    eprintln!("[sim] escuchando en {SOCKET_PATH}{}",
              if crash_mode { " (modo crash: entrará en pánico tras 20 tramas)" } else { "" });

    loop {
        let (mut stream, _) = listener.accept().await.expect("accept");
        eprintln!("[sim] cliente conectado");

        let mut frame_count = 0u32;
        loop {
            tokio::time::sleep(Duration::from_millis(200)).await;
            frame_count += 1;

            if crash_mode && frame_count > 20 {
                eprintln!("[sim] modo crash: ¡entrando en pánico!");
                panic!("crash deliberado para probar el reinicio del supervisor");
            }

            let fault = frame_count % 10 == 0;
            let frame = SensorFrame {
                timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64).unwrap_or(0),
                temperature_mc: 24_000 + (frame_count as i32 % 20) * 100,
                pressure_pa: 101_325,
                fault_injected: fault,
            };

            if fault {
                eprintln!("[sim] inyectando fallo en trama #{frame_count}");
            }

            let bytes = bincode::serde::encode_to_vec(&frame, bincode::config::standard())
                .expect("encode");
            if stream.write_all(&(bytes.len() as u32).to_be_bytes()).await.is_err() { break; }
            if stream.write_all(&bytes).await.is_err() { break; }
        }
        eprintln!("[sim] cliente desconectado");
    }
}
