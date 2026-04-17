//! Ejercicio 1 — Solución

#![allow(dead_code)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct FlakyAdc {
    pub read_count: Arc<AtomicU32>,
    pub fail_count: Arc<AtomicU32>,
}

impl FlakyAdc {
    pub fn new() -> Self {
        Self { read_count: Arc::new(AtomicU32::new(0)), fail_count: Arc::new(AtomicU32::new(0)) }
    }
    pub async fn read(&self) -> Result<u16, &'static str> {
        let n = self.read_count.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(5)).await;
        if n % 3 == 2 { self.fail_count.fetch_add(1, Ordering::SeqCst); Err("timeout") }
        else { Ok((n * 17 % 4096) as u16) }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthState { Nominal, Degraded { reason: String }, Failed { reason: String } }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BreakerState { Closed, Open, HalfOpen }

#[derive(Debug)]
pub enum BreakerError { Open, Underlying(String) }

pub struct CircuitBreaker {
    state: Mutex<BreakerState>,
    failure_count: AtomicU32,
    threshold: u32,
    recovery_timeout: Duration,
    opened_at: Mutex<Option<Instant>>,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            state: Mutex::new(BreakerState::Closed),
            failure_count: AtomicU32::new(0),
            threshold,
            recovery_timeout,
            opened_at: Mutex::new(None),
        }
    }

    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, BreakerError>
    where F: FnOnce() -> Fut, Fut: std::future::Future<Output = Result<T, String>> {
        // Comprobar estado
        {
            let mut state = self.state.lock().await;
            if *state == BreakerState::Open {
                let opened = self.opened_at.lock().await;
                if opened.map(|t| t.elapsed() < self.recovery_timeout).unwrap_or(true) {
                    return Err(BreakerError::Open);
                }
                *state = BreakerState::HalfOpen;
            }
        }

        // Llamar a la función
        match f().await {
            Ok(v) => {
                self.failure_count.store(0, Ordering::SeqCst);
                *self.state.lock().await = BreakerState::Closed;
                Ok(v)
            }
            Err(e) => {
                let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.threshold {
                    *self.state.lock().await = BreakerState::Open;
                    *self.opened_at.lock().await = Some(Instant::now());
                }
                Err(BreakerError::Underlying(e))
            }
        }
    }

    pub fn state(&self) -> BreakerState {
        *self.state.try_lock().unwrap_or_else(|_| panic!("mutex envenenado"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn circuit_opens_after_threshold() {
        let breaker = CircuitBreaker::new(3, Duration::from_secs(10));
        for _ in 0..3 {
            let _ = breaker.call(|| async { Err::<u16, _>("fallo forzado".into()) }).await;
        }
        assert_eq!(breaker.state(), BreakerState::Open);
    }

    #[tokio::test]
    async fn circuit_open_rejects_immediately() {
        let breaker = Arc::new(CircuitBreaker::new(1, Duration::from_secs(60)));
        let _ = breaker.call(|| async { Err::<u16, _>("fallo".into()) }).await;
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let result = breaker.call(move || { c.fetch_add(1, Ordering::SeqCst); async { Ok::<u16, String>(42) } }).await;
        assert!(matches!(result, Err(BreakerError::Open)));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn health_degrades_on_repeated_failures() {
        let health = Arc::new(Mutex::new(HealthState::Nominal));
        let adc = FlakyAdc::new();
        let mut failures = 0;
        for _ in 0..10 { if adc.read().await.is_err() { failures += 1; } }
        if failures >= 3 {
            *health.lock().await = HealthState::Degraded { reason: format!("{failures} fallos") };
        }
        assert!(!matches!(*health.lock().await, HealthState::Nominal));
    }
}

#[tokio::main]
async fn main() {
    println!("Ejecuta los tests con: cargo test --example ex1_fdir_chain_sol");
}
