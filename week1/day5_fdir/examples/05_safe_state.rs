//! Example 05: Typestate Pattern for Mode Management
//!
//! The typestate pattern uses Rust's type system to enforce valid state transitions
//! at **compile time**. The key insight: instead of a single `struct OBC` with a
//! runtime `mode: Mode` enum, we parameterise the struct: `struct OBC<Mode>`.
//!
//! Methods that are only legal in certain modes exist only on the corresponding
//! `impl OBC<ThatMode>` block. Calling them in the wrong mode is a **compile error**,
//! not a runtime panic. Zero runtime cost, maximum safety.
//!
//! This is directly applicable to spacecraft OBC software:
//! - You must not fire thrusters in SafeMode (structural risk).
//! - You must not run science instruments during slew manoeuvres (pointing error).
//! - You must not accept telecommands during emergency mode (no auth possible).
//!
//! `PhantomData<Mode>` tells the compiler that OBC is parameterised by Mode even
//! though Mode doesn't appear in any field. It has zero size at runtime.
//!
//! Run: cargo run --example 05_safe_state

use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::Arc,
    time::Instant,
};

use tokio::sync::RwLock;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Health table (minimal, standalone)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum HealthState {
    Nominal,
    Degraded { reason: String },
    Failed { reason: String },
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

pub struct HealthTable {
    entries: HashMap<String, HealthState>,
}

impl HealthTable {
    pub fn new() -> Self { Self { entries: HashMap::new() } }
    pub fn set(&mut self, id: &str, state: HealthState) {
        info!(component = %id, state = %state, "health updated");
        self.entries.insert(id.to_string(), state);
    }
    pub fn get(&self, id: &str) -> Option<&HealthState> { self.entries.get(id) }
}

pub type SharedHealth = Arc<RwLock<HealthTable>>;

// ---------------------------------------------------------------------------
// Mode marker types
// ---------------------------------------------------------------------------

/// Normal spacecraft operations. Full capability available.
pub struct Nominal;

/// Minimal-risk configuration. Thrusters safed, non-essentials off, comm only.
pub struct SafeMode;

/// Last-resort mode. Survival heaters, emergency beacon, no commands.
pub struct Emergency;

// ---------------------------------------------------------------------------
// Housekeeping report
// ---------------------------------------------------------------------------

pub struct HkReport {
    pub timestamp: Instant,
    pub cpu_percent: f32,
    pub memory_kb: u32,
    pub temperature_mc: i32,
}

pub struct DiagResult {
    pub passed: bool,
    pub details: String,
}

// ---------------------------------------------------------------------------
// OBC type — parameterised by mode
// ---------------------------------------------------------------------------

/// On-Board Computer, with compile-time mode enforcement.
///
/// `_mode: PhantomData<Mode>` is a zero-size field that carries the mode type
/// through the type system without occupying any memory.
pub struct OBC<Mode> {
    _mode: PhantomData<Mode>,
    pub health: SharedHealth,
}

// ---------------------------------------------------------------------------
// Nominal mode — full capabilities
// ---------------------------------------------------------------------------

impl OBC<Nominal> {
    /// Create the OBC in Nominal mode (initial state after boot).
    pub fn new(health: SharedHealth) -> Self {
        info!("OBC: initialising in Nominal mode");
        Self {
            _mode: PhantomData,
            health,
        }
    }

    /// Collect housekeeping telemetry — only meaningful in normal ops.
    pub fn collect_housekeeping(&self) -> HkReport {
        info!("OBC[Nominal]: collecting housekeeping");
        HkReport {
            timestamp: Instant::now(),
            cpu_percent: 12.5,
            memory_kb: 8192,
            temperature_mc: 22_000, // 22.000 °C
        }
    }

    /// Execute an orbit manoeuvre — ONLY available in Nominal mode.
    ///
    /// In SafeMode or Emergency, this method does not exist on the type.
    /// The compiler will reject any attempt to call it.
    pub fn execute_orbit_manoeuvre(&self, delta_v_ms: f64) -> Result<(), String> {
        info!(delta_v = delta_v_ms, "OBC[Nominal]: executing orbit manoeuvre");
        if delta_v_ms.abs() > 100.0 {
            return Err(format!("delta-v {delta_v_ms} m/s exceeds single-burn limit"));
        }
        Ok(())
    }

    /// Downlink science data — only in Nominal.
    pub fn downlink_science_data(&self, payload_id: &str) -> Result<(), String> {
        info!(payload = %payload_id, "OBC[Nominal]: downlinking science data");
        Ok(())
    }

    /// Transition to SafeMode. Consumes self, returning OBC<SafeMode>.
    ///
    /// Consuming self is critical: you cannot use the Nominal OBC after this.
    /// There is no "accidentally operating in Nominal while thinking you're safe".
    pub fn enter_safe_mode(self, reason: &str) -> OBC<SafeMode> {
        warn!(reason = %reason, "OBC[Nominal]: entering SafeMode");
        // In real hardware: disable non-essential loads, safe thrusters, etc.
        OBC {
            _mode: PhantomData,
            health: self.health,
        }
    }
}

// ---------------------------------------------------------------------------
// SafeMode — restricted capabilities
// ---------------------------------------------------------------------------

impl OBC<SafeMode> {
    /// Run self-diagnostics — makes sense in safe mode to find the fault.
    pub fn run_diagnostics(&self) -> DiagResult {
        info!("OBC[SafeMode]: running diagnostics");
        // Simulate: check memory, check sensor connectivity, check power budget.
        DiagResult {
            passed: true,
            details: "memory OK, sensors responding, power nominal".into(),
        }
    }

    /// Transmit a safe-mode beacon so ground knows we're alive.
    pub fn transmit_beacon(&self) {
        info!("OBC[SafeMode]: transmitting safe-mode beacon on emergency frequency");
    }

    /// Attempt recovery to Nominal mode.
    ///
    /// Returns Ok(OBC<Nominal>) if diagnostics pass, Err(OBC<SafeMode>) if not.
    /// The Err case means we stay in SafeMode — typestate ensures we can't
    /// accidentally use the Nominal interface after a failed recovery.
    pub fn recover_to_nominal(self) -> Result<OBC<Nominal>, OBC<SafeMode>> {
        let diag = self.run_diagnostics();
        if diag.passed {
            info!("OBC[SafeMode]: diagnostics passed — recovering to Nominal");
            Ok(OBC {
                _mode: PhantomData,
                health: self.health,
            })
        } else {
            warn!(details = %diag.details, "OBC[SafeMode]: diagnostics failed — staying in SafeMode");
            Err(OBC {
                _mode: PhantomData,
                health: self.health,
            })
        }
    }

    /// Escalate to Emergency mode — used when safe mode can't be maintained.
    pub fn escalate_to_emergency(self) -> OBC<Emergency> {
        error!("OBC[SafeMode]: escalating to EMERGENCY mode");
        OBC {
            _mode: PhantomData,
            health: self.health,
        }
    }
}

// ---------------------------------------------------------------------------
// Emergency mode — survival only
// ---------------------------------------------------------------------------

impl OBC<Emergency> {
    /// Activate survival heaters (protect batteries from thermal damage).
    pub fn activate_survival_heaters(&self) {
        error!("OBC[Emergency]: activating survival heaters");
    }

    /// Transmit emergency beacon.
    pub fn transmit_emergency_beacon(&self) {
        error!("OBC[Emergency]: transmitting EMERGENCY beacon");
    }
    // NOTE: No thrusters, no science, no manoeuvres available here.
}

// ---------------------------------------------------------------------------
// Compile-time safety demonstration
// ---------------------------------------------------------------------------

/// This function accepts only OBC<SafeMode>. It cannot accidentally fire thrusters.
///
/// If you try to call `obc.execute_orbit_manoeuvre(...)` here, the compiler says:
///   "no method named `execute_orbit_manoeuvre` found for type `OBC<SafeMode>`"
/// There's no runtime check, no panic at 2am, no anomaly report after the fact.
#[allow(dead_code)]
fn safe_mode_operations(obc: &OBC<SafeMode>) {
    obc.transmit_beacon();
    let diag = obc.run_diagnostics();
    info!(passed = diag.passed, details = %diag.details, "diagnostics result");

    // Uncommenting the next line is a COMPILE ERROR — orbit_manoeuvre doesn't
    // exist on OBC<SafeMode>. This is the whole point of the typestate pattern.
    // obc.execute_orbit_manoeuvre(5.0);  // error[E0599]: no method named ...
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("=== Day 5: Typestate Mode Management Demo ===");

    let health: SharedHealth = Arc::new(RwLock::new(HealthTable::new()));

    // --- Normal operations ---
    let obc = OBC::<Nominal>::new(health.clone());

    let hk = obc.collect_housekeeping();
    info!(cpu = hk.cpu_percent, temp_mc = hk.temperature_mc, "housekeeping collected");

    obc.execute_orbit_manoeuvre(5.0).expect("manoeuvre failed");
    obc.downlink_science_data("LIDAR_SCAN_42").expect("downlink failed");

    // --- Fault detected: enter safe mode ---
    // `obc` is moved here. The Nominal OBC no longer exists after this line.
    let safe_obc = obc.enter_safe_mode("star tracker NAK on I2C after 3 retries");

    // We can no longer do this — `obc` is consumed:
    // obc.execute_orbit_manoeuvre(1.0);  // error[E0382]: use of moved value: `obc`

    // --- Safe mode operations ---
    safe_mode_operations(&safe_obc);
    safe_obc.transmit_beacon();

    // --- Attempt recovery ---
    match safe_obc.recover_to_nominal() {
        Ok(nominal_obc) => {
            info!("Recovery successful! Back in Nominal mode.");
            // We're back — can do Nominal things again.
            nominal_obc.collect_housekeeping();
            // And we cannot do SafeMode things:
            // nominal_obc.run_diagnostics();  // error[E0599]: no method found
        }
        Err(still_safe) => {
            warn!("Recovery failed. Staying in SafeMode.");
            // Escalate.
            let emergency_obc = still_safe.escalate_to_emergency();
            emergency_obc.activate_survival_heaters();
            emergency_obc.transmit_emergency_beacon();
        }
    }

    info!("Demo complete. Key point: the compiler enforced mode constraints — no runtime checks needed.");
}
