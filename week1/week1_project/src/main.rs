//! Daemon Sensor — Proyecto capstone de la Semana 1.
//!
//! Integra todos los temas de los Días 1–5:
//!  - Día 1: runtime async tokio, apagado elegante
//!  - Día 2: patrones de I/O en Linux (lecturas por socket Unix)
//!  - Día 4: IPC (Unix Domain Sockets, enmarcado con bincode)
//!  - Día 5: FDIR (watchdog, supervisor)
//!
//! Arquitectura:
//! ```text
//! [sensor-sim] ──(socket Unix)──► [sensor-daemon]
//!                                   │
//!                                   ├── tarea watchdog
//!                                   ├── supervisor (reinicia el sensor en fallo)
//!                                   └── buffer de telemetría (estadísticas)
//! ```
//!
//! Ejecutar sensor-sim primero, luego sensor-daemon:
//!   cargo run --bin sensor-sim &
//!   cargo run --bin sensor-daemon

mod health;
mod sensor;
mod supervisor;
mod telemetry;

use health::{HealthState, HealthTable};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    // Logging JSON estructurado — habitual en daemons de producción
    tracing_subscriber::fmt()
        .json()
        .with_current_span(false)
        .with_target(false)
        .init();

    info!("Daemon sensor iniciando");

    // Estado compartido
    let health_table = Arc::new(RwLock::new(HealthTable::new()));
    let telem_buffer = Arc::new(Mutex::new(telemetry::TelemetryBuffer::new(100)));
    let shutdown = CancellationToken::new();

    // Apagado elegante en Ctrl+C / SIGTERM
    {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            info!("señal de apagado recibida");
            shutdown.cancel();
        });
    }

    // El supervisor gestiona la tarea de lectura del sensor
    let sup = supervisor::Supervisor::new(
        Arc::clone(&health_table),
        Arc::clone(&telem_buffer),
        shutdown.clone(),
    );

    sup.run().await;

    // Informe de salud final
    let health = health_table.read().await;
    info!(overall = ?health.overall(), "apagado del daemon sensor completado");
}
