//! Tabla de salud simple para el daemon sensor.

use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone, PartialEq)]
pub enum HealthState {
    Nominal,
    Degraded { reason: String },
    Failed { reason: String },
}

impl std::fmt::Display for HealthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nominal => write!(f, "NOMINAL"),
            Self::Degraded { reason } => write!(f, "DEGRADADO({reason})"),
            Self::Failed { reason } => write!(f, "FALLIDO({reason})"),
        }
    }
}

pub struct HealthTable {
    entries: HashMap<String, HealthState>,
}

impl HealthTable {
    pub fn new() -> Self { Self { entries: HashMap::new() } }

    pub fn set(&mut self, component: impl Into<String>, state: HealthState) {
        let id = component.into();
        let prev = self.entries.get(&id).cloned();
        info!(component = id, state = %state, "actualización de salud");
        if prev.as_ref() != Some(&state) {
            // Registrar transición de estado
            match &state {
                HealthState::Degraded { reason } =>
                    tracing::warn!(component = id, reason, "componente degradado"),
                HealthState::Failed { reason } =>
                    tracing::error!(component = id, reason, "componente fallido"),
                HealthState::Nominal => {}
            }
        }
        self.entries.insert(id, state);
    }

    pub fn get(&self, component: &str) -> Option<&HealthState> {
        self.entries.get(component)
    }

    pub fn overall(&self) -> HealthState {
        if self.entries.values().any(|s| matches!(s, HealthState::Failed { .. })) {
            HealthState::Failed { reason: "uno o más componentes han fallado".into() }
        } else if self.entries.values().any(|s| matches!(s, HealthState::Degraded { .. })) {
            HealthState::Degraded { reason: "uno o más componentes degradados".into() }
        } else {
            HealthState::Nominal
        }
    }
}
