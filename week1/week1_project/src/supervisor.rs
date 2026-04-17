//! Supervisor de tareas — reinicia la tarea de lectura del sensor en caso de fallo.

use crate::health::{HealthState, HealthTable};
use crate::sensor::{self, SensorFrame};
use crate::telemetry::TelemetryBuffer;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

const MAX_RESTARTS: u32 = 5;
const BASE_BACKOFF_MS: u64 = 500;

pub struct Supervisor {
    health: Arc<RwLock<HealthTable>>,
    telem: Arc<Mutex<TelemetryBuffer>>,
    shutdown: CancellationToken,
}

impl Supervisor {
    pub fn new(
        health: Arc<RwLock<HealthTable>>,
        telem: Arc<Mutex<TelemetryBuffer>>,
        shutdown: CancellationToken,
    ) -> Self {
        Self { health, telem, shutdown }
    }

    pub async fn run(self) {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<SensorFrame>(32);
        let mut restarts = 0u32;
        let health = Arc::clone(&self.health);
        let telem = Arc::clone(&self.telem);
        let shutdown = self.shutdown.clone();

        // Lanzar tarea consumidora de TM
        let consumer_shutdown = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(frame) = rx.recv() => {
                        telem.lock().await.push(frame);
                    }
                    _ = consumer_shutdown.cancelled() => break,
                }
            }
        });

        // Bucle del supervisor
        loop {
            if shutdown.is_cancelled() { break; }

            let child_token = shutdown.child_token();
            let task_tx = tx.clone();

            info!(attempt = restarts + 1, "iniciando tarea de lectura de sensor");
            health.write().await.set("sensor", HealthState::Nominal);

            let result = sensor::read_sensor_frames(task_tx, child_token).await;

            match result {
                Ok(()) => {
                    info!("tarea sensor terminó limpiamente");
                    break;
                }
                Err(e) => {
                    restarts += 1;
                    let reason = e.to_string();
                    warn!(restarts, reason, "tarea sensor falló");

                    if restarts >= MAX_RESTARTS {
                        error!("máximo de reinicios ({MAX_RESTARTS}) superado, marcando como FALLIDO");
                        health.write().await.set("sensor", HealthState::Failed { reason });
                        break;
                    } else {
                        health.write().await.set("sensor", HealthState::Degraded { reason });
                        // Retroceso exponencial
                        let delay_ms = BASE_BACKOFF_MS * (1u64 << restarts.min(6));
                        warn!(delay_ms, "esperando antes de reiniciar");
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }
    }
}
