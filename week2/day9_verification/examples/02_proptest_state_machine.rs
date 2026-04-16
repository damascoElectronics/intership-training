//! Ejemplo 02 — Pruebas basadas en propiedades para una máquina de estados
//!
//! La máquina de estados de salud FDIR del Día 5 debe satisfacer invariantes
//! bajo CUALQUIER secuencia de eventos. proptest genera miles de secuencias
//! aleatorias de eventos y verifica que los invariantes siempre se cumplan.
//!
//! Ejecutar con:  cargo test --example 02_proptest_state_machine

use proptest::prelude::*;

// ── Máquina de estados de salud ───────────────────────────────────────────────────

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
            // Todas las demás transiciones no tienen efecto
            (s, _) => s.clone(),
        }
    }

    /// Invariante del sistema: estas combinaciones de estados nunca deben ocurrir.
    pub fn is_valid(&self) -> bool {
        // Todas las variantes son válidas; esto es trivialmente verdadero aquí, pero en un sistema
        // real con estado compuesto se verificarían combinaciones imposibles.
        true
    }
}

// ── Estrategia proptest para HealthEvent ─────────────────────────────────────────

fn any_health_event() -> impl Strategy<Value = HealthEvent> {
    prop_oneof![
        Just(HealthEvent::FaultDetected),
        Just(HealthEvent::FaultCleared),
        Just(HealthEvent::CriticalFault),
        Just(HealthEvent::ManualReset),
        Just(HealthEvent::EscalateToSafeMode),
    ]
}

// ── Propiedades ─────────────────────────────────────────────────────────────────

proptest! {
    /// Tras cualquier secuencia de eventos, el estado de salud siempre es válido.
    #[test]
    fn state_invariants_hold(events in proptest::collection::vec(any_health_event(), 0..50)) {
        let mut state = HealthState::Nominal;
        for event in &events {
            state = state.apply(event);
            prop_assert!(state.is_valid(), "estado inválido alcanzado: {state:?}");
        }
    }

    /// Un estado Failed solo puede pasar a Nominal mediante ManualReset (posiblemente a través de SafeMode).
    #[test]
    fn failed_requires_manual_reset(
        pre_events in proptest::collection::vec(any_health_event(), 0..20),
        post_events in proptest::collection::vec(any_health_event(), 1..20),
    ) {
        // Llegar al estado Failed
        let mut state = HealthState::Nominal;
        for e in &pre_events { state = state.apply(e); }

        if state != HealthState::Failed { return Ok(()); }

        // Aplicar eventos posteriores SIN ManualReset
        let no_reset: Vec<_> = post_events.iter()
            .filter(|e| !matches!(e, HealthEvent::ManualReset))
            .collect();

        for e in &no_reset { state = state.apply(e); }

        // El estado NO debe ser Nominal (no hubo restablecimiento)
        prop_assert_ne!(state, HealthState::Nominal,
            "se alcanzó Nominal desde Failed sin ManualReset");
    }

    /// Tras ManualReset desde SafeMode, el sistema vuelve a Nominal.
    #[test]
    fn safemode_reset_returns_to_nominal(_ignored: u8) {
        let state = HealthState::SafeMode;
        let next = state.apply(&HealthEvent::ManualReset);
        assert_eq!(next, HealthState::Nominal);
    }
}

fn main() {
    println!("Ejecutar con: cargo test --example 02_proptest_state_machine");
}
