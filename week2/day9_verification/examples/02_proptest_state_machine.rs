//! Example 02 — Property-based testing of a state machine
//!
//! The FDIR health state machine from Day 5 must satisfy invariants
//! under ANY sequence of events.  proptest generates thousands of random
//! event sequences and checks that the invariants always hold.
//!
//! Run with:  cargo test --example 02_proptest_state_machine

use proptest::prelude::*;

// ── Health state machine ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum HealthState {
    Nominal,
    Degraded,
    Failed,
    SafeMode,
}

#[derive(Debug, Clone)]
pub enum HealthEvent {
    FaultDetected,
    FaultCleared,
    CriticalFault,
    ManualReset,
    EscalateToSafeMode,
}

impl HealthState {
    pub fn apply(&self, event: &HealthEvent) -> Self {
        match (self, event) {
            (Self::Nominal,   HealthEvent::FaultDetected)     => Self::Degraded,
            (Self::Nominal,   HealthEvent::CriticalFault)     => Self::Failed,
            (Self::Degraded,  HealthEvent::FaultCleared)      => Self::Nominal,
            (Self::Degraded,  HealthEvent::CriticalFault)     => Self::Failed,
            (Self::Degraded,  HealthEvent::EscalateToSafeMode)=> Self::SafeMode,
            (Self::Failed,    HealthEvent::EscalateToSafeMode)=> Self::SafeMode,
            (Self::Failed,    HealthEvent::ManualReset)       => Self::Nominal,
            (Self::SafeMode,  HealthEvent::ManualReset)       => Self::Nominal,
            // All other transitions are no-ops
            (s, _) => s.clone(),
        }
    }

    /// System invariant: these state combinations must never occur.
    pub fn is_valid(&self) -> bool {
        // All variants are valid; this is trivially true here, but in a real
        // system with compound state you'd check for impossible combos.
        true
    }
}

// ── Proptest strategy for HealthEvent ─────────────────────────────────────────

fn any_health_event() -> impl Strategy<Value = HealthEvent> {
    prop_oneof![
        Just(HealthEvent::FaultDetected),
        Just(HealthEvent::FaultCleared),
        Just(HealthEvent::CriticalFault),
        Just(HealthEvent::ManualReset),
        Just(HealthEvent::EscalateToSafeMode),
    ]
}

// ── Properties ─────────────────────────────────────────────────────────────────

proptest! {
    /// After any sequence of events, the health state is always valid.
    #[test]
    fn state_invariants_hold(events in proptest::collection::vec(any_health_event(), 0..50)) {
        let mut state = HealthState::Nominal;
        for event in &events {
            state = state.apply(event);
            prop_assert!(state.is_valid(), "invalid state reached: {state:?}");
        }
    }

    /// A Failed state can only become Nominal via ManualReset (possibly through SafeMode).
    #[test]
    fn failed_requires_manual_reset(
        pre_events in proptest::collection::vec(any_health_event(), 0..20),
        post_events in proptest::collection::vec(any_health_event(), 1..20),
    ) {
        // Get to Failed state
        let mut state = HealthState::Nominal;
        for e in &pre_events { state = state.apply(e); }

        if state != HealthState::Failed { return Ok(()); }

        // Apply post events WITHOUT ManualReset
        let no_reset: Vec<_> = post_events.iter()
            .filter(|e| !matches!(e, HealthEvent::ManualReset))
            .collect();

        for e in &no_reset { state = state.apply(e); }

        // State should NOT be Nominal (no reset happened)
        prop_assert_ne!(state, HealthState::Nominal,
            "reached Nominal from Failed without ManualReset");
    }

    /// After ManualReset from SafeMode, system returns to Nominal.
    #[test]
    fn safemode_reset_returns_to_nominal(_ignored: u8) {
        let state = HealthState::SafeMode;
        let next = state.apply(&HealthEvent::ManualReset);
        assert_eq!(next, HealthState::Nominal);
    }
}

fn main() {
    println!("Run with: cargo test --example 02_proptest_state_machine");
}
