//! Sensor reader task — connects to sensor-sim via Unix socket.

use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub const SENSOR_SIM_SOCKET: &str = "/tmp/sensor_sim.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorFrame {
    pub timestamp_ms: u64,
    pub temperature_mc: i32,  // millidegrees Celsius
    pub pressure_pa: u32,     // Pascals
    pub fault_injected: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SensorError {
    #[error("cannot connect to sensor sim: {0}")]
    Connection(#[from] std::io::Error),
    #[error("frame decode error")]
    Decode,
    #[error("fault injected")]
    FaultInjected,
}

/// Reads sensor frames from the sensor-sim daemon.
///
/// Returns Err when the connection is lost or a fault is injected.
/// The caller (supervisor) decides whether to restart.
pub async fn read_sensor_frames(
    tx: tokio::sync::mpsc::Sender<SensorFrame>,
    token: CancellationToken,
) -> Result<(), SensorError> {
    let mut stream = UnixStream::connect(SENSOR_SIM_SOCKET).await?;
    info!("connected to sensor sim at {SENSOR_SIM_SOCKET}");

    loop {
        // Read 4-byte length prefix
        let mut len_buf = [0u8; 4];
        tokio::select! {
            res = stream.read_exact(&mut len_buf) => {
                if res.is_err() { return Err(SensorError::Connection(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "connection closed"))); }
            }
            _ = token.cancelled() => return Ok(()),
        }

        let len = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await.map_err(SensorError::Connection)?;

        let (frame, _): (SensorFrame, _) = bincode::serde::decode_from_slice(&buf, bincode::config::standard())
            .map_err(|_| SensorError::Decode)?;

        if frame.fault_injected {
            warn!("fault injected in frame, treating as sensor error");
            return Err(SensorError::FaultInjected);
        }

        info!(
            temp_c = frame.temperature_mc as f64 / 1000.0,
            pressure_pa = frame.pressure_pa,
            "sensor frame received"
        );

        if tx.send(frame).await.is_err() {
            break; // consumer dropped
        }
    }
    Ok(())
}
