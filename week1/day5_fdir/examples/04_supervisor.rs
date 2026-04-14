//! Example 04: Supervisor Task Pattern
//!
//! Erlang's actor model is built around "let it crash": don't defensively handle
//! every possible error inside a process; instead, let it crash and rely on the
//! supervisor to restart it. This keeps business logic clean and separates fault
//! recovery into a dedicated layer.
//!
//! We adapt this for Rust async:
//! - Tasks are cheap (Tokio green threads).
//! - If a task returns Err or panics via `JoinHandle`, the supervisor catches it.
//! - The supervisor restarts with exponential backoff to avoid hammering a broken
//!   resource in a tight loop (crash-loop protection).
//! - After `max_restarts` attempts, the supervisor gives up and marks the component
//!   Failed in the HealthTable — a human or higher-level FDIR must intervene.
//!
//! Run: cargo run --example 04_supervisor

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
// Shared types (abbreviated from 03_health_table.rs for standalone compilation)
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
        info!(component = %id, state = %state, "health updated");
        self.entries.insert(id.to_string(), state);
    }
}

pub type SharedHealth = Arc<RwLock<HealthTable>>;

// ---------------------------------------------------------------------------
// Task error type
// ---------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum TaskError {
    #[error("transient error: {0}")]
    Transient(String),
    #[error("fatal error: {0}")]
    Fatal(String),
}

// ---------------------------------------------------------------------------
// TaskSpec
// ---------------------------------------------------------------------------

/// Describes a supervised task.
///
/// The factory is a `Box<dyn Fn() -> BoxFuture>` so the supervisor can create
/// a fresh task instance for each restart. A captured `Arc` lets the factory
/// share state across restarts (e.g., the socket path, config, health table ref).
type BoxFuture = Pin<Box<dyn Future<Output = Result<(), TaskError>> + Send>>;

pub struct TaskSpec {
    pub name: String,
    /// Creates a new instance of the task. Called on initial start and each restart.
    pub factory: Box<dyn Fn() -> BoxFuture + Send + Sync>,
    /// How many times to restart before giving up.
    pub max_restarts: u32,
    /// Base delay for exponential backoff (milliseconds).
    /// Delay = base * 2^attempt, capped at 30 seconds.
    pub restart_delay_base_ms: u64,
}

impl TaskSpec {
    pub fn restart_delay(&self, attempt: u32) -> Duration {
        // Exponential backoff: 100ms, 200ms, 400ms, 800ms... capped at 30s.
        let ms = self.restart_delay_base_ms.saturating_mul(1u64 << attempt.min(8));
        Duration::from_millis(ms.min(30_000))
    }
}

// ---------------------------------------------------------------------------
// Supervisor
// ---------------------------------------------------------------------------

/// Run all tasks, restarting on failure with exponential backoff.
///
/// Shutdown is coordinated via a `CancellationToken`: when cancelled, the supervisor
/// stops restarting failed tasks and waits for running tasks to complete.
pub async fn supervisor(tasks: Vec<TaskSpec>, health: SharedHealth, shutdown: CancellationToken) {
    // Track per-task state: (JoinHandle, restart_count, name).
    // We keep a Vec<Option<JoinHandle>> indexed the same as `tasks`.
    let mut handles: Vec<Option<JoinHandle<Result<(), TaskError>>>> =
        tasks.iter().map(|_| None).collect();
    let mut restart_counts: Vec<u32> = vec![0; tasks.len()];
    let mut failed: Vec<bool> = vec![false; tasks.len()];

    // Initial spawn of all tasks.
    for (i, spec) in tasks.iter().enumerate() {
        info!(task = %spec.name, "supervisor: initial spawn");
        handles[i] = Some(tokio::spawn((spec.factory)()));
    }

    // Main supervisor loop.
    //
    // `tokio::select!` on shutdown OR on any task completing.
    // Because JoinHandle is not Clone, we poll them manually each iteration.
    loop {
        // Check if shutdown was requested.
        if shutdown.is_cancelled() {
            info!("supervisor: shutdown requested, stopping restart loop");
            break;
        }

        // Poll each handle to see if any task finished.
        // We use a short sleep so this isn't a busy loop.
        sleep(Duration::from_millis(50)).await;

        for i in 0..tasks.len() {
            if failed[i] {
                continue; // already gave up on this task
            }

            let finished = if let Some(handle) = &handles[i] {
                handle.is_finished()
            } else {
                false
            };

            if !finished {
                continue;
            }

            // Task finished — collect result.
            let result = handles[i].take().unwrap().await;
            let spec = &tasks[i];

            match result {
                Ok(Ok(())) => {
                    // Clean exit. The task completed normally (unusual for a daemon).
                    info!(task = %spec.name, "task exited cleanly");
                    // Don't restart — it's done.
                }
                Ok(Err(e)) => {
                    // Task returned Err.
                    warn!(
                        task = %spec.name,
                        error = %e,
                        restart_count = restart_counts[i],
                        max = spec.max_restarts,
                        "task failed"
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
                    // Panic or cancellation inside the task.
                    error!(
                        task = %spec.name,
                        error = %join_err,
                        "task panicked or was cancelled"
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

        // If all tasks have either exited cleanly or permanently failed, we're done.
        let all_done = handles.iter().all(|h| h.is_none());
        if all_done {
            info!("supervisor: all tasks finished");
            break;
        }
    }

    info!("supervisor: exiting");
}

/// Handle a single task failure: restart with backoff or mark as permanently failed.
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
            attempts = count,
            "max restarts exceeded — marking FAILED and giving up"
        );
        failed[idx] = true;
        health.write().await.set(
            &spec.name,
            HealthState::Failed {
                reason: format!("crashed {} times, max_restarts={}", count, spec.max_restarts),
            },
        );
        return;
    }

    let delay = spec.restart_delay(count);
    warn!(
        task = %spec.name,
        attempt = count + 1,
        delay_ms = delay.as_millis(),
        "restarting with backoff"
    );

    // Mark as Degraded during the restart window.
    health.write().await.set(
        &spec.name,
        HealthState::Degraded {
            reason: format!("restarting (attempt {})", count + 1),
        },
    );

    sleep(delay).await;

    if shutdown.is_cancelled() {
        return;
    }

    restart_counts[idx] += 1;
    handles[idx] = Some(tokio::spawn((spec.factory)()));

    // Mark as Nominal optimistically; task will update health if it fails again.
    health.write().await.set(&spec.name, HealthState::Nominal);
    info!(task = %spec.name, "restarted");
}

// ---------------------------------------------------------------------------
// Demo tasks
// ---------------------------------------------------------------------------

fn make_healthy_task() -> BoxFuture {
    Box::pin(async move {
        info!("healthy_task: running forever");
        loop {
            sleep(Duration::from_millis(500)).await;
            info!("healthy_task: tick");
        }
        // This never returns, simulating a long-running daemon.
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

        info!(name = %name, instance = count, "flaky_task starting");

        // Work normally for a bit, then fail.
        sleep(Duration::from_millis(400)).await;

        if count <= 3 {
            warn!(name = %name, instance = count, "flaky_task: simulated crash!");
            Err(TaskError::Transient(format!(
                "random hardware error on instance {}",
                count
            )))
        } else {
            // Eventually stabilises after enough restarts.
            info!(name = %name, instance = count, "flaky_task: now stable, running forever");
            loop {
                sleep(Duration::from_secs(1)).await;
                info!(name = %name, instance = count, "flaky_task: tick (stable)");
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

    info!("=== Day 5: Supervisor Demo ===");

    let health: SharedHealth = Arc::new(RwLock::new(HealthTable::new()));
    let shutdown = CancellationToken::new();

    // Shared counter to track restart instances across factory calls.
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

    // Let the supervisor run for 8 seconds, then shut down.
    sleep(Duration::from_secs(8)).await;
    info!("main: requesting shutdown");
    shutdown.cancel();
    let _ = sup_handle.await;

    // Print final health.
    let ht = health.read().await;
    info!("--- Final Health ---");
    for (id, state) in &ht.entries {
        info!(component = %id, state = %state);
    }
}
