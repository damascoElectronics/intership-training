//! Tarea de lectura del sensor — se conecta al sensor-sim mediante socket Unix.

use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub const SENSOR_SIM_SOCKET: &str = "/tmp/sensor_sim.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorFrame {
    pub timestamp_ms: u64,
    pub temperature_mc: i32,  // miligrados Celsius
    pub pressure_pa: u32,     // Pascales
    pub fault_injected: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SensorError {
    #[error("no se puede conectar al simulador de sensor: {0}")]
    Connection(#[from] std::io::Error),
    #[error("error de decodificación de trama")]
    Decode,
    #[error("fallo inyectado")]
    FaultInjected,
}

/// Lee tramas de sensor desde el daemon sensor-sim.
///
/// Devuelve Err cuando se pierde la conexión o se inyecta un fallo.
/// El llamador (supervisor) decide si reiniciar.
pub async fn read_sensor_frames(
    tx: tokio::sync::mpsc::Sender<SensorFrame>,
    token: CancellationToken,
) -> Result<(), SensorError> {
    let mut stream = UnixStream::connect(SENSOR_SIM_SOCKET).await?;
    info!("conectado al simulador de sensor en {SENSOR_SIM_SOCKET}");

    loop {
        // Leer prefijo de longitud de 4 bytes
        let mut len_buf = [0u8; 4];
        tokio::select! {
            res = stream.read_exact(&mut len_buf) => {
                if res.is_err() { return Err(SensorError::Connection(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "conexión cerrada"))); }
            }
            _ = token.cancelled() => return Ok(()),
        }

        let len = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await.map_err(SensorError::Connection)?;

        let (frame, _): (SensorFrame, _) = bincode::serde::decode_from_slice(&buf, bincode::config::standard())
            .map_err(|_| SensorError::Decode)?;

        if frame.fault_injected {
            warn!("fallo inyectado en la trama, tratando como error de sensor");
            return Err(SensorError::FaultInjected);
        }

        info!(
            temp_c = frame.temperature_mc as f64 / 1000.0,
            pressure_pa = frame.pressure_pa,
            "trama de sensor recibida"
        );

        if tx.send(frame).await.is_err() {
            break; // consumidor descartado
        }
    }
    Ok(())
}
