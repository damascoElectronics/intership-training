//! Exercise 2 — Async sysfs temperature polling task
//!
//! Implement an async monitoring task that reads a sysfs temperature file
//! every 500ms and sends a warning on a channel when over threshold.
//!
//! Run tests:  cargo test --example ex2_sysfs_poll

#![allow(dead_code, unused_variables)]

use tokio::sync::mpsc;
use tokio::time::Duration;

// ─── Pre-written types ────────────────────────────────────────────────────────

/// Configuration for the thermal monitoring task.
#[derive(Clone)]
pub struct ThermalMonitor {
    /// Path to the sysfs temperature file (millidegrees C).
    pub sysfs_path: String,
    /// Warning threshold in millidegrees Celsius.
    pub warn_threshold_mc: i64,
    /// How often to poll.
    pub poll_interval: Duration,
}

/// A thermal alert emitted when the threshold is crossed.
#[derive(Debug, Clone)]
pub struct ThermalAlert {
    pub temp_mc: i64,
    pub threshold_mc: i64,
    pub message: String,
}

// ─── Your implementation ─────────────────────────────────────────────────────

/// Spawns an async monitoring task that reads `monitor.sysfs_path` every
/// `monitor.poll_interval` and sends a [`ThermalAlert`] on `tx` whenever the
/// temperature exceeds `monitor.warn_threshold_mc`.
///
/// The task runs until the `tx` sender is dropped.
pub async fn run_monitor(monitor: ThermalMonitor, tx: mpsc::Sender<ThermalAlert>) {
    todo!(
        "Use tokio::time::interval for polling. \
         Read the file with tokio::fs::read_to_string. \
         Parse the millidegree value. \
         If above threshold: send ThermalAlert on tx. \
         Stop when tx.send() returns Err (receiver dropped)."
    )
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[tokio::test]
    async fn alert_when_over_threshold() {
        // Use /proc/self/status VmRSS as a large number to exceed a tiny threshold.
        // Actually, we'll use a temp file with a hardcoded value.
        let temp_file = "/tmp/test_temp_mc";
        tokio::fs::write(temp_file, "80000").await.unwrap(); // 80°C

        let monitor = ThermalMonitor {
            sysfs_path: temp_file.into(),
            warn_threshold_mc: 70_000, // 70°C threshold
            poll_interval: Duration::from_millis(50),
        };
        let (tx, mut rx) = mpsc::channel(4);

        tokio::spawn(run_monitor(monitor, tx));

        // Should receive an alert within 200ms
        let alert = timeout(Duration::from_millis(200), rx.recv())
            .await
            .expect("no timeout")
            .expect("channel open");

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

        // Should NOT receive an alert within 200ms
        let result = timeout(Duration::from_millis(200), rx.recv()).await;
        assert!(result.is_err(), "should not receive alert below threshold");
    }
}

#[tokio::main]
async fn main() {
    println!("Run tests with: cargo test --example ex2_sysfs_poll");
}
