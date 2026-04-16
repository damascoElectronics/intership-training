//! Ejemplo 04: Patrón de Tarea Supervisora
//!
//! El modelo de actores de Erlang se construye alrededor del "dejar que se caiga": no manejar
//! defensivamente cada error posible dentro de un proceso; en cambio, dejar que se caiga y
//! confiar en que el supervisor lo reinicie. Esto mantiene la lógica de negocio limpia y
//! separa la recuperación de fallos en una capa dedicada.
//!
//! Adaptamos esto para Rust async:
//! - Las tareas son baratas (hilos verdes de Tokio).
//! - Si una tarea devuelve Err o entra en pánico a través de un `JoinHandle`, el supervisor lo captura.
//! - El supervisor reinicia con retroceso exponencial para evitar martillar un recurso roto
//!   en un bucle cerrado (protección contra crash-loop).
//! - Tras `max_restarts` intentos, el supervisor se rinde y marca el componente como
//!   Failed en la HealthTable — un humano o FDIR de mayor nivel debe intervenir.
//!
//! Ejecutar: cargo run --example 04_supervisor

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use tokio::{
    sync::RwLock,
    task::JoinHandle,
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Tipos compartidos (abreviados de 03_health_table.rs para compilación independiente)
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
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn set(&mut self, id: &str, state: HealthState) {
        info!(component = %id, state = %state, "salud actualizada");
        self.entries.insert(id.to_string(), state);
    }
}

pub type SharedHealth = Arc<RwLock<HealthTable>>;

// ---------------------------------------------------------------------------
// Tipo de error de tarea
// ---------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum TaskError {
    #[error("error transitorio: {0}")]
    Transient(String),
    #[error("error fatal: {0}")]
    Fatal(String),
}

// ---------------------------------------------------------------------------
// TaskSpec
// ---------------------------------------------------------------------------

/// Describe una tarea supervisada.
///
/// La fábrica es un `Box<dyn Fn() -> BoxFuture>` para que el supervisor pueda crear
/// una instancia nueva de tarea para cada reinicio. Un `Arc` capturado permite que la
/// fábrica comparta estado entre reinicios (p. ej., la ruta del socket, la configuración,
/// la referencia a la tabla de salud).
type BoxFuture = Pin<Box<dyn Future<Output = Result<(), TaskError>> + Send>>;

pub struct TaskSpec {
    pub name: String,
    /// Crea una nueva instancia de la tarea. Se llama en el inicio inicial y en cada reinicio.
    pub factory: Box<dyn Fn() -> BoxFuture + Send + Sync>,
    /// Cuántas veces reiniciar antes de rendirse.
    pub max_restarts: u32,
    /// Retardo base para el retroceso exponencial (milisegundos).
    /// Retardo = base * 2^intento, limitado a 30 segundos.
    pub restart_delay_base_ms: u64,
}

impl TaskSpec {
    pub fn restart_delay(&self, attempt: u32) -> Duration {
        // Retroceso exponencial: 100ms, 200ms, 400ms, 800ms... limitado a 30s.
        let ms = self.restart_delay_base_ms.saturating_mul(1u64 << attempt.min(8));
        Duration::from_millis(ms.min(30_000))
    }
}

// ---------------------------------------------------------------------------
// Supervisor
// ---------------------------------------------------------------------------

/// Ejecutar todas las tareas, reiniciando en caso de fallo con retroceso exponencial.
///
/// El apagado se coordina mediante un `CancellationToken`: cuando se cancela, el supervisor
/// deja de reiniciar las tareas fallidas y espera a que las tareas en ejecución terminen.
pub async fn supervisor(tasks: Vec<TaskSpec>, health: SharedHealth, shutdown: CancellationToken) {
    // Rastrear el estado por tarea: (JoinHandle, restart_count, name).
    // Mantenemos un Vec<Option<JoinHandle>> indexado igual que `tasks`.
    let mut handles: Vec<Option<JoinHandle<Result<(), TaskError>>>> =
        tasks.iter().map(|_| None).collect();
    let mut restart_counts: Vec<u32> = vec![0; tasks.len()];
    let mut failed: Vec<bool> = vec![false; tasks.len()];

    // Lanzamiento inicial de todas las tareas.
    for (i, spec) in tasks.iter().enumerate() {
        info!(task = %spec.name, "supervisor: lanzamiento inicial");
        handles[i] = Some(tokio::spawn((spec.factory)()));
    }

    // Bucle principal del supervisor.
    //
    // `tokio::select!` sobre apagado O sobre cualquier tarea que termine.
    // Dado que JoinHandle no es Clone, los sondeamos manualmente en cada iteración.
    loop {
        // Comprobar si se solicitó el apagado.
        if shutdown.is_cancelled() {
            info!("supervisor: apagado solicitado, deteniendo bucle de reinicio");
            break;
        }

        // Sondear cada handle para ver si alguna tarea terminó.
        // Usamos un sleep corto para que no sea un bucle ocupado.
        sleep(Duration::from_millis(50)).await;

        for i in 0..tasks.len() {
            if failed[i] {
                continue; // ya nos rendimos con esta tarea
            }

            let finished = if let Some(handle) = &handles[i] {
                handle.is_finished()
            } else {
                false
            };

            if !finished {
                continue;
            }

            // La tarea terminó — recoger el resultado.
            let result = handles[i].take().unwrap().await;
            let spec = &tasks[i];

            match result {
                Ok(Ok(())) => {
                    // Salida limpia. La tarea terminó normalmente (inusual para un demonio).
                    info!(task = %spec.name, "la tarea salió limpiamente");
                    // No reiniciar — ha terminado.
                }
                Ok(Err(e)) => {
                    // La tarea devolvió Err.
                    warn!(
                        task = %spec.name,
                        error = %e,
                        reinicio_count = restart_counts[i],
                        max = spec.max_restarts,
                        "la tarea falló"
                    );
                    handle_restart(
                        i,
                        spec,
                        &mut handles,
                        &mut restart_counts,
                        &mut failed,
                        &health,
                        shutdown.clone(),
                    )
                    .await;
                }
                Err(join_err) => {
                    // Pánico o cancelación dentro de la tarea.
                    error!(
                        task = %spec.name,
                        error = %join_err,
                        "la tarea entró en pánico o fue cancelada"
                    );
                    handle_restart(
                        i,
                        spec,
                        &mut handles,
                        &mut restart_counts,
                        &mut failed,
                        &health,
                        shutdown.clone(),
                    )
                    .await;
                }
            }
        }

        // Si todas las tareas han salido limpiamente o fallado permanentemente, hemos terminado.
        let all_done = handles.iter().all(|h| h.is_none());
        if all_done {
            info!("supervisor: todas las tareas terminaron");
            break;
        }
    }

    info!("supervisor: saliendo");
}

/// Manejar un único fallo de tarea: reiniciar con retroceso o marcar como fallado permanentemente.
async fn handle_restart(
    idx: usize,
    spec: &TaskSpec,
    handles: &mut Vec<Option<JoinHandle<Result<(), TaskError>>>>,
    restart_counts: &mut Vec<u32>,
    failed: &mut Vec<bool>,
    health: &SharedHealth,
    shutdown: CancellationToken,
) {
    if shutdown.is_cancelled() {
        return;
    }

    let count = restart_counts[idx];

    if count >= spec.max_restarts {
        error!(
            task = %spec.name,
            intentos = count,
            "reinicios máximos excedidos — marcando como FALLADO y abandonando"
        );
        failed[idx] = true;
        health.write().await.set(
            &spec.name,
            HealthState::Failed {
                reason: format!("se cayó {} veces, max_restarts={}", count, spec.max_restarts),
            },
        );
        return;
    }

    let delay = spec.restart_delay(count);
    warn!(
        task = %spec.name,
        intento = count + 1,
        delay_ms = delay.as_millis(),
        "reiniciando con retroceso"
    );

    // Marcar como Degradado durante la ventana de reinicio.
    health.write().await.set(
        &spec.name,
        HealthState::Degraded {
            reason: format!("reiniciando (intento {})", count + 1),
        },
    );

    sleep(delay).await;

    if shutdown.is_cancelled() {
        return;
    }

    restart_counts[idx] += 1;
    handles[idx] = Some(tokio::spawn((spec.factory)()));

    // Marcar como Nominal de forma optimista; la tarea actualizará la salud si falla de nuevo.
    health.write().await.set(&spec.name, HealthState::Nominal);
    info!(task = %spec.name, "reiniciada");
}

// ---------------------------------------------------------------------------
// Tareas de demostración
// ---------------------------------------------------------------------------

fn make_healthy_task() -> BoxFuture {
    Box::pin(async move {
        info!("healthy_task: ejecutándose indefinidamente");
        loop {
            sleep(Duration::from_millis(500)).await;
            info!("healthy_task: tick");
        }
        // Esto nunca retorna, simulando un demonio de larga ejecución.
        #[allow(unreachable_code)]
        Ok(())
    })
}

fn make_flaky_task(name: String, shared_counter: Arc<std::sync::Mutex<u32>>) -> BoxFuture {
    Box::pin(async move {
        let count = {
            let mut c = shared_counter.lock().unwrap();
            *c += 1;
            *c
        };

        info!(name = %name, instance = count, "flaky_task iniciando");

        // Trabajar normalmente un momento, luego fallar.
        sleep(Duration::from_millis(400)).await;

        if count <= 3 {
            warn!(name = %name, instance = count, "flaky_task: ¡caída simulada!");
            Err(TaskError::Transient(format!(
                "error de hardware aleatorio en instancia {}",
                count
            )))
        } else {
            // Finalmente se estabiliza tras suficientes reinicios.
            info!(name = %name, instance = count, "flaky_task: ahora estable, ejecutándose indefinidamente");
            loop {
                sleep(Duration::from_secs(1)).await;
                info!(name = %name, instance = count, "flaky_task: tick (estable)");
            }
            #[allow(unreachable_code)]
            Ok(())
        }
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("=== Día 5: Demo de Supervisor ===");

    let health: SharedHealth = Arc::new(RwLock::new(HealthTable::new()));
    let shutdown = CancellationToken::new();

    // Contador compartido para rastrear instancias de reinicio entre llamadas a la fábrica.
    let flaky_counter = Arc::new(std::sync::Mutex::new(0u32));

    let tasks = vec![
        TaskSpec {
            name: "healthy-daemon".into(),
            factory: Box::new(make_healthy_task),
            max_restarts: 5,
            restart_delay_base_ms: 100,
        },
        {
            let counter = flaky_counter.clone();
            TaskSpec {
                name: "flaky-sensor".into(),
                factory: Box::new(move || make_flaky_task("flaky-sensor".into(), counter.clone())),
                max_restarts: 5,
                restart_delay_base_ms: 200,
            }
        },
    ];

    let health_clone = health.clone();
    let shutdown_clone = shutdown.clone();
    let sup_handle = tokio::spawn(supervisor(tasks, health_clone, shutdown_clone));

    // Dejar correr el supervisor durante 8 segundos, luego apagar.
    sleep(Duration::from_secs(8)).await;
    info!("main: solicitando apagado");
    shutdown.cancel();
    let _ = sup_handle.await;

    // Imprimir salud final.
    let ht = health.read().await;
    info!("--- Salud Final ---");
    for (id, state) in &ht.entries {
        info!(component = %id, state = %state);
    }
}
