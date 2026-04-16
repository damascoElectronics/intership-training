//! Estado de salud de componentes y agregación de salud a nivel de sistema.
//!
//! Derivado de los patrones de FDIR del Día 5. Cada daemon reporta su propio
//! [`HealthState`] y el supervisor puede llamar a [`SystemHealth::aggregate`]
//! para obtener un veredicto único a nivel de sistema.

use serde::{Deserialize, Serialize};

/// Un identificador tipado para un componente de software OBC.
///
/// Usar un newtipo en lugar de un `String` simple convierte las comparaciones accidentales
/// entre identificadores no relacionados en un error de tiempo de compilación.
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

/// Estado de salud de un único componente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthState {
    /// Todos los parámetros dentro de los límites de operación normales.
    Nominal,
    /// El componente es funcional pero opera fuera de los límites nominales.
    Degraded {
        /// Descripción legible de la anomalía.
        reason: String,
    },
    /// El componente ha fallado y no está proporcionando servicio.
    Failed {
        /// Descripción legible del fallo.
        reason: String,
    },
}

impl HealthState {
    /// Devuelve `true` para [`HealthState::Nominal`].
    pub fn is_nominal(&self) -> bool {
        matches!(self, HealthState::Nominal)
    }

    /// Devuelve `true` para [`HealthState::Failed`].
    pub fn is_failed(&self) -> bool {
        matches!(self, HealthState::Failed { .. })
    }

    /// Severidad como número (0 = nominal, 1 = degradado, 2 = fallido).
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

/// Informe de salud agregado para toda la pila de software OBC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    /// Estados de salud por componente.
    pub components: Vec<(ComponentId, HealthState)>,
    /// Resumen a nivel de sistema (peor caso de todos los componentes).
    pub overall: HealthState,
}

impl SystemHealth {
    /// Crea un nuevo [`SystemHealth`] agregando los estados de componentes proporcionados.
    ///
    /// El estado general es el peor caso entre todos los componentes:
    /// * Cualquier `Failed` → `Failed`
    /// * Cualquier `Degraded` → `Degraded`
    /// * Todos `Nominal` → `Nominal`
    pub fn new(components: Vec<(ComponentId, HealthState)>) -> Self {
        let overall = Self::aggregate(&components);
        Self { components, overall }
    }

    /// Calcula el [`HealthState`] de peor caso en un slice de estados de componentes.
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
                    reason: "temperatura alta".into(),
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
                    reason: "fallo crítico".into(),
                },
            ),
        ];
        assert!(matches!(
            SystemHealth::aggregate(&comps),
            HealthState::Failed { .. }
        ));
    }
}
