//! Example 04 — Async sysfs temperature monitoring
//!
//! Linux thermal management exposes temperature sensor readings through sysfs:
//!   /sys/class/thermal/thermal_zone0/temp  →  temperature in millidegrees C
//!
//! This example uses tokio::time::interval for periodic polling — more
//! reliable than inotify on sysfs files, which don't always trigger events.
//!
//! Pattern: async task reads sensor → checks threshold → emits warning
//!
//! Run with:  cargo run --example 04_sysfs_poll

use tokio::time::{interval, Duration};

/// Temperature alert levels
const WARN_TEMP_MC:  i64 = 70_000;  // 70°C
const CRIT_TEMP_MC:  i64 = 85_000;  // 85°C
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Reads the temperature from the first thermal zone (in millidegrees C).
/// Falls back to a simulated value on systems without thermal sysfs.
async fn read_temp_mc() -> i64 {
    match tokio::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").await {
        Ok(s) => s.trim().parse::<i64>().unwrap_or(25_000),
        Err(_) => {
            // Simulate: ramp from 25°C to 95°C over time for demo purposes
            use std::time::{SystemTime, UNIX_EPOCH};
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            // 25°C + (seconds % 70) × 1°C — slowly climbs then resets
            (25_000 + (secs % 70) as i64 * 1_000).min(95_000)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ThermalAlert { Nominal, Warning, Critical }

fn check_threshold(temp_mc: i64) -> ThermalAlert {
    if temp_mc >= CRIT_TEMP_MC {
        ThermalAlert::Critical
    } else if temp_mc >= WARN_TEMP_MC {
        ThermalAlert::Warning
    } else {
        ThermalAlert::Nominal
    }
}

#[tokio::main]
async fn main() {
    println!("=== Async sysfs temperature monitor ===");
    println!("Poll interval: {}ms", POLL_INTERVAL.as_millis());
    println!("Warning at: {:.0}°C  |  Critical at: {:.0}°C",
             WARN_TEMP_MC as f64 / 1000.0, CRIT_TEMP_MC as f64 / 1000.0);
    println!("Ctrl+C to stop.\n");

    let mut ticker = interval(POLL_INTERVAL);
    let mut last_alert = ThermalAlert::Nominal;
    let mut readings = 0u32;

    loop {
        ticker.tick().await;
        readings += 1;

        let temp_mc = read_temp_mc().await;
        let alert = check_threshold(temp_mc);
        let temp_c = temp_mc as f64 / 1000.0;

        // Only log on state change or every 10 readings to avoid noise
        if alert != last_alert || readings % 10 == 0 {
            let prefix = match alert {
                ThermalAlert::Nominal  => "[ OK ]",
                ThermalAlert::Warning  => "[WARN]",
                ThermalAlert::Critical => "[CRIT]",
            };
            println!("{prefix} reading #{readings:4}: {temp_c:.1}°C");
        }

        if alert != last_alert {
            match alert {
                ThermalAlert::Warning =>
                    println!("  → THERMAL WARNING: temperature above {:.0}°C threshold",
                             WARN_TEMP_MC as f64 / 1000.0),
                ThermalAlert::Critical =>
                    println!("  → THERMAL CRITICAL: temperature above {:.0}°C!",
                             CRIT_TEMP_MC as f64 / 1000.0),
                ThermalAlert::Nominal =>
                    println!("  → Thermal state returned to nominal"),
            }
            last_alert = alert;
        }

        // Stop after 20 readings for the demo
        if readings >= 20 {
            println!("\nDemo complete after {readings} readings.");
            break;
        }
    }
}
