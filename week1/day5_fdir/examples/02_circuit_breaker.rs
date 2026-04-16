//! Ejemplo 02: Disyuntor (Circuit Breaker)
//!
//! Problema: un componente remoto (demonio de sensor, dispositivo I2C, servicio de red) está
//! fallando. Reintentar ingenuamente martillea el componente fallido y desperdicia CPU. Peor
//! aún, si cada llamada se bloquea durante un timeout antes de fallar, un bucle de llamadas
//! saturado puede privar de recursos a otras tareas.
//!
//! El patrón disyuntor — tomado de la ingeniería eléctrica y popularizado por Michael Nygard
//! ("Release It!") — resuelve esto:
//!   CERRADO  → operación normal, los fallos incrementan el contador
//!   ABIERTO  → fallo rápido: ni siquiera intentar, devolver error inmediatamente
//!   SEMI-ABIERTO → sondeo: dejar pasar una llamada para ver si se produjo la recuperación
//!
//! Esto es especialmente útil en demonios embebidos que hablan con hardware por I2C/SPI/UART,
//! donde un bus bloqueado puede bloquear una lectura entera durante segundos.
//!
//! Ejecutar: cargo run --example 02_circuit_breaker

use std::{
    future::Future,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use tokio::time::sleep;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Máquina de estados
// ---------------------------------------------------------------------------

/// Los tres estados del disyuntor.
///
/// ¿Por qué un enum en lugar de structs separados? Aquí necesitamos transiciones de estado
/// en tiempo de ejecución impulsadas por eventos externos, por lo que un enum sencillo es
/// más limpio. Comparar con el patrón typestate en 05_safe_state.rs donde las transiciones
/// son en tiempo de compilación.
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Operación normal. Las llamadas se reenvían. Los fallos incrementan el contador.
    Closed,
    /// El circuito ha disparado. Las llamadas fallan inmediatamente sin llegar al componente.
    /// Tras `recovery_timeout`, el disyuntor sondea moviéndose a HalfOpen.
    Open,
    /// Se permite pasar una llamada de sondeo.
    /// Éxito → Closed (reiniciar contador). Fallo → de vuelta a Open.
    HalfOpen,
}

// ---------------------------------------------------------------------------
// CircuitBreaker
// ---------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum CircuitBreakerError<E: std::fmt::Debug> {
    /// La llamada subyacente devolvió un error.
    #[error("llamada fallida: {0:?}")]
    CallFailed(E),
    /// El circuito está abierto; la llamada fue rechazada sin intentarlo.
    #[error("circuito abierto — fallo rápido")]
    CircuitOpen,
}

/// Un disyuntor que envuelve llamadas asíncronas a un recurso potencialmente fallido.
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failure_count: Arc<AtomicU32>,
    /// Cuántos fallos consecutivos antes de disparar a Open.
    threshold: u32,
    /// Cuánto tiempo permanecer en Open antes de sondear.
    recovery_timeout: Duration,
    /// ¿Cuándo fue la última transición a Open? Se usa para comprobar si el timeout transcurrió.
    last_failure_time: Arc<Mutex<Option<Instant>>>,
    /// Nombre, para el registro de logs.
    name: String,
}

impl CircuitBreaker {
    pub fn new(name: impl Into<String>, threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed)),
            failure_count: Arc::new(AtomicU32::new(0)),
            threshold,
            recovery_timeout,
            last_failure_time: Arc::new(Mutex::new(None)),
            name: name.into(),
        }
    }

    /// Envolver una llamada asíncrona con la lógica del disyuntor.
    ///
    /// `f` produce un Future que representa un intento de llamar al recurso.
    /// Devuelve `Ok(T)` en caso de éxito, o un `CircuitBreakerError` en fallo/disparo.
    pub async fn call<F, Fut, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        // --- Pre-llamada: decidir si se permite pasar la llamada ---
        let current_state = {
            let mut state = self.state.lock().unwrap();

            // Si estamos en Open, comprobar si el timeout de recuperación ha transcurrido.
            if *state == CircuitState::Open {
                let last_fail = self.last_failure_time.lock().unwrap();
                if let Some(t) = *last_fail {
                    if t.elapsed() >= self.recovery_timeout {
                        // Ha transcurrido suficiente tiempo; dejar pasar un sondeo.
                        info!(cb = %self.name, "moviendo OPEN → HALF-OPEN (sondeando)");
                        *state = CircuitState::HalfOpen;
                    }
                }
            }

            state.clone()
        };

        match current_state {
            CircuitState::Open => {
                // Fallo rápido: no llamar a la función subyacente en absoluto.
                warn!(cb = %self.name, "circuito ABIERTO — rechazando llamada");
                return Err(CircuitBreakerError::CircuitOpen);
            }
            CircuitState::Closed | CircuitState::HalfOpen => {
                // Continuar hacia la llamada real.
            }
        }

        // --- Realizar la llamada ---
        let result = f().await;

        // --- Post-llamada: actualizar el estado según el resultado ---
        match &result {
            Ok(_) => {
                let old_failures = self.failure_count.swap(0, Ordering::SeqCst);
                let mut state = self.state.lock().unwrap();
                if *state == CircuitState::HalfOpen {
                    info!(cb = %self.name, "sondeo exitoso — moviendo HALF-OPEN → CLOSED");
                } else if old_failures > 0 {
                    info!(cb = %self.name, fallos_eliminados = old_failures, "llamada exitosa, fallos reiniciados");
                }
                *state = CircuitState::Closed;
            }
            Err(e) => {
                let new_count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                warn!(
                    cb = %self.name,
                    contador_fallos = new_count,
                    umbral = self.threshold,
                    error = ?e,
                    "llamada fallida"
                );

                let mut state = self.state.lock().unwrap();
                if *state == CircuitState::HalfOpen || new_count >= self.threshold {
                    error!(cb = %self.name, "disparando circuito ABIERTO");
                    *state = CircuitState::Open;
                    *self.last_failure_time.lock().unwrap() = Some(Instant::now());
                }
            }
        }

        result.map_err(CircuitBreakerError::CallFailed)
    }

    /// Estado actual — útil para informes de salud.
    pub fn state(&self) -> CircuitState {
        self.state.lock().unwrap().clone()
    }
}

// ---------------------------------------------------------------------------
// Sensor simulado poco fiable
// ---------------------------------------------------------------------------

/// Simula un sensor que falla aleatoriamente.
///
/// En la vida real esto sería una llamada a socket Unix al demonio de sensor (patrón del día 4).
/// Lo mantenemos simple aquí para centrarnos en la lógica del disyuntor.
struct UnreliableSensor {
    fail_count: std::cell::Cell<u32>,
}

#[derive(Debug)]
struct SensorError(String);

impl UnreliableSensor {
    fn new() -> Self {
        Self {
            fail_count: std::cell::Cell::new(0),
        }
    }

    /// Devuelve Ok(temperatura) algunas veces, Err en fallos consecutivos.
    async fn read_temperature(&self) -> Result<f32, SensorError> {
        // Simular latencia I2C.
        sleep(Duration::from_millis(50)).await;

        let n = self.fail_count.get();
        self.fail_count.set(n + 1);

        // Falla en las primeras 7 llamadas, luego se recupera.
        if n < 7 {
            Err(SensorError(format!("I2C NAK en intento {}", n)))
        } else {
            Ok(23.5 + n as f32 * 0.1) // lectura de temperatura plausible
        }
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

    info!("=== Día 5: Demo de Disyuntor ===");

    // threshold=3: disparar tras 3 fallos consecutivos.
    // recovery_timeout=1s: sondear tras 1 segundo en estado Open.
    let cb = CircuitBreaker::new("temperature-sensor", 3, Duration::from_secs(1));
    let sensor = UnreliableSensor::new();

    for i in 0..20 {
        // Espaciar las llamadas para darle tiempo al timeout de recuperación.
        sleep(Duration::from_millis(300)).await;

        let result = cb.call(|| sensor.read_temperature()).await;
        match result {
            Ok(temp) => info!(intento = i, temp_c = temp, estado = ?cb.state(), "lectura OK"),
            Err(CircuitBreakerError::CircuitOpen) => {
                warn!(intento = i, "llamada rechazada por circuito abierto (fallo rápido)");
            }
            Err(CircuitBreakerError::CallFailed(e)) => {
                error!(intento = i, error = ?e, "la llamada llegó al sensor pero falló");
            }
        }
    }

    info!(estado_final = ?cb.state(), "Demo completada");
    info!("Observar: tras 3 fallos el circuito se abrió; las llamadas fueron rechazadas hasta el timeout de 1s; luego un sondeo tuvo éxito y el circuito se cerró de nuevo.");
}
