//! Sensor Daemon — Week 1 capstone project.
//!
//! Integrates all Day 1–5 topics:
//!  - Day 1: tokio async runtime, graceful shutdown
//!  - Day 2: Linux I/O patterns (Unix socket reads)
//!  - Day 4: IPC (Unix Domain Sockets, bincode framing)
//!  - Day 5: FDIR (watchdog, supervisor)
//!
//! Architecture:
//! ```text
//! [sensor-sim] ──(Unix socket)──► [sensor-daemon]
//!                                   │
//!                                   ├── watchdog task
//!                                   ├── supervisor (restarts sensor on failure)
//!                                   └── telemetry buffer (stats)
//! ```
//!
//! Run sensor-sim first, then sensor-daemon:
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
    // Structured JSON logging — common in production daemons
    tracing_subscriber::fmt()
        .json()
        .with_current_span(false)
        .with_target(false)
        .init();

    info!("Sensor daemon starting");

    // Shared state
    let health_table = Arc::new(RwLock::new(HealthTable::new()));
    let telem_buffer = Arc::new(Mutex::new(telemetry::TelemetryBuffer::new(100)));
    let shutdown = CancellationToken::new();

    // Graceful shutdown on Ctrl+C / SIGTERM
    {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            info!("shutdown signal received");
            shutdown.cancel();
        });
    }

    // Supervisor manages the sensor reading task
    let sup = supervisor::Supervisor::new(
        Arc::clone(&health_table),
        Arc::clone(&telem_buffer),
        shutdown.clone(),
    );

    sup.run().await;

    // Final health report
    let health = health_table.read().await;
    info!(overall = ?health.overall(), "sensor daemon shutdown complete");
}
