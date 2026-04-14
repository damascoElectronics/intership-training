//! Example 01: Software Watchdog
//!
//! A hardware watchdog resets the entire CPU if not kicked. That's great as a last
//! resort, but within a single process we often want finer granularity: detect that
//! *one* task has hung while others keep running, without killing everything.
//!
//! This is a software watchdog: each monitored task holds a WatchdogToken and calls
//! `.kick()` to prove it's alive. A background monitor task checks all tokens and
//! escalates when one expires.
//!
//! Run: cargo run --example 01_watchdog

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::time::sleep;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// WatchdogToken
// ---------------------------------------------------------------------------

/// A token given to each task that wants watchdog supervision.
///
/// The task calls `.kick()` periodically. If it doesn't, the monitor fires.
/// Arc<Mutex<Instant>> lets both the task and the monitor share the timestamp
/// without copying or unsafe code.
#[derive(Clone)]
pub struct WatchdogToken {
    /// Human-readable name for logging.
    pub name: String,
    /// How long the token may go unkicked before it's considered expired.
    pub timeout: Duration,
    /// Protected last-kick timestamp, shared with the monitor.
    last_kick: Arc<Mutex<Instant>>,
}

impl WatchdogToken {
    /// Create a new token. The deadline clock starts immediately.
    pub fn new(name: impl Into<String>, timeout: Duration) -> Self {
        Self {
            name: name.into(),
            timeout,
            last_kick: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Called by the monitored task to prove it's still running.
    ///
    /// In real hardware this would also kick /dev/watchdog or the IWDG register.
    pub fn kick(&self) {
        let mut ts = self.last_kick.lock().unwrap();
        *ts = Instant::now();
        // Trace-level: this fires every loop iteration, too noisy for info.
        tracing::trace!(task = %self.name, "watchdog kicked");
    }

    /// Called by the monitor to check whether this token has expired.
    pub fn is_expired(&self) -> bool {
        let ts = self.last_kick.lock().unwrap();
        ts.elapsed() > self.timeout
    }

    /// How long since the last kick — useful for health reporting.
    pub fn time_since_kick(&self) -> Duration {
        self.last_kick.lock().unwrap().elapsed()
    }
}

// ---------------------------------------------------------------------------
// WatchdogMonitor
// ---------------------------------------------------------------------------

/// Owns all tokens, periodically checks them, calls the handler on expiry.
pub struct WatchdogMonitor {
    tokens: Vec<WatchdogToken>,
    /// How often the monitor wakes up to sweep all tokens.
    check_interval: Duration,
}

impl WatchdogMonitor {
    pub fn new(check_interval: Duration) -> Self {
        Self {
            tokens: Vec::new(),
            check_interval,
        }
    }

    /// Register a new task. Returns the token the task must keep and kick.
    pub fn register(&mut self, name: impl Into<String>, timeout: Duration) -> WatchdogToken {
        let token = WatchdogToken::new(name, timeout);
        self.tokens.push(token.clone());
        token
    }

    /// Run the monitor loop. Call this as a spawned task.
    ///
    /// `on_expiry` is called once per expired token per sweep. In production you'd
    /// wire this to the HealthTable and the supervisor escalation chain.
    pub async fn run<F>(self, mut on_expiry: F)
    where
        F: FnMut(&WatchdogToken),
    {
        // Track which tokens have already been reported as expired so we don't
        // spam the handler every check interval.
        let mut reported: HashMap<String, bool> = self
            .tokens
            .iter()
            .map(|t| (t.name.clone(), false))
            .collect();

        loop {
            sleep(self.check_interval).await;

            for token in &self.tokens {
                if token.is_expired() {
                    if !reported[&token.name] {
                        // First time we see this expiry: fire the callback.
                        on_expiry(token);
                        *reported.get_mut(&token.name).unwrap() = true;
                    }
                } else {
                    // Token recovered (e.g., after a transient stall).
                    if reported[&token.name] {
                        info!(task = %token.name, "watchdog: task recovered (kicked again)");
                    }
                    *reported.get_mut(&token.name).unwrap() = false;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Demo tasks
// ---------------------------------------------------------------------------

/// A well-behaved task that kicks its watchdog every 300 ms.
async fn healthy_task(token: WatchdogToken) {
    info!(task = %token.name, "starting (will kick every 300 ms)");
    loop {
        // Simulate work.
        sleep(Duration::from_millis(300)).await;
        token.kick();
        info!(task = %token.name, "did some work, kicked watchdog");
    }
}

/// A task that works fine for a bit, then "hangs" (stops kicking).
async fn hanging_task(token: WatchdogToken) {
    info!(task = %token.name, "starting (will hang after 1 second)");

    // Normal operation: kick a couple of times.
    for _ in 0..3 {
        sleep(Duration::from_millis(300)).await;
        token.kick();
        info!(task = %token.name, "kicked watchdog (still healthy)");
    }

    // Now we enter a simulated deadlock / infinite blocking call.
    // We never call token.kick() again, so the watchdog will fire.
    warn!(task = %token.name, "entering simulated hang...");
    sleep(Duration::from_secs(60)).await; // In real life: blocking syscall, deadlock, etc.
}

/// A task that kicks but too infrequently (simulates a slow / overloaded task).
async fn slow_task(token: WatchdogToken) {
    info!(task = %token.name, "starting (kicks every 1.5 s, timeout is 1 s)");
    loop {
        // This task is legitimately slow — it does a big computation.
        // But it's so slow that it misses its watchdog window.
        sleep(Duration::from_millis(1500)).await;
        token.kick(); // Too late — the monitor will have fired by now.
        info!(task = %token.name, "finished slow work, kicked watchdog");
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

    info!("=== Day 5: Software Watchdog Demo ===");

    // Build the monitor and register three tasks.
    // Each task gets an individual timeout appropriate for its expected cadence.
    let mut monitor = WatchdogMonitor::new(Duration::from_millis(200));

    let healthy_token = monitor.register("healthy_task", Duration::from_millis(1000));
    let hanging_token = monitor.register("hanging_task", Duration::from_millis(1000));
    // The slow task has a tight timeout so the watchdog fires even though the task
    // *does* eventually kick — it just kicks too rarely.
    let slow_token = monitor.register("slow_task", Duration::from_millis(1000));

    // Spawn the monitored tasks.
    tokio::spawn(healthy_task(healthy_token));
    tokio::spawn(hanging_task(hanging_token));
    tokio::spawn(slow_task(slow_token));

    // Spawn the watchdog monitor. The closure is our FDIR escalation entry point.
    // In a real system this would call into the health table and supervisor.
    tokio::spawn(monitor.run(|token| {
        error!(
            task = %token.name,
            stalled_for_ms = %token.time_since_kick().as_millis(),
            "WATCHDOG EXPIRED — task is not responding!"
        );
        // Next step in a real system:
        //   1. Mark component Failed in HealthTable
        //   2. Notify supervisor to restart the task
        //   3. If it happens too often, escalate to safe mode
    }));

    // Let the demo run for 6 seconds, then stop.
    info!("Running for 6 seconds — watch for watchdog expiry events...");
    sleep(Duration::from_secs(6)).await;

    info!("Demo complete. In a real system we'd signal shutdown here.");
}
