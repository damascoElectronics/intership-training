//! Daemon de Sensores — interfaz de hardware simulada con FDIR integrado.
//!
//! Genera lecturas simuladas de sensores de temperatura y presión.
//! Aplica FDIR: tras 3 lecturas anómalas consecutivas, se marca a sí mismo
//! como Degradado y deja de reportar.

use obc_core::{HealthState, SpacePacket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{info, warn};

const SENSOR_SOCKET: &str = "/tmp/obc_sensor.sock";
const ROUTER_SOCKET: &str = "/tmp/obc_router.sock";

const TEMP_NOMINAL_MC: i32 = 25_000;  // 25,0 °C en milligrados
const TEMP_LIMIT_MC: i32   = 60_000;  // umbral de fallo a 60,0 °C
const PRESSURE_PA: u32     = 101_325; // 1 atm

struct SensorState {
    consecutive_faults: u32,
    health: HealthState,
    reading_count: u64,
}

impl SensorState {
    fn new() -> Self { Self { consecutive_faults: 0, health: HealthState::Nominal, reading_count: 0 } }

    fn read_temperature_mc(&mut self) -> i32 {
        self.reading_count += 1;
        // Simular un aumento lento de temperatura con picos ocasionales
        let base = TEMP_NOMINAL_MC + (self.reading_count as i32 * 100);
        // Cada 7.ª lectura: inyectar un pico por encima del límite (simula un fallo transitorio)
        if self.reading_count % 7 == 0 {
            TEMP_LIMIT_MC + 5_000  // fallo de sobretemperatura
        } else {
            base.min(TEMP_NOMINAL_MC + 10_000) // limitar a +10°C de deriva nominal
        }
    }

    fn apply_fdir(&mut self, temp_mc: i32) {
        if temp_mc > TEMP_LIMIT_MC {
            self.consecutive_faults += 1;
            warn!("FDIR: lectura anómala temp={:.1}°C (fallo #{}/3)",
                  temp_mc as f64 / 1000.0, self.consecutive_faults);
            if self.consecutive_faults >= 3 {
                self.health = HealthState::Degraded {
                    reason: format!("3 lecturas consecutivas de sobretemperatura (última: {:.1}°C)", temp_mc as f64 / 1000.0)
                };
            }
        } else {
            if self.consecutive_faults > 0 {
                info!("FDIR: lectura nominal de nuevo, reiniciando contador de fallos");
            }
            self.consecutive_faults = 0;
        }
    }
}

async fn send_to_router(payload: &[u8]) -> std::io::Result<()> {
    let mut s = UnixStream::connect(ROUTER_SOCKET).await?;
    s.write_all(&(payload.len() as u32).to_be_bytes()).await?;
    s.write_all(payload).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_target(false).with_env_filter("info").init();

    let _ = std::fs::remove_file(SENSOR_SOCKET);
    let listener = UnixListener::bind(SENSOR_SOCKET)?;
    info!("Daemon de sensores escuchando en {SENSOR_SOCKET}");

    let mut state = SensorState::new();

    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                loop {
                    let mut len_buf = [0u8; 4];
                    if stream.read_exact(&mut len_buf).await.is_err() { break; }
                    let len = u32::from_be_bytes(len_buf) as usize;
                    if len > 65536 { break; }
                    let mut buf = vec![0u8; len];
                    if stream.read_exact(&mut buf).await.is_err() { break; }

                    let pkt: SpacePacket = match bincode::serde::decode_from_slice(
                        &buf, bincode::config::standard()) {
                        Ok((p, _)) => p,
                        Err(_) => break,
                    };

                    // Leer sensor
                    let temp_mc = state.read_temperature_mc();
                    state.apply_fdir(temp_mc);

                    // Construir TM de respuesta
                    let report = serde_json::json!({
                        "temperature_mc": temp_mc,
                        "pressure_pa": PRESSURE_PA,
                        "health": format!("{}", state.health),
                        "reading_count": state.reading_count,
                    });

                    let tm = SpacePacket::new_tm(0x002, pkt.seq_count, 2, 1, report.to_string().into_bytes());
                    let bytes = bincode::serde::encode_to_vec(&tm, bincode::config::standard())
                        .unwrap_or_default();

                    info!("lectura de sensor #{}: temp={:.1}°C estado={}",
                          state.reading_count, temp_mc as f64 / 1000.0, state.health);

                    if let Err(e) = send_to_router(&bytes).await {
                        warn!("no se pudo enviar TM al router: {e}");
                    }
                }
            }
            Err(e) => tracing::error!("aceptación: {e}"),
        }
    }
}
