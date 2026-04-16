//! Ejercicio 2 — Tarea de polling asíncrono de temperatura en sysfs
//!
//! Implementa una tarea de monitoreo asíncrono que lee un archivo de temperatura
//! de sysfs cada 500ms y envía una advertencia en un canal cuando supera el umbral.
//!
//! Ejecutar tests:  cargo test --example ex2_sysfs_poll

#![allow(dead_code, unused_variables)]

use tokio::sync::mpsc;
use tokio::time::Duration;

// ─── Tipos predefinidos ───────────────────────────────────────────────────────

/// Configuración para la tarea de monitoreo térmico.
#[derive(Clone)]
pub struct ThermalMonitor {
    /// Ruta al archivo de temperatura sysfs (miligrados C).
    pub sysfs_path: String,
    /// Umbral de advertencia en miligrados Celsius.
    pub warn_threshold_mc: i64,
    /// Con qué frecuencia hacer polling.
    pub poll_interval: Duration,
}

/// Una alerta térmica emitida cuando se cruza el umbral.
#[derive(Debug, Clone)]
pub struct ThermalAlert {
    pub temp_mc: i64,
    pub threshold_mc: i64,
    pub message: String,
}

// ─── Tu implementación ───────────────────────────────────────────────────────

/// Lanza una tarea de monitoreo asíncrono que lee `monitor.sysfs_path` cada
/// `monitor.poll_interval` y envía un [`ThermalAlert`] en `tx` cuando la
/// temperatura supera `monitor.warn_threshold_mc`.
///
/// La tarea se ejecuta hasta que el emisor `tx` se descarta.
pub async fn run_monitor(monitor: ThermalMonitor, tx: mpsc::Sender<ThermalAlert>) {
    todo!(
        "Usa tokio::time::interval para el polling. \
         Lee el archivo con tokio::fs::read_to_string. \
         Parsea el valor en miligrados. \
         Si supera el umbral: envía ThermalAlert en tx. \
         Detente cuando tx.send() devuelva Err (receptor descartado)."
    )
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[tokio::test]
    async fn alert_when_over_threshold() {
        // Usa /proc/self/status VmRSS como un número grande para superar un umbral pequeño.
        // En realidad, usaremos un archivo temporal con un valor fijo.
        let temp_file = "/tmp/test_temp_mc";
        tokio::fs::write(temp_file, "80000").await.unwrap(); // 80°C

        let monitor = ThermalMonitor {
            sysfs_path: temp_file.into(),
            warn_threshold_mc: 70_000, // umbral de 70°C
            poll_interval: Duration::from_millis(50),
        };
        let (tx, mut rx) = mpsc::channel(4);

        tokio::spawn(run_monitor(monitor, tx));

        // Debe recibir una alerta en 200ms
        let alert = timeout(Duration::from_millis(200), rx.recv())
            .await
            .expect("sin timeout")
            .expect("canal abierto");

        assert_eq!(alert.temp_mc, 80_000);
        assert!(alert.temp_mc > alert.threshold_mc);
    }

    #[tokio::test]
    async fn no_alert_when_below_threshold() {
        let temp_file = "/tmp/test_temp_mc_low";
        tokio::fs::write(temp_file, "25000").await.unwrap(); // 25°C

        let monitor = ThermalMonitor {
            sysfs_path: temp_file.into(),
            warn_threshold_mc: 70_000,
            poll_interval: Duration::from_millis(50),
        };
        let (tx, mut rx) = mpsc::channel(4);

        tokio::spawn(run_monitor(monitor, tx));

        // NO debe recibir una alerta en 200ms
        let result = timeout(Duration::from_millis(200), rx.recv()).await;
        assert!(result.is_err(), "no debe recibir alerta por debajo del umbral");
    }
}

#[tokio::main]
async fn main() {
    println!("Ejecutar tests con: cargo test --example ex2_sysfs_poll");
}
