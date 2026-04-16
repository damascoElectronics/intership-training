//! Ejemplo 01: Watchdog por Software
//!
//! Un watchdog por hardware reinicia toda la CPU si no se le "patea". Eso es genial
//! como último recurso, pero dentro de un único proceso a menudo queremos una
//! granularidad más fina: detectar que *una* tarea se ha bloqueado mientras las demás
//! siguen funcionando, sin matar todo el proceso.
//!
//! Este es un watchdog por software: cada tarea monitorizada tiene un WatchdogToken y
//! llama a `.kick()` para demostrar que está viva. Una tarea monitora en segundo plano
//! verifica todos los tokens y escala cuando uno expira.
//!
//! Ejecutar: cargo run --example 01_watchdog

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::time::sleep;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// WatchdogToken
// ---------------------------------------------------------------------------

/// Token entregado a cada tarea que desea supervisión watchdog.
///
/// La tarea llama a `.kick()` periódicamente. Si no lo hace, el monitor se activa.
/// Arc<Mutex<Instant>> permite que tanto la tarea como el monitor compartan la marca
/// de tiempo sin copiar ni usar código inseguro.
#[derive(Clone)]
pub struct WatchdogToken {
    /// Nombre legible para el registro de logs.
    pub name: String,
    /// Tiempo máximo que el token puede estar sin ser pateado antes de considerarse expirado.
    pub timeout: Duration,
    /// Marca de tiempo del último kick, compartida con el monitor.
    last_kick: Arc<Mutex<Instant>>,
}

impl WatchdogToken {
    /// Crear un nuevo token. El reloj de deadline comienza inmediatamente.
    pub fn new(name: impl Into<String>, timeout: Duration) -> Self {
        Self {
            name: name.into(),
            timeout,
            last_kick: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Llamado por la tarea monitorizada para demostrar que sigue en ejecución.
    ///
    /// En hardware real esto también patearía /dev/watchdog o el registro IWDG.
    pub fn kick(&self) {
        let mut ts = self.last_kick.lock().unwrap();
        *ts = Instant::now();
        // Nivel trace: se dispara en cada iteración del bucle, demasiado ruidoso para info.
        tracing::trace!(task = %self.name, "watchdog pateado");
    }

    /// Llamado por el monitor para comprobar si este token ha expirado.
    pub fn is_expired(&self) -> bool {
        let ts = self.last_kick.lock().unwrap();
        ts.elapsed() > self.timeout
    }

    /// Tiempo transcurrido desde el último kick — útil para informes de salud.
    pub fn time_since_kick(&self) -> Duration {
        self.last_kick.lock().unwrap().elapsed()
    }
}

// ---------------------------------------------------------------------------
// WatchdogMonitor
// ---------------------------------------------------------------------------

/// Posee todos los tokens, los verifica periódicamente y llama al manejador al expirar.
pub struct WatchdogMonitor {
    tokens: Vec<WatchdogToken>,
    /// Con qué frecuencia se despierta el monitor para revisar todos los tokens.
    check_interval: Duration,
}

impl WatchdogMonitor {
    pub fn new(check_interval: Duration) -> Self {
        Self {
            tokens: Vec::new(),
            check_interval,
        }
    }

    /// Registrar una nueva tarea. Devuelve el token que la tarea debe conservar y patear.
    pub fn register(&mut self, name: impl Into<String>, timeout: Duration) -> WatchdogToken {
        let token = WatchdogToken::new(name, timeout);
        self.tokens.push(token.clone());
        token
    }

    /// Ejecutar el bucle del monitor. Llamar esto como una tarea lanzada con spawn.
    ///
    /// `on_expiry` se llama una vez por token expirado por barrido. En producción esto
    /// se conectaría a la HealthTable y a la cadena de escalada del supervisor.
    pub async fn run<F>(self, mut on_expiry: F)
    where
        F: FnMut(&WatchdogToken),
    {
        // Rastrear qué tokens ya han sido reportados como expirados para no
        // inundar el manejador en cada intervalo de comprobación.
        let mut reported: HashMap<String, bool> = self
            .tokens
            .iter()
            .map(|t| (t.name.clone(), false))
            .collect();

        loop {
            sleep(self.check_interval).await;

            for token in &self.tokens {
                if token.is_expired() {
                    if !reported[&token.name] {
                        // Primera vez que vemos esta expiración: disparar el callback.
                        on_expiry(token);
                        *reported.get_mut(&token.name).unwrap() = true;
                    }
                } else {
                    // Token recuperado (p. ej., tras un bloqueo transitorio).
                    if reported[&token.name] {
                        info!(task = %token.name, "watchdog: tarea recuperada (pateó de nuevo)");
                    }
                    *reported.get_mut(&token.name).unwrap() = false;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tareas de demostración
// ---------------------------------------------------------------------------

/// Una tarea bien comportada que patea su watchdog cada 300 ms.
async fn healthy_task(token: WatchdogToken) {
    info!(task = %token.name, "iniciando (pateará cada 300 ms)");
    loop {
        // Simular trabajo.
        sleep(Duration::from_millis(300)).await;
        token.kick();
        info!(task = %token.name, "hizo algo de trabajo, pateó el watchdog");
    }
}

/// Una tarea que funciona bien un tiempo y luego se "bloquea" (deja de patear).
async fn hanging_task(token: WatchdogToken) {
    info!(task = %token.name, "iniciando (se bloqueará después de 1 segundo)");

    // Operación normal: patear un par de veces.
    for _ in 0..3 {
        sleep(Duration::from_millis(300)).await;
        token.kick();
        info!(task = %token.name, "pateó el watchdog (sigue saludable)");
    }

    // Ahora entramos en un deadlock simulado / llamada bloqueante infinita.
    // Nunca llamamos a token.kick() de nuevo, así que el watchdog se activará.
    warn!(task = %token.name, "entrando en bloqueo simulado...");
    sleep(Duration::from_secs(60)).await; // En la vida real: syscall bloqueante, deadlock, etc.
}

/// Una tarea que patea pero con demasiada poca frecuencia (simula una tarea lenta / sobrecargada).
async fn slow_task(token: WatchdogToken) {
    info!(task = %token.name, "iniciando (patea cada 1,5 s, el timeout es 1 s)");
    loop {
        // Esta tarea es legítimamente lenta — realiza un gran cómputo.
        // Pero es tan lenta que pierde su ventana de watchdog.
        sleep(Duration::from_millis(1500)).await;
        token.kick(); // Demasiado tarde — el monitor ya habrá disparado.
        info!(task = %token.name, "terminó el trabajo lento, pateó el watchdog");
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("=== Día 5: Demo de Watchdog por Software ===");

    // Construir el monitor y registrar tres tareas.
    // Cada tarea recibe un timeout individual apropiado para su cadencia esperada.
    let mut monitor = WatchdogMonitor::new(Duration::from_millis(200));

    let healthy_token = monitor.register("healthy_task", Duration::from_millis(1000));
    let hanging_token = monitor.register("hanging_task", Duration::from_millis(1000));
    // La tarea lenta tiene un timeout ajustado para que el watchdog se active aunque la tarea
    // *sí* patee eventualmente — simplemente lo hace con demasiada poca frecuencia.
    let slow_token = monitor.register("slow_task", Duration::from_millis(1000));

    // Lanzar las tareas monitorizadas.
    tokio::spawn(healthy_task(healthy_token));
    tokio::spawn(hanging_task(hanging_token));
    tokio::spawn(slow_task(slow_token));

    // Lanzar el monitor watchdog. El closure es nuestro punto de entrada de escalada FDIR.
    // En un sistema real esto llamaría a la tabla de salud y al supervisor.
    tokio::spawn(monitor.run(|token| {
        error!(
            task = %token.name,
            parado_por_ms = %token.time_since_kick().as_millis(),
            "WATCHDOG EXPIRADO — ¡la tarea no responde!"
        );
        // Siguiente paso en un sistema real:
        //   1. Marcar el componente como Failed en la HealthTable
        //   2. Notificar al supervisor para reiniciar la tarea
        //   3. Si ocurre con demasiada frecuencia, escalar a modo seguro
    }));

    // Dejar correr la demo durante 6 segundos, luego detener.
    info!("Ejecutando durante 6 segundos — observar los eventos de expiración del watchdog...");
    sleep(Duration::from_secs(6)).await;

    info!("Demo completada. En un sistema real se señalizaría el apagado aquí.");
}
