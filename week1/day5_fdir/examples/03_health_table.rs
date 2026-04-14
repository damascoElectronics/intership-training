//! Example 03: Shared Health Table
//!
//! The health table is the central nervous system of FDIR. Every component writes
//! its own health; a query function synthesises a system-level view.
//!
//! Design decisions:
//! - Arc<RwLock<HealthTable>>: many concurrent readers (telemetry), exclusive writer
//!   per component. RwLock is better than Mutex when reads dominate.
//! - VecDeque event log: a ring buffer of recent health events satisfies ECSS FDIR-4
//!   (all fault detection events shall be logged). We cap it to avoid unbounded growth.
//! - Each entry stores `transition_count`: if a component flaps repeatedly between
//!   Nominal and Degraded, that itself is a fault signature worth detecting.
//!
//! Run: cargo run --example 03_health_table

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{sync::RwLock, time::sleep};
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Health types
// ---------------------------------------------------------------------------

/// Health state of a single component.
///
/// Why not a bool? Because "degraded" is qualitatively different from "failed":
/// a Degraded GPS still gives lower-accuracy position; a Failed GPS gives nothing.
/// The system can make better decisions with three states than two.
#[derive(Debug, Clone, PartialEq)]
pub enum HealthState {
    /// Operating within normal parameters.
    Nominal,
    /// Reduced capability or intermittent faults; not yet failed.
    Degraded { reason: String },
    /// No usable output; component must be bypassed or replaced.
    Failed { reason: String },
}

impl HealthState {
    /// Severity ordering, used to synthesise system-level health.
    pub fn severity(&self) -> u8 {
        match self {
            HealthState::Nominal => 0,
            HealthState::Degraded { .. } => 1,
            HealthState::Failed { .. } => 2,
        }
    }
}

impl std::fmt::Display for HealthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthState::Nominal => write!(f, "Nominal"),
            HealthState::Degraded { reason } => write!(f, "Degraded({reason})"),
            HealthState::Failed { reason } => write!(f, "Failed({reason})"),
        }
    }
}

/// Health record for one named component.
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub id: String,
    pub state: HealthState,
    /// Monotonic timestamp of the last state update.
    pub last_updated: Instant,
    /// How many times this component has transitioned to a non-Nominal state.
    /// High flap counts are themselves a fault signature.
    pub transition_count: u32,
}

impl ComponentHealth {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state: HealthState::Nominal,
            last_updated: Instant::now(),
            transition_count: 0,
        }
    }

    /// Update the state. Increments transition_count if moving away from Nominal.
    pub fn update(&mut self, new_state: HealthState) {
        if new_state != HealthState::Nominal {
            self.transition_count += 1;
        }
        self.state = new_state;
        self.last_updated = Instant::now();
    }
}

/// A timestamped record of a health state change, kept in the ring buffer.
#[derive(Debug, Clone)]
pub struct HealthEvent {
    pub component_id: String,
    pub old_state: HealthState,
    pub new_state: HealthState,
    pub timestamp: Instant,
}

/// Central health table: all components register here.
pub struct HealthTable {
    entries: HashMap<String, ComponentHealth>,
    /// Ring buffer of recent health transitions. Satisfies ECSS FDIR-4.
    event_log: VecDeque<HealthEvent>,
    event_log_capacity: usize,
}

impl HealthTable {
    pub fn new(event_log_capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            event_log: VecDeque::with_capacity(event_log_capacity),
            event_log_capacity,
        }
    }

    /// Register a new component. Must be called before `set_health`.
    pub fn register(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.entries
            .entry(id.clone())
            .or_insert_with(|| ComponentHealth::new(id));
    }

    /// Update a component's health state, logging the transition.
    pub fn set_health(&mut self, id: &str, new_state: HealthState) {
        let entry = self.entries.entry(id.to_string()).or_insert_with(|| {
            ComponentHealth::new(id)
        });

        let old_state = entry.state.clone();
        entry.update(new_state.clone());

        // Record the event in the ring buffer.
        if self.event_log.len() >= self.event_log_capacity {
            self.event_log.pop_front(); // discard oldest
        }
        self.event_log.push_back(HealthEvent {
            component_id: id.to_string(),
            old_state,
            new_state,
            timestamp: Instant::now(),
        });
    }

    /// System-level health: the worst state across all components.
    ///
    /// This is the single bit that ground control watches. If it's non-Nominal,
    /// they know to look at per-component health for details.
    pub fn system_health(&self) -> HealthState {
        self.entries
            .values()
            .max_by_key(|c| c.state.severity())
            .map(|c| c.state.clone())
            .unwrap_or(HealthState::Nominal)
    }

    /// Get health for a specific component.
    pub fn get(&self, id: &str) -> Option<&ComponentHealth> {
        self.entries.get(id)
    }

    /// Recent events from the ring buffer.
    pub fn recent_events(&self, n: usize) -> Vec<&HealthEvent> {
        self.event_log.iter().rev().take(n).collect()
    }

    /// Count how many components are in each severity level.
    pub fn summary(&self) -> (usize, usize, usize) {
        let nominal = self.entries.values().filter(|c| c.state == HealthState::Nominal).count();
        let degraded = self
            .entries
            .values()
            .filter(|c| matches!(c.state, HealthState::Degraded { .. }))
            .count();
        let failed = self
            .entries
            .values()
            .filter(|c| matches!(c.state, HealthState::Failed { .. }))
            .count();
        (nominal, degraded, failed)
    }
}

// ---------------------------------------------------------------------------
// Shared type alias
// ---------------------------------------------------------------------------

/// The shared health table type used throughout the project.
///
/// Arc: shared ownership.
/// RwLock: many readers (telemetry downlink, health queries) one writer per update.
pub type SharedHealthTable = Arc<RwLock<HealthTable>>;

// ---------------------------------------------------------------------------
// Demo
// ---------------------------------------------------------------------------

/// Simulates a sensor component periodically updating its health.
async fn sensor_component(
    name: String,
    health: SharedHealthTable,
    fail_after: Duration,
) {
    info!(component = %name, "starting");

    let start = Instant::now();

    loop {
        sleep(Duration::from_millis(500)).await;

        let elapsed = start.elapsed();

        let new_state = if elapsed < fail_after {
            HealthState::Nominal
        } else if elapsed < fail_after + Duration::from_secs(2) {
            HealthState::Degraded {
                reason: format!("checksum errors after {}s", elapsed.as_secs()),
            }
        } else {
            HealthState::Failed {
                reason: "no response to reset command".into(),
            }
        };

        {
            // Write lock for the state update.
            let mut ht = health.write().await;
            ht.set_health(&name, new_state.clone());
        }

        info!(component = %name, state = %new_state, elapsed_ms = elapsed.as_millis(), "health updated");
    }
}

/// Periodically reads and prints the overall system health.
async fn health_reporter(health: SharedHealthTable) {
    loop {
        sleep(Duration::from_secs(1)).await;

        // Read lock — does not block writers.
        let ht = health.read().await;
        let system = ht.system_health();
        let (nominal, degraded, failed) = ht.summary();

        warn!(
            system_state = %system,
            nominal_components = nominal,
            degraded_components = degraded,
            failed_components = failed,
            "=== SYSTEM HEALTH REPORT ==="
        );

        // Print the three most recent events.
        for event in ht.recent_events(3) {
            info!(
                component = %event.component_id,
                old = %event.old_state,
                new = %event.new_state,
                "recent transition"
            );
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("=== Day 5: Health Table Demo ===");

    // Create the shared table with a 50-event ring buffer.
    let health: SharedHealthTable = Arc::new(RwLock::new(HealthTable::new(50)));

    // Register all components up-front so the reporter shows them immediately.
    {
        let mut ht = health.write().await;
        ht.register("temperature-sensor");
        ht.register("pressure-sensor");
        ht.register("gps-receiver");
    }

    // Spawn components: temperature sensor fails fast, pressure is fine, GPS degrades slowly.
    tokio::spawn(sensor_component(
        "temperature-sensor".into(),
        health.clone(),
        Duration::from_secs(2), // fails after 2s
    ));
    tokio::spawn(sensor_component(
        "pressure-sensor".into(),
        health.clone(),
        Duration::from_secs(30), // effectively never fails in this demo
    ));
    tokio::spawn(sensor_component(
        "gps-receiver".into(),
        health.clone(),
        Duration::from_secs(4), // fails after 4s
    ));

    // Spawn the health reporter.
    tokio::spawn(health_reporter(health.clone()));

    // Run for 8 seconds.
    sleep(Duration::from_secs(8)).await;

    // Final detailed report.
    let ht = health.read().await;
    info!("\n\n--- Final Per-Component Report ---");
    for (id, comp) in &ht.entries {
        info!(
            id = %id,
            state = %comp.state,
            transitions = comp.transition_count,
            stale_ms = comp.last_updated.elapsed().as_millis(),
        );
    }
}
