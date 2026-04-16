//! Ejemplo 05: Patrón Typestate para Gestión de Modos
//!
//! El patrón typestate usa el sistema de tipos de Rust para hacer cumplir transiciones de
//! estado válidas en **tiempo de compilación**. La idea clave: en lugar de un único
//! `struct OBC` con un enum `mode: Mode` en tiempo de ejecución, parametrizamos la
//! estructura: `struct OBC<Mode>`.
//!
//! Los métodos que solo son válidos en ciertos modos existen únicamente en el bloque
//! `impl OBC<EseModo>` correspondiente. Llamarlos en el modo equivocado es un **error de
//! compilación**, no un pánico en ejecución. Coste cero en tiempo de ejecución, máxima seguridad.
//!
//! Esto es directamente aplicable al software de OBC de naves espaciales:
//! - No se deben disparar propulsores en SafeMode (riesgo estructural).
//! - No se deben ejecutar instrumentos científicos durante maniobras de orientación (error de apuntamiento).
//! - No se deben aceptar telecomandos durante el modo de emergencia (sin autenticación posible).
//!
//! `PhantomData<Mode>` le dice al compilador que OBC está parametrizado por Mode aunque
//! Mode no aparezca en ningún campo. Tiene tamaño cero en tiempo de ejecución.
//!
//! Ejecutar: cargo run --example 05_safe_state

use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::Arc,
    time::Instant,
};

use tokio::sync::RwLock;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Tabla de salud (mínima, independiente)
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
        info!(component = %id, state = %state, "salud actualizada");
        self.entries.insert(id.to_string(), state);
    }
    pub fn get(&self, id: &str) -> Option<&HealthState> { self.entries.get(id) }
}

pub type SharedHealth = Arc<RwLock<HealthTable>>;

// ---------------------------------------------------------------------------
// Tipos marcadores de modo
// ---------------------------------------------------------------------------

/// Operaciones normales de la nave espacial. Capacidad completa disponible.
pub struct Nominal;

/// Configuración de mínimo riesgo. Propulsores asegurados, no esenciales apagados, solo comunicaciones.
pub struct SafeMode;

/// Modo de último recurso. Calefactores de supervivencia, baliza de emergencia, sin comandos.
pub struct Emergency;

// ---------------------------------------------------------------------------
// Informe de telemetría de mantenimiento (housekeeping)
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
// Tipo OBC — parametrizado por modo
// ---------------------------------------------------------------------------

/// Computadora de a Bordo (OBC), con cumplimiento de modo en tiempo de compilación.
///
/// `_mode: PhantomData<Mode>` es un campo de tamaño cero que transporta el tipo de modo
/// a través del sistema de tipos sin ocupar ninguna memoria.
pub struct OBC<Mode> {
    _mode: PhantomData<Mode>,
    pub health: SharedHealth,
}

// ---------------------------------------------------------------------------
// Modo Nominal — capacidades completas
// ---------------------------------------------------------------------------

impl OBC<Nominal> {
    /// Crear el OBC en modo Nominal (estado inicial tras el arranque).
    pub fn new(health: SharedHealth) -> Self {
        info!("OBC: inicializando en modo Nominal");
        Self {
            _mode: PhantomData,
            health,
        }
    }

    /// Recopilar telemetría de mantenimiento — solo tiene sentido en operación normal.
    pub fn collect_housekeeping(&self) -> HkReport {
        info!("OBC[Nominal]: recopilando telemetría de mantenimiento");
        HkReport {
            timestamp: Instant::now(),
            cpu_percent: 12.5,
            memory_kb: 8192,
            temperature_mc: 22_000, // 22.000 °C
        }
    }

    /// Ejecutar una maniobra orbital — SOLO disponible en modo Nominal.
    ///
    /// En SafeMode o Emergency, este método no existe en el tipo.
    /// El compilador rechazará cualquier intento de llamarlo.
    pub fn execute_orbit_manoeuvre(&self, delta_v_ms: f64) -> Result<(), String> {
        info!(delta_v = delta_v_ms, "OBC[Nominal]: ejecutando maniobra orbital");
        if delta_v_ms.abs() > 100.0 {
            return Err(format!("delta-v {delta_v_ms} m/s excede el límite de quema única"));
        }
        Ok(())
    }

    /// Transmitir datos científicos — solo en Nominal.
    pub fn downlink_science_data(&self, payload_id: &str) -> Result<(), String> {
        info!(payload = %payload_id, "OBC[Nominal]: transmitiendo datos científicos");
        Ok(())
    }

    /// Transición a SafeMode. Consume self, devolviendo OBC<SafeMode>.
    ///
    /// Consumir self es crítico: no se puede usar el OBC en Nominal después de esto.
    /// No hay "operar accidentalmente en Nominal mientras se cree que se está en seguro".
    pub fn enter_safe_mode(self, reason: &str) -> OBC<SafeMode> {
        warn!(reason = %reason, "OBC[Nominal]: entrando en SafeMode");
        // En hardware real: deshabilitar cargas no esenciales, asegurar propulsores, etc.
        OBC {
            _mode: PhantomData,
            health: self.health,
        }
    }
}

// ---------------------------------------------------------------------------
// SafeMode — capacidades restringidas
// ---------------------------------------------------------------------------

impl OBC<SafeMode> {
    /// Ejecutar autodiagnósticos — tiene sentido en modo seguro para encontrar el fallo.
    pub fn run_diagnostics(&self) -> DiagResult {
        info!("OBC[SafeMode]: ejecutando diagnósticos");
        // Simular: verificar memoria, verificar conectividad de sensores, verificar presupuesto de energía.
        DiagResult {
            passed: true,
            details: "memoria OK, sensores respondiendo, energía nominal".into(),
        }
    }

    /// Transmitir una baliza de modo seguro para que tierra sepa que seguimos activos.
    pub fn transmit_beacon(&self) {
        info!("OBC[SafeMode]: transmitiendo baliza de modo seguro en frecuencia de emergencia");
    }

    /// Intentar recuperación al modo Nominal.
    ///
    /// Devuelve Ok(OBC<Nominal>) si los diagnósticos pasan, Err(OBC<SafeMode>) si no.
    /// El caso Err significa que permanecemos en SafeMode — el typestate garantiza que no
    /// podemos usar accidentalmente la interfaz Nominal tras una recuperación fallida.
    pub fn recover_to_nominal(self) -> Result<OBC<Nominal>, OBC<SafeMode>> {
        let diag = self.run_diagnostics();
        if diag.passed {
            info!("OBC[SafeMode]: diagnósticos pasados — recuperando a Nominal");
            Ok(OBC {
                _mode: PhantomData,
                health: self.health,
            })
        } else {
            warn!(details = %diag.details, "OBC[SafeMode]: diagnósticos fallidos — permaneciendo en SafeMode");
            Err(OBC {
                _mode: PhantomData,
                health: self.health,
            })
        }
    }

    /// Escalar a modo Emergency — usado cuando no se puede mantener el modo seguro.
    pub fn escalate_to_emergency(self) -> OBC<Emergency> {
        error!("OBC[SafeMode]: escalando a modo EMERGENCY");
        OBC {
            _mode: PhantomData,
            health: self.health,
        }
    }
}

// ---------------------------------------------------------------------------
// Modo Emergency — solo supervivencia
// ---------------------------------------------------------------------------

impl OBC<Emergency> {
    /// Activar calefactores de supervivencia (proteger baterías de daños térmicos).
    pub fn activate_survival_heaters(&self) {
        error!("OBC[Emergency]: activando calefactores de supervivencia");
    }

    /// Transmitir baliza de emergencia.
    pub fn transmit_emergency_beacon(&self) {
        error!("OBC[Emergency]: transmitiendo baliza de EMERGENCIA");
    }
    // NOTA: Sin propulsores, sin ciencia, sin maniobras disponibles aquí.
}

// ---------------------------------------------------------------------------
// Demostración de seguridad en tiempo de compilación
// ---------------------------------------------------------------------------

/// Esta función acepta solo OBC<SafeMode>. No puede disparar propulsores accidentalmente.
///
/// Si intentas llamar a `obc.execute_orbit_manoeuvre(...)` aquí, el compilador dice:
///   "no method named `execute_orbit_manoeuvre` found for type `OBC<SafeMode>`"
/// No hay comprobación en ejecución, no hay pánico a las 2am, no hay informe de anomalía después.
#[allow(dead_code)]
fn safe_mode_operations(obc: &OBC<SafeMode>) {
    obc.transmit_beacon();
    let diag = obc.run_diagnostics();
    info!(passed = diag.passed, details = %diag.details, "resultado de diagnósticos");

    // Descomentar la siguiente línea es un ERROR DE COMPILACIÓN — execute_orbit_manoeuvre
    // no existe en OBC<SafeMode>. Este es el objetivo del patrón typestate.
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

    info!("=== Día 5: Demo de Gestión de Modos con Typestate ===");

    let health: SharedHealth = Arc::new(RwLock::new(HealthTable::new()));

    // --- Operaciones normales ---
    let obc = OBC::<Nominal>::new(health.clone());

    let hk = obc.collect_housekeeping();
    info!(cpu = hk.cpu_percent, temp_mc = hk.temperature_mc, "telemetría de mantenimiento recopilada");

    obc.execute_orbit_manoeuvre(5.0).expect("maniobra fallida");
    obc.downlink_science_data("LIDAR_SCAN_42").expect("transmisión fallida");

    // --- Fallo detectado: entrar en modo seguro ---
    // `obc` se mueve aquí. El OBC en Nominal ya no existe después de esta línea.
    let safe_obc = obc.enter_safe_mode("rastreador de estrellas NAK en I2C tras 3 reintentos");

    // Ya no podemos hacer esto — `obc` ha sido consumido:
    // obc.execute_orbit_manoeuvre(1.0);  // error[E0382]: uso de valor movido: `obc`

    // --- Operaciones en modo seguro ---
    safe_mode_operations(&safe_obc);
    safe_obc.transmit_beacon();

    // --- Intentar recuperación ---
    match safe_obc.recover_to_nominal() {
        Ok(nominal_obc) => {
            info!("¡Recuperación exitosa! De vuelta en modo Nominal.");
            // Estamos de vuelta — podemos hacer cosas de Nominal de nuevo.
            nominal_obc.collect_housekeeping();
            // Y no podemos hacer cosas de SafeMode:
            // nominal_obc.run_diagnostics();  // error[E0599]: no method found
        }
        Err(still_safe) => {
            warn!("Recuperación fallida. Permaneciendo en SafeMode.");
            // Escalar.
            let emergency_obc = still_safe.escalate_to_emergency();
            emergency_obc.activate_survival_heaters();
            emergency_obc.transmit_emergency_beacon();
        }
    }

    info!("Demo completada. Punto clave: el compilador hizo cumplir las restricciones de modo — no se necesitan comprobaciones en tiempo de ejecución.");
}
