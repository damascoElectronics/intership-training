//! Component health state and system-level health aggregation.
//!
//! Derived from Day 5 (FDIR) patterns.  Each daemon reports its own
//! [`HealthState`] and the supervisor can call [`SystemHealth::aggregate`]
//! to roll up a single system-level verdict.

use serde::{Deserialize, Serialize};

/// A typed identifier for an OBC software component.
///
/// Using a newtype rather than a bare `String` makes accidental comparisons
/// between unrelated identifiers a compile-time error.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComponentId(pub String);

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<S: Into<String>> From<S> for ComponentId {
    fn from(s: S) -> Self {
        ComponentId(s.into())
    }
}

/// Health state of a single component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthState {
    /// All parameters within normal operating limits.
    Nominal,
    /// Component is functional but operating outside nominal limits.
    Degraded {
        /// Human-readable description of the anomaly.
        reason: String,
    },
    /// Component has failed and is not providing service.
    Failed {
        /// Human-readable description of the failure.
        reason: String,
    },
}

impl HealthState {
    /// Returns `true` for [`HealthState::Nominal`].
    pub fn is_nominal(&self) -> bool {
        matches!(self, HealthState::Nominal)
    }

    /// Returns `true` for [`HealthState::Failed`].
    pub fn is_failed(&self) -> bool {
        matches!(self, HealthState::Failed { .. })
    }

    /// Severity as a number (0 = nominal, 1 = degraded, 2 = failed).
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
            HealthState::Nominal => write!(f, "NOMINAL"),
            HealthState::Degraded { reason } => write!(f, "DEGRADED({})", reason),
            HealthState::Failed { reason } => write!(f, "FAILED({})", reason),
        }
    }
}

/// Aggregated health report for the full OBC software stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    /// Per-component health states.
    pub components: Vec<(ComponentId, HealthState)>,
    /// System-level roll-up (worst-case of all components).
    pub overall: HealthState,
}

impl SystemHealth {
    /// Create a new [`SystemHealth`] by aggregating the provided component states.
    ///
    /// The overall state is the worst-case across all components:
    /// * Any `Failed` → `Failed`
    /// * Any `Degraded` → `Degraded`
    /// * All `Nominal` → `Nominal`
    pub fn new(components: Vec<(ComponentId, HealthState)>) -> Self {
        let overall = Self::aggregate(&components);
        Self { components, overall }
    }

    /// Compute the worst-case [`HealthState`] across a slice of component states.
    pub fn aggregate(components: &[(ComponentId, HealthState)]) -> HealthState {
        let mut worst = HealthState::Nominal;
        for (id, state) in components {
            if state.severity() > worst.severity() {
                worst = match state {
                    HealthState::Failed { reason } => HealthState::Failed {
                        reason: format!("{}: {}", id, reason),
                    },
                    HealthState::Degraded { reason } => HealthState::Degraded {
                        reason: format!("{}: {}", id, reason),
                    },
                    HealthState::Nominal => HealthState::Nominal,
                };
            }
        }
        worst
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_all_nominal() {
        let comps = vec![
            (ComponentId("a".into()), HealthState::Nominal),
            (ComponentId("b".into()), HealthState::Nominal),
        ];
        assert_eq!(SystemHealth::aggregate(&comps), HealthState::Nominal);
    }

    #[test]
    fn aggregate_picks_worst() {
        let comps = vec![
            (ComponentId("a".into()), HealthState::Nominal),
            (
                ComponentId("b".into()),
                HealthState::Degraded {
                    reason: "high temp".into(),
                },
            ),
            (ComponentId("c".into()), HealthState::Nominal),
        ];
        assert!(matches!(
            SystemHealth::aggregate(&comps),
            HealthState::Degraded { .. }
        ));
    }

    #[test]
    fn aggregate_failed_dominates() {
        let comps = vec![
            (
                ComponentId("a".into()),
                HealthState::Degraded {
                    reason: "x".into(),
                },
            ),
            (
                ComponentId("b".into()),
                HealthState::Failed {
                    reason: "crash".into(),
                },
            ),
        ];
        assert!(matches!(
            SystemHealth::aggregate(&comps),
            HealthState::Failed { .. }
        ));
    }
}
