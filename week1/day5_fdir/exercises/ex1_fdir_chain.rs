//! Ejercicio 1 — Componer una cadena FDIR: watchdog + circuit breaker + supervisor
//!
//! Conecta los tres patrones FDIR de los ejemplos del día en una cadena
//! completa que protege el acceso a un daemon ADC simulado.
//!
//! Ejecutar tests:  cargo test --example ex1_fdir_chain

#![allow(dead_code, unused_variables)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

// ─── Tipos predefinidos ───────────────────────────────────────────────────────

/// Un ADC simulado que falla aleatoriamente (30% de probabilidad por lectura).
#[derive(Clone)]
pub struct FlakyAdc {
    pub read_count: Arc<AtomicU32>,
    pub fail_count: Arc<AtomicU32>,
}

impl FlakyAdc {
    pub fn new() -> Self {
        Self {
            read_count: Arc::new(AtomicU32::new(0)),
            fail_count: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Devuelve una lectura o un error (tasa de fallo del 30%).
    pub async fn read(&self) -> Result<u16, &'static str> {
        let n = self.read_count.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(5)).await;
        if n % 3 == 2 {
            // Cada 3ª lectura falla (33%)
            self.fail_count.fetch_add(1, Ordering::SeqCst);
            Err("Error de lectura ADC: timeout del dispositivo")
        } else {
            Ok((n * 17 % 4096) as u16) // valor falso pero determinista
        }
    }
}

/// Estados de salud
#[derive(Debug, Clone, PartialEq)]
pub enum HealthState {
    Nominal,
    Degraded { reason: String },
    Failed { reason: String },
}

// ─── Tu implementación ───────────────────────────────────────────────────────

/// Un cortacircuito con tres estados.
pub struct CircuitBreaker {
    // TODO: añadir campos: state (enum), failure_count, threshold, recovery_timeout
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BreakerState { Closed, Open, HalfOpen }

#[derive(Debug)]
pub enum BreakerError {
    Open,
    Underlying(String),
}

impl CircuitBreaker {
    pub fn new(threshold: u32, recovery_timeout: Duration) -> Self {
        todo!("inicializar con estado Closed, 0 fallos, threshold y timeout dados")
    }

    /// Intenta llamar a `f`. Si el breaker está Open, devuelve `Err(BreakerError::Open)`.
    /// Si `f` falla, incrementa el contador de fallos; al superar el threshold, abre el breaker.
    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, BreakerError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        todo!(
            "1. Si state == Open y no ha pasado suficiente tiempo: devolver Err(BreakerError::Open)
             2. Si state == Open y recovery_timeout ha transcurrido: establecer state = HalfOpen
             3. Llamar a f()
             4. En éxito: resetear failure_count, establecer state = Closed, devolver Ok
             5. En fallo: incrementar failure_count
                - Si failure_count >= threshold: establecer state = Open, registrar tiempo
                - Devolver Err(BreakerError::Underlying(...))"
        )
    }

    pub fn state(&self) -> BreakerState {
        todo!("devolver el estado actual")
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn circuit_opens_after_threshold() {
        let breaker = CircuitBreaker::new(3, Duration::from_secs(10));
        let adc = FlakyAdc::new();

        // Forzar fallos inyectando una llamada que siempre falla
        let always_fail = || async { Err::<u16, _>("fallo forzado".to_string()) };

        for _ in 0..3 {
            let _ = breaker.call(always_fail).await;
        }
        assert_eq!(breaker.state(), BreakerState::Open, "el breaker debe estar abierto tras 3 fallos");
    }

    #[tokio::test]
    async fn circuit_open_rejects_immediately() {
        let breaker = Arc::new(CircuitBreaker::new(1, Duration::from_secs(60)));
        // Abrir el breaker
        let _ = breaker.call(|| async { Err::<u16, _>("fallo".to_string()) }).await;
        assert_eq!(breaker.state(), BreakerState::Open);

        // La siguiente llamada debe ser rechazada sin llamar a f
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let result = breaker.call(move || {
            c.fetch_add(1, Ordering::SeqCst);
            async { Ok::<u16, String>(42) }
        }).await;

        assert!(matches!(result, Err(BreakerError::Open)));
        assert_eq!(calls.load(Ordering::SeqCst), 0, "f no debe ser llamado cuando el breaker está abierto");
    }

    #[tokio::test]
    async fn health_degrades_on_repeated_failures() {
        let health = Arc::new(Mutex::new(HealthState::Nominal));
        let adc = FlakyAdc::new();

        // Simular 10 lecturas; cada 3ª falla
        let mut failures = 0;
        for _ in 0..10 {
            if adc.read().await.is_err() {
                failures += 1;
            }
        }

        // Tras 3 fallos, marcar como degradado
        if failures >= 3 {
            *health.lock().await = HealthState::Degraded {
                reason: format!("{failures} fallos observados"),
            };
        }

        assert!(
            !matches!(*health.lock().await, HealthState::Nominal),
            "la salud debe estar degradada tras los fallos"
        );
    }
}

#[tokio::main]
async fn main() {
    println!("Ejecuta los tests con: cargo test --example ex1_fdir_chain");
}
