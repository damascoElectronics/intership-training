//! Ejemplo 03: Tabla de Salud Compartida
//!
//! La tabla de salud es el sistema nervioso central del FDIR. Cada componente escribe
//! su propia salud; una función de consulta sintetiza una vista a nivel de sistema.
//!
//! Decisiones de diseño:
//! - Arc<RwLock<HealthTable>>: muchos lectores concurrentes (telemetría), escritor exclusivo
//!   por componente. RwLock es mejor que Mutex cuando dominan las lecturas.
//! - Registro de eventos VecDeque: un buffer circular de eventos de salud recientes satisface
//!   el requisito ECSS FDIR-4 (todos los eventos de detección de fallos deben registrarse).
//!   Se limita para evitar crecimiento ilimitado.
//! - Cada entrada almacena `transition_count`: si un componente oscila repetidamente entre
//!   Nominal y Degradado, eso en sí mismo es una firma de fallo digna de detectar.
//!
//! Ejecutar: cargo run --example 03_health_table

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{sync::RwLock, time::sleep};
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Tipos de salud
// ---------------------------------------------------------------------------

/// Estado de salud de un único componente.
///
/// ¿Por qué no un bool? Porque "degradado" es cualitativamente diferente de "fallado":
/// un GPS Degradado sigue dando posición con menor precisión; un GPS Failed no da nada.
/// El sistema puede tomar mejores decisiones con tres estados que con dos.
#[derive(Debug, Clone, PartialEq)]
pub enum HealthState {
    /// Operando dentro de los parámetros normales.
    Nominal,
    /// Capacidad reducida o fallos intermitentes; aún no ha fallado.
    Degraded { reason: String },
    /// Sin salida utilizable; el componente debe ser derivado o reemplazado.
    Failed { reason: String },
}

impl HealthState {
    /// Ordenación por severidad, usada para sintetizar la salud a nivel de sistema.
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

/// Registro de salud de un componente con nombre.
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub id: String,
    pub state: HealthState,
    /// Marca de tiempo monotónica de la última actualización de estado.
    pub last_updated: Instant,
    /// Cuántas veces este componente ha transitado a un estado no Nominal.
    /// Los conteos altos de oscilación son en sí mismos una firma de fallo.
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

    /// Actualizar el estado. Incrementa transition_count si se aleja de Nominal.
    pub fn update(&mut self, new_state: HealthState) {
        if new_state != HealthState::Nominal {
            self.transition_count += 1;
        }
        self.state = new_state;
        self.last_updated = Instant::now();
    }
}

/// Registro con marca de tiempo de un cambio de estado de salud, guardado en el buffer circular.
#[derive(Debug, Clone)]
pub struct HealthEvent {
    pub component_id: String,
    pub old_state: HealthState,
    pub new_state: HealthState,
    pub timestamp: Instant,
}

/// Tabla de salud central: todos los componentes se registran aquí.
pub struct HealthTable {
    entries: HashMap<String, ComponentHealth>,
    /// Buffer circular de transiciones de salud recientes. Satisface ECSS FDIR-4.
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

    /// Registrar un nuevo componente. Debe llamarse antes que `set_health`.
    pub fn register(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.entries
            .entry(id.clone())
            .or_insert_with(|| ComponentHealth::new(id));
    }

    /// Actualizar el estado de salud de un componente, registrando la transición.
    pub fn set_health(&mut self, id: &str, new_state: HealthState) {
        let entry = self.entries.entry(id.to_string()).or_insert_with(|| {
            ComponentHealth::new(id)
        });

        let old_state = entry.state.clone();
        entry.update(new_state.clone());

        // Registrar el evento en el buffer circular.
        if self.event_log.len() >= self.event_log_capacity {
            self.event_log.pop_front(); // descartar el más antiguo
        }
        self.event_log.push_back(HealthEvent {
            component_id: id.to_string(),
            old_state,
            new_state,
            timestamp: Instant::now(),
        });
    }

    /// Salud a nivel de sistema: el peor estado entre todos los componentes.
    ///
    /// Este es el único indicador que vigila el control en tierra. Si no es Nominal,
    /// saben que deben mirar la salud por componente para más detalles.
    pub fn system_health(&self) -> HealthState {
        self.entries
            .values()
            .max_by_key(|c| c.state.severity())
            .map(|c| c.state.clone())
            .unwrap_or(HealthState::Nominal)
    }

    /// Obtener la salud de un componente específico.
    pub fn get(&self, id: &str) -> Option<&ComponentHealth> {
        self.entries.get(id)
    }

    /// Eventos recientes del buffer circular.
    pub fn recent_events(&self, n: usize) -> Vec<&HealthEvent> {
        self.event_log.iter().rev().take(n).collect()
    }

    /// Contar cuántos componentes hay en cada nivel de severidad.
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
// Alias de tipo compartido
// ---------------------------------------------------------------------------

/// El tipo de tabla de salud compartida usado en todo el proyecto.
///
/// Arc: propiedad compartida.
/// RwLock: muchos lectores (descarga de telemetría, consultas de salud) un escritor por actualización.
pub type SharedHealthTable = Arc<RwLock<HealthTable>>;

// ---------------------------------------------------------------------------
// Demo
// ---------------------------------------------------------------------------

/// Simula un componente sensor que actualiza periódicamente su salud.
async fn sensor_component(
    name: String,
    health: SharedHealthTable,
    fail_after: Duration,
) {
    info!(component = %name, "iniciando");

    let start = Instant::now();

    loop {
        sleep(Duration::from_millis(500)).await;

        let elapsed = start.elapsed();

        let new_state = if elapsed < fail_after {
            HealthState::Nominal
        } else if elapsed < fail_after + Duration::from_secs(2) {
            HealthState::Degraded {
                reason: format!("errores de checksum después de {}s", elapsed.as_secs()),
            }
        } else {
            HealthState::Failed {
                reason: "sin respuesta al comando de reinicio".into(),
            }
        };

        {
            // Bloqueo de escritura para la actualización de estado.
            let mut ht = health.write().await;
            ht.set_health(&name, new_state.clone());
        }

        info!(component = %name, state = %new_state, elapsed_ms = elapsed.as_millis(), "salud actualizada");
    }
}

/// Lee e imprime periódicamente la salud general del sistema.
async fn health_reporter(health: SharedHealthTable) {
    loop {
        sleep(Duration::from_secs(1)).await;

        // Bloqueo de lectura — no bloquea a los escritores.
        let ht = health.read().await;
        let system = ht.system_health();
        let (nominal, degraded, failed) = ht.summary();

        warn!(
            estado_sistema = %system,
            componentes_nominales = nominal,
            componentes_degradados = degraded,
            componentes_fallidos = failed,
            "=== INFORME DE SALUD DEL SISTEMA ==="
        );

        // Imprimir los tres eventos más recientes.
        for event in ht.recent_events(3) {
            info!(
                component = %event.component_id,
                anterior = %event.old_state,
                nuevo = %event.new_state,
                "transición reciente"
            );
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("=== Día 5: Demo de Tabla de Salud ===");

    // Crear la tabla compartida con un buffer circular de 50 eventos.
    let health: SharedHealthTable = Arc::new(RwLock::new(HealthTable::new(50)));

    // Registrar todos los componentes de antemano para que el reporter los muestre inmediatamente.
    {
        let mut ht = health.write().await;
        ht.register("temperature-sensor");
        ht.register("pressure-sensor");
        ht.register("gps-receiver");
    }

    // Lanzar componentes: el sensor de temperatura falla rápido, la presión está bien, el GPS se degrada despacio.
    tokio::spawn(sensor_component(
        "temperature-sensor".into(),
        health.clone(),
        Duration::from_secs(2), // falla tras 2s
    ));
    tokio::spawn(sensor_component(
        "pressure-sensor".into(),
        health.clone(),
        Duration::from_secs(30), // efectivamente nunca falla en esta demo
    ));
    tokio::spawn(sensor_component(
        "gps-receiver".into(),
        health.clone(),
        Duration::from_secs(4), // falla tras 4s
    ));

    // Lanzar el reporter de salud.
    tokio::spawn(health_reporter(health.clone()));

    // Ejecutar durante 8 segundos.
    sleep(Duration::from_secs(8)).await;

    // Informe detallado final.
    let ht = health.read().await;
    info!("\n\n--- Informe Final por Componente ---");
    for (id, comp) in &ht.entries {
        info!(
            id = %id,
            state = %comp.state,
            transiciones = comp.transition_count,
            obsoleto_ms = comp.last_updated.elapsed().as_millis(),
        );
    }
}
