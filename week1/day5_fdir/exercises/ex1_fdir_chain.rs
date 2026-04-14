//! Exercise 1 — Compose a FDIR chain: watchdog + circuit breaker + supervisor
//!
//! Wire together the three FDIR patterns from this day's examples into a
//! complete chain that protects access to a simulated ADC daemon.
//!
//! Run tests:  cargo test --example ex1_fdir_chain

#![allow(dead_code, unused_variables)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

// ─── Pre-written types ────────────────────────────────────────────────────────

/// A simulated ADC that fails randomly (30% chance per read).
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

    /// Returns a reading or an error (30% failure rate).
    pub async fn read(&self) -> Result<u16, &'static str> {
        let n = self.read_count.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(5)).await;
        if n % 3 == 2 {
            // Every 3rd read fails (33%)
            self.fail_count.fetch_add(1, Ordering::SeqCst);
            Err("ADC read error: device timeout")
        } else {
            Ok((n * 17 % 4096) as u16) // fake but deterministic value
        }
    }
}

/// Health states
#[derive(Debug, Clone, PartialEq)]
pub enum HealthState {
    Nominal,
    Degraded { reason: String },
    Failed { reason: String },
}

// ─── Your implementation ─────────────────────────────────────────────────────

/// A circuit breaker with three states.
pub struct CircuitBreaker {
    // TODO: add fields: state (enum), failure_count, threshold, recovery_timeout
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
        todo!("initialize with Closed state, 0 failures, given threshold and timeout")
    }

    /// Attempts to call `f`. If the breaker is Open, returns `Err(BreakerError::Open)`.
    /// If `f` fails, increments failure count; above threshold, opens the breaker.
    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, BreakerError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        todo!(
            "1. If state == Open and not enough time passed: return Err(BreakerError::Open)
             2. If state == Open and recovery_timeout elapsed: set state = HalfOpen
             3. Call f()
             4. On success: reset failure_count, set state = Closed, return Ok
             5. On failure: increment failure_count
                - If failure_count >= threshold: set state = Open, record time
                - Return Err(BreakerError::Underlying(...))"
        )
    }

    pub fn state(&self) -> BreakerState {
        todo!("return current state")
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

        // Force failures by injecting an always-failing call
        let always_fail = || async { Err::<u16, _>("forced failure".to_string()) };

        for _ in 0..3 {
            let _ = breaker.call(always_fail).await;
        }
        assert_eq!(breaker.state(), BreakerState::Open, "breaker should be open after 3 failures");
    }

    #[tokio::test]
    async fn circuit_open_rejects_immediately() {
        let breaker = Arc::new(CircuitBreaker::new(1, Duration::from_secs(60)));
        // Open the breaker
        let _ = breaker.call(|| async { Err::<u16, _>("fail".to_string()) }).await;
        assert_eq!(breaker.state(), BreakerState::Open);

        // Next call should be rejected without calling f
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let result = breaker.call(move || {
            c.fetch_add(1, Ordering::SeqCst);
            async { Ok::<u16, String>(42) }
        }).await;

        assert!(matches!(result, Err(BreakerError::Open)));
        assert_eq!(calls.load(Ordering::SeqCst), 0, "f should not be called when breaker is open");
    }

    #[tokio::test]
    async fn health_degrades_on_repeated_failures() {
        let health = Arc::new(Mutex::new(HealthState::Nominal));
        let adc = FlakyAdc::new();

        // Simulate reading 10 times; every 3rd fails
        let mut failures = 0;
        for _ in 0..10 {
            if adc.read().await.is_err() {
                failures += 1;
            }
        }

        // After 3 failures, mark as degraded
        if failures >= 3 {
            *health.lock().await = HealthState::Degraded {
                reason: format!("{failures} failures observed"),
            };
        }

        assert!(
            !matches!(*health.lock().await, HealthState::Nominal),
            "health should be degraded after failures"
        );
    }
}

#[tokio::main]
async fn main() {
    println!("Run tests with: cargo test --example ex1_fdir_chain");
}
