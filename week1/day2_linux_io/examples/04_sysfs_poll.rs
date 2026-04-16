//! Ejemplo 04 — Monitoreo asíncrono de temperatura por sysfs
//!
//! La gestión térmica de Linux expone lecturas de sensores de temperatura a través de sysfs:
//!   /sys/class/thermal/thermal_zone0/temp  →  temperatura en miligrados C
//!
//! Este ejemplo usa tokio::time::interval para polling periódico — más
//! confiable que inotify en archivos sysfs, que no siempre disparan eventos.
//!
//! Patrón: tarea async lee sensor → comprueba umbral → emite advertencia
//!
//! Ejecutar con:  cargo run --example 04_sysfs_poll

use tokio::time::{interval, Duration};

/// Niveles de alerta de temperatura
const WARN_TEMP_MC:  i64 = 70_000;  // 70°C
const CRIT_TEMP_MC:  i64 = 85_000;  // 85°C
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Lee la temperatura de la primera zona térmica (en miligrados C).
/// Recurre a un valor simulado en sistemas sin sysfs térmico.
async fn read_temp_mc() -> i64 {
    match tokio::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").await {
        Ok(s) => s.trim().parse::<i64>().unwrap_or(25_000),
        Err(_) => {
            // Simulación: sube de 25°C a 95°C con el tiempo para demostración
            use std::time::{SystemTime, UNIX_EPOCH};
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            // 25°C + (segundos % 70) × 1°C — sube lentamente y se reinicia
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
    println!("=== Monitor de temperatura sysfs asíncrono ===");
    println!("Intervalo de polling: {}ms", POLL_INTERVAL.as_millis());
    println!("Advertencia a: {:.0}°C  |  Crítico a: {:.0}°C",
             WARN_TEMP_MC as f64 / 1000.0, CRIT_TEMP_MC as f64 / 1000.0);
    println!("Ctrl+C para detener.\n");

    let mut ticker = interval(POLL_INTERVAL);
    let mut last_alert = ThermalAlert::Nominal;
    let mut readings = 0u32;

    loop {
        ticker.tick().await;
        readings += 1;

        let temp_mc = read_temp_mc().await;
        let alert = check_threshold(temp_mc);
        let temp_c = temp_mc as f64 / 1000.0;

        // Solo registrar cuando cambia el estado o cada 10 lecturas para evitar ruido
        if alert != last_alert || readings % 10 == 0 {
            let prefix = match alert {
                ThermalAlert::Nominal  => "[ OK ]",
                ThermalAlert::Warning  => "[WARN]",
                ThermalAlert::Critical => "[CRIT]",
            };
            println!("{prefix} lectura #{readings:4}: {temp_c:.1}°C");
        }

        if alert != last_alert {
            match alert {
                ThermalAlert::Warning =>
                    println!("  → ADVERTENCIA TÉRMICA: temperatura por encima del umbral de {:.0}°C",
                             WARN_TEMP_MC as f64 / 1000.0),
                ThermalAlert::Critical =>
                    println!("  → CRÍTICO TÉRMICO: temperatura por encima de {:.0}°C!",
                             CRIT_TEMP_MC as f64 / 1000.0),
                ThermalAlert::Nominal =>
                    println!("  → Estado térmico volvió a nominal"),
            }
            last_alert = alert;
        }

        // Detener tras 20 lecturas para la demo
        if readings >= 20 {
            println!("\nDemo completa tras {readings} lecturas.");
            break;
        }
    }
}
