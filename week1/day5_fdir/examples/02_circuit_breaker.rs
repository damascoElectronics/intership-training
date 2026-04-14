//! Example 02: Circuit Breaker
//!
//! Problem: a remote component (sensor daemon, I2C device, network service) is failing.
//! Naively retrying hammers the failing component and wastes CPU. Worse, if each call
//! blocks for a timeout before failing, a saturated call loop can starve other tasks.
//!
//! The circuit breaker pattern — borrowed from electrical engineering and popularised
//! by Michael Nygard ("Release It!") — solves this:
//!   CLOSED  → normal operation, failures increment counter
//!   OPEN    → fast-fail: don't even try, return error immediately
//!   HALF-OPEN → probe: let one call through to see if recovery happened
//!
//! This is especially useful in embedded daemons that talk to hardware over I2C/SPI/UART,
//! where a hung bus can block an entire read for seconds.
//!
//! Run: cargo run --example 02_circuit_breaker

use std::{
    future::Future,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use tokio::time::sleep;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

/// The three states of the circuit breaker.
///
/// Why an enum rather than separate structs?  Here we need runtime state transitions
/// driven by external events, so a plain enum is cleaner. Compare to the typestate
/// pattern in 05_safe_state.rs where transitions are compile-time.
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Normal operation. Calls are forwarded. Failures increment the counter.
    Closed,
    /// The circuit has tripped. Calls fail immediately without reaching the component.
    /// After `recovery_timeout`, the breaker probes by moving to HalfOpen.
    Open,
    /// One probe call is allowed through.
    /// Success → Closed (reset counter).  Failure → back to Open.
    HalfOpen,
}

// ---------------------------------------------------------------------------
// CircuitBreaker
// ---------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum CircuitBreakerError<E: std::fmt::Debug> {
    /// The underlying call returned an error.
    #[error("call failed: {0:?}")]
    CallFailed(E),
    /// The circuit is open; call was rejected without trying.
    #[error("circuit open — fast-fail")]
    CircuitOpen,
}

/// A circuit breaker that wraps async calls to a potentially-failing resource.
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failure_count: Arc<AtomicU32>,
    /// How many consecutive failures before we trip to Open.
    threshold: u32,
    /// How long to stay Open before probing.
    recovery_timeout: Duration,
    /// When did we last transition to Open? Used to check if timeout elapsed.
    last_failure_time: Arc<Mutex<Option<Instant>>>,
    /// Name, for logging.
    name: String,
}

impl CircuitBreaker {
    pub fn new(name: impl Into<String>, threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed)),
            failure_count: Arc::new(AtomicU32::new(0)),
            threshold,
            recovery_timeout,
            last_failure_time: Arc::new(Mutex::new(None)),
            name: name.into(),
        }
    }

    /// Wrap an async call with circuit-breaker logic.
    ///
    /// `f` produces a Future that represents one attempt to call the resource.
    /// Returns `Ok(T)` on success, or a `CircuitBreakerError` on failure/trip.
    pub async fn call<F, Fut, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        // --- Pre-call: decide whether to allow the call through ---
        let current_state = {
            let mut state = self.state.lock().unwrap();

            // If we're Open, check whether the recovery timeout has elapsed.
            if *state == CircuitState::Open {
                let last_fail = self.last_failure_time.lock().unwrap();
                if let Some(t) = *last_fail {
                    if t.elapsed() >= self.recovery_timeout {
                        // Enough time has passed; let one probe through.
                        info!(cb = %self.name, "moving OPEN → HALF-OPEN (probing)");
                        *state = CircuitState::HalfOpen;
                    }
                }
            }

            state.clone()
        };

        match current_state {
            CircuitState::Open => {
                // Fast-fail: don't call the underlying function at all.
                warn!(cb = %self.name, "circuit OPEN — rejecting call");
                return Err(CircuitBreakerError::CircuitOpen);
            }
            CircuitState::Closed | CircuitState::HalfOpen => {
                // Fall through to the actual call.
            }
        }

        // --- Perform the call ---
        let result = f().await;

        // --- Post-call: update state based on outcome ---
        match &result {
            Ok(_) => {
                let old_failures = self.failure_count.swap(0, Ordering::SeqCst);
                let mut state = self.state.lock().unwrap();
                if *state == CircuitState::HalfOpen {
                    info!(cb = %self.name, "probe succeeded — moving HALF-OPEN → CLOSED");
                } else if old_failures > 0 {
                    info!(cb = %self.name, failures_cleared = old_failures, "call succeeded, failures reset");
                }
                *state = CircuitState::Closed;
            }
            Err(e) => {
                let new_count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                warn!(
                    cb = %self.name,
                    failure_count = new_count,
                    threshold = self.threshold,
                    error = ?e,
                    "call failed"
                );

                let mut state = self.state.lock().unwrap();
                if *state == CircuitState::HalfOpen || new_count >= self.threshold {
                    error!(cb = %self.name, "tripping circuit OPEN");
                    *state = CircuitState::Open;
                    *self.last_failure_time.lock().unwrap() = Some(Instant::now());
                }
            }
        }

        result.map_err(CircuitBreakerError::CallFailed)
    }

    /// Current state — useful for health reporting.
    pub fn state(&self) -> CircuitState {
        self.state.lock().unwrap().clone()
    }
}

// ---------------------------------------------------------------------------
// Simulated unreliable sensor daemon
// ---------------------------------------------------------------------------

/// Simulates a sensor that fails randomly.
///
/// In real life this would be a Unix socket call to the sensor daemon (day 4 pattern).
/// We keep it simple here to focus on the circuit breaker logic.
struct UnreliableSensor {
    fail_count: std::cell::Cell<u32>,
}

#[derive(Debug)]
struct SensorError(String);

impl UnreliableSensor {
    fn new() -> Self {
        Self {
            fail_count: std::cell::Cell::new(0),
        }
    }

    /// Returns Ok(temperature) some of the time, Err on consecutive failures.
    async fn read_temperature(&self) -> Result<f32, SensorError> {
        // Simulate I2C latency.
        sleep(Duration::from_millis(50)).await;

        let n = self.fail_count.get();
        self.fail_count.set(n + 1);

        // Fail for the first 7 calls, then recover.
        if n < 7 {
            Err(SensorError(format!("I2C NAK on attempt {}", n)))
        } else {
            Ok(23.5 + n as f32 * 0.1) // plausible temperature reading
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("=== Day 5: Circuit Breaker Demo ===");

    // threshold=3: trip after 3 consecutive failures.
    // recovery_timeout=1s: probe after 1 second in Open state.
    let cb = CircuitBreaker::new("temperature-sensor", 3, Duration::from_secs(1));
    let sensor = UnreliableSensor::new();

    for i in 0..20 {
        // Space calls out to give the recovery timeout time to tick.
        sleep(Duration::from_millis(300)).await;

        let result = cb.call(|| sensor.read_temperature()).await;
        match result {
            Ok(temp) => info!(attempt = i, temp_c = temp, state = ?cb.state(), "read OK"),
            Err(CircuitBreakerError::CircuitOpen) => {
                warn!(attempt = i, "call rejected by open circuit (fast-fail)");
            }
            Err(CircuitBreakerError::CallFailed(e)) => {
                error!(attempt = i, error = ?e, "call reached sensor but failed");
            }
        }
    }

    info!(final_state = ?cb.state(), "Demo complete");
    info!("Observe: after 3 failures the circuit opened; calls were rejected until the 1s timeout; then one probe succeeded and the circuit closed again.");
}
