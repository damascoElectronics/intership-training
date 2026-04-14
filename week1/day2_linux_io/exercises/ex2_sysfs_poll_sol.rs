//! Exercise 2 — Solution

#![allow(dead_code)]

use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

#[derive(Clone)]
pub struct ThermalMonitor {
    pub sysfs_path: String,
    pub warn_threshold_mc: i64,
    pub poll_interval: Duration,
}

#[derive(Debug, Clone)]
pub struct ThermalAlert {
    pub temp_mc: i64,
    pub threshold_mc: i64,
    pub message: String,
}

pub async fn run_monitor(monitor: ThermalMonitor, tx: mpsc::Sender<ThermalAlert>) {
    let mut ticker = interval(monitor.poll_interval);
    loop {
        ticker.tick().await;
        let temp_mc: i64 = match tokio::fs::read_to_string(&monitor.sysfs_path).await {
            Ok(s) => s.trim().parse().unwrap_or(0),
            Err(_) => continue, // file not readable, skip
        };
        if temp_mc > monitor.warn_threshold_mc {
            let alert = ThermalAlert {
                temp_mc,
                threshold_mc: monitor.warn_threshold_mc,
                message: format!("Temperature {:.1}°C exceeds threshold {:.1}°C",
                                 temp_mc as f64 / 1000.0,
                                 monitor.warn_threshold_mc as f64 / 1000.0),
            };
            if tx.send(alert).await.is_err() {
                break; // receiver dropped → stop monitoring
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[tokio::test]
    async fn alert_when_over_threshold() {
        let temp_file = "/tmp/test_temp_mc_sol";
        tokio::fs::write(temp_file, "80000").await.unwrap();
        let monitor = ThermalMonitor {
            sysfs_path: temp_file.into(),
            warn_threshold_mc: 70_000,
            poll_interval: Duration::from_millis(50),
        };
        let (tx, mut rx) = mpsc::channel(4);
        tokio::spawn(run_monitor(monitor, tx));
        let alert = timeout(Duration::from_millis(200), rx.recv())
            .await.expect("no timeout").expect("channel open");
        assert_eq!(alert.temp_mc, 80_000);
    }

    #[tokio::test]
    async fn no_alert_when_below_threshold() {
        let temp_file = "/tmp/test_temp_mc_sol_low";
        tokio::fs::write(temp_file, "25000").await.unwrap();
        let monitor = ThermalMonitor {
            sysfs_path: temp_file.into(),
            warn_threshold_mc: 70_000,
            poll_interval: Duration::from_millis(50),
        };
        let (tx, mut rx) = mpsc::channel(4);
        tokio::spawn(run_monitor(monitor, tx));
        let result = timeout(Duration::from_millis(200), rx.recv()).await;
        assert!(result.is_err());
    }
}

#[tokio::main]
async fn main() {
    println!("Run tests with: cargo test --example ex2_sysfs_poll_sol");
}
