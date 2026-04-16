# Día 1: Fundamentos de Async con Tokio

## ¿Por qué Async? Metal Desnudo vs. Daemons en Linux

En tu STM32 o ESP32 probablemente usaste alguna de estas opciones:
- Un **superloop** (`while(1) { poll_uart(); poll_spi(); ... }`)
- Un **RTOS** (tareas FreeRTOS + colas)
- **Manejadores de interrupciones** que despiertan tareas

El problema fundamental es el mismo en todos lados: **tienes N cosas que necesitan atención, y solo puedes hacer una a la vez en un núcleo de CPU.** Necesitas un planificador para multiplexarlas.

En metal desnudo tenías dos herramientas: interrupciones (el hardware te empuja) y polling (tú jalas). En Linux obtienes una tercera: **el sistema de eventos de I/O del kernel** (`epoll`). El runtime async de Tokio está construido sobre `epoll`, lo que significa:

- Sin CPU desperdiciada girando en un bucle de polling ajustado
- El kernel despierta tu hilo *solo* cuando llegan datos
- Un único hilo del SO puede gestionar miles de operaciones de I/O concurrentes
- Escribes código de aspecto secuencial (`let data = socket.read().await`) que en realidad es multitarea cooperativa bajo el capó

**¿Por qué no usar hilos del SO?** Cada hilo del SO cuesta ~8 MB de pila por defecto. Un daemon gestionando 100 conexiones de sensores usaría 800 MB solo en pilas. Las tareas de Tokio cuestan ~cientos de bytes. La matemática es obvia.

---

## Internos del Runtime de Tokio

### El Panorama General

```
Tu código (async fns + puntos .await)
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│                   Runtime de Tokio                      │
│                                                         │
│  ┌──────────────┐    ┌──────────────┐                  │
│  │  Hilo        │    │  Hilo        │  ← hilos del SO  │
│  │  Worker 0    │    │  Worker 1    │    (por defecto:  │
│  │              │    │              │     num_cpus)     │
│  │  [Tarea A]   │    │  [Tarea C]   │                  │
│  │  [Tarea B]   │    │  [Tarea D]   │                  │
│  └──────┬───────┘    └──────┬───────┘                  │
│         │                  │                            │
│         └────────┬─────────┘                           │
│                  │  cola work-stealing                  │
│                  ▼                                      │
│  ┌───────────────────────────────┐                     │
│  │         Reactor               │                     │
│  │  (biblioteca mio → epoll/kqueue) │                  │
│  │                               │                     │
│  │  fds registrados: socket A,   │                     │
│  │  socket B, timer C, pipe D... │                     │
│  └───────────────────────────────┘                     │
└─────────────────────────────────────────────────────────┘
         │
         ▼
    Kernel de Linux (epoll_wait)
```

### Pool de Hilos con Work-Stealing

Tokio lanza N hilos del SO (por defecto: número de núcleos de CPU). Cada hilo tiene una **cola de ejecución local** de tareas. Cuando la cola de un hilo está vacía, **roba** tareas de las colas de otros hilos. Esto significa:

- El trabajo de CPU se distribuye automáticamente entre núcleos sin que tengas que pensar en ello
- Una tarea puede ejecutarse en diferentes hilos del SO entre puntos `.await` (por eso importa `Send` — ver más abajo)
- Ningún hilo está ocioso mientras haya trabajo por hacer

### El Reactor (Integración con epoll)

```
                    ┌─────────────────────────────┐
                    │  Tu tarea llama              │
                    │  socket.read().await         │
                    └──────────┬──────────────────┘
                               │
                    ┌──────────▼──────────────────┐
                    │  Future::poll() devuelve     │
                    │  Poll::Pending               │
                    │  (aún no hay datos)          │
                    └──────────┬──────────────────┘
                               │ registra Waker con el reactor
                    ┌──────────▼──────────────────┐
                    │  El reactor llama            │
                    │  epoll_ctl(ADD, fd, EPOLLIN) │
                    └──────────┬──────────────────┘
                               │ la tarea queda suspendida (usa 0 CPU)
                    ┌──────────▼──────────────────┐
                    │  ... pasa el tiempo ...      │
                    │  el kernel recibe paquete TCP│
                    └──────────┬──────────────────┘
                               │
                    ┌──────────▼──────────────────┐
                    │  epoll_wait() devuelve fd    │
                    │  El reactor llama waker.wake()│
                    └──────────┬──────────────────┘
                               │ tarea re-encolada
                    ┌──────────▼──────────────────┐
                    │  Future::poll() vuelve a     │
                    │  llamarse, devuelve          │
                    │  Poll::Ready(data)           │
                    └─────────────────────────────┘
```

Este es el núcleo de async en Rust: `.await` es azúcar sintáctico para "haz polling de este future; si devuelve Pending, registra un waker y cede el hilo; cuando se despierte, haz polling de nuevo."

### Lo que `#[tokio::main]` Expande en Realidad

```rust
// Lo que escribes:
#[tokio::main]
async fn main() { ... }

// Lo que genera la macro (aproximadamente):
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { ... })  // ejecuta tu async main en el runtime
}
```

---

## `spawn` vs `spawn_blocking`: La Distinción Crítica

Aquí es donde tu experiencia en embebidos puede confundirte.

**En tu mundo RTOS:** llamar a `HAL_SPI_Transmit()` bloquea la tarea que llama. Las demás tareas del RTOS siguen ejecutándose porque el RTOS hace cambios de contexto de forma preemptiva.

**En tokio:** llamar a una función bloqueante **bloquea el hilo del SO completo**. Ese hilo del SO no puede ejecutar ninguna otra tarea async hasta que la llamada bloqueante retorne. Si todos los hilos worker están bloqueados, los nuevos eventos de I/O quedan sin procesar.

```
Hilo Worker 0: [Tarea A ──────────────────────────────────────] ¡bloqueado!
                         ^ llama std::fs::read() aquí
                           no puede ejecutar Tarea B, C, D hasta que retorne
```

La regla: **nunca llames funciones bloqueantes dentro de código async.**

```rust
// MAL: bloquea el hilo worker
let data = std::fs::read("/dev/sda")?;   // NO HAGAS ESTO
let _    = std::thread::sleep(dur);       // NO HAGAS ESTO
let _    = mutex.lock().unwrap();         // cuidado con mutexes en disputa

// BIEN: ejecuta trabajo bloqueante en un pool de hilos bloqueantes dedicado
let data = tokio::fs::read("/dev/sda").await?;    // I/O asíncrono
tokio::time::sleep(dur).await;                     // sleep asíncrono
tokio::task::spawn_blocking(|| heavy_cpu_work()).await?;  // delega lo bloqueante
```

`spawn_blocking` mueve el closure a un pool de hilos separado que puede crecer ilimitadamente (hasta 512 hilos por defecto). Esos hilos *pueden* bloquearse — no son hilos worker async.

```
Pool de Workers Async (fijo, ej. 8 hilos):
  Worker 0:  Tarea A, Tarea B, Tarea C ...   ← tareas async, nunca se bloquean
  Worker 1:  Tarea D, Tarea E ...

Pool de Hilos Bloqueantes (ampliable, hasta 512):
  Bloqueante 0: std::fs::read(...)          ← puede bloquearse todo el día
  Bloqueante 1: heavy_compression(...)
```

---

## `Send + Sync`: Por qué el Compilador Hace Seguimiento de Esto

Entre dos puntos `.await`, tu tarea puede ser movida a un **hilo del SO diferente** por el planificador work-stealing. Esto significa que todo lo que tu future mantiene a través de un `.await` debe implementar `Send` (seguro para mover entre hilos).

```rust
// Esto NO compila:
let rc = std::rc::Rc::new(42);   // Rc es !Send (conteo de referencias no thread-safe)
do_something().await;             // la tarea podría migrar de hilo aquí
println!("{}", rc);               // rc sigue siendo mantenido, ¡pero podríamos estar en un nuevo hilo!

// Usa Arc en su lugar:
let arc = std::sync::Arc::new(42);  // Arc es Send (conteo de referencias atómico)
do_something().await;
println!("{}", arc);                 // correcto: Arc puede cruzar límites de hilos
```

El compilador de Rust **verifica estáticamente** los límites de `Send` en tiempo de compilación. Está haciendo el análisis de seguridad de hilos que harías manualmente en C, y se niega a compilar si encuentra una violación.

---

## Concurrencia Estructurada y Cancelación de Tareas

**No estructurada:** lanzar tareas y esperar que terminen (el modelo antiguo de hilos)
```
main lanza tarea A → tarea A lanza tarea B → main sale → ¿tarea B sigue ejecutándose?
```

**Estructurada:** las tareas forman un árbol; el padre vive más que los hijos; la cancelación se propaga
```
main
 ├── tarea A (emisor de heartbeat)
 │    └── cancelada cuando el token se descarta
 └── tarea B (colector de telemetría)
      └── cancelada cuando el token se descarta
```

`CancellationToken` de `tokio-util` es la herramienta idiomática:
```rust
let token = CancellationToken::new();

// Dale un clon a cada tarea hija
let child_token = token.child_token(); // la cancelación del hijo se propaga desde el padre
tokio::spawn(async move {
    tokio::select! {
        _ = child_token.cancelled() => { /* limpiar y salir */ }
        _ = do_work() => {}
    }
});

// Más tarde, cancela todo
token.cancel();  // todos los tokens hijos también son cancelados
```

---

## Patrón de Apagado Ordenado

Los daemons reales necesitan manejar `SIGTERM` (systemd deteniendo el servicio) y `SIGINT` (Ctrl+C durante el desarrollo). El patrón:

1. Instalar manejador de señales
2. Difundir token de cancelación
3. Esperar a que las tareas confirmen (con timeout para no colgarse indefinidamente)
4. Vaciar cualquier estado en buffer (telemetría, logs)
5. Salir

```
Llega SIGTERM
      │
      ▼
CancellationToken::cancel()
      │
      ├──→ Tarea A: select! ve cancelled(), envía telemetría final, retorna
      ├──→ Tarea B: select! ve cancelled(), vacía buffer, retorna
      └──→ Tarea C: select! ve cancelled(), cierra dispositivo, retorna
      │
      ▼ (o timeout tras 5s si una tarea se cuelga)
tokio::join!(handle_a, handle_b, handle_c)
      │
      ▼
el proceso sale limpiamente (systemd ve salida limpia, sin reinicio)
```

---

## Ejecutar los Ejemplos

```bash
# Desde week1/day1_async_foundations/
cargo run --example 01_basic_runtime
cargo run --example 02_spawn_tasks
cargo run --example 03_channels
cargo run --example 04_select_macro
cargo run --example 05_graceful_shutdown

# Ejercicios (intenta implementar antes de ver la solución)
cargo run --example ex1_heartbeat
cargo run --example ex1_heartbeat_sol

# Ejecutar tests
cargo test
```

---

## Conceptos Clave

| Concepto | Analogía en Metal Desnudo | Equivalente en Tokio |
|---------|------------------|-----------------|
| Polling con superloop | `while(1) { poll_all(); }` | `epoll_wait` en el reactor |
| ISR despertando una tarea | interrupción HAL → cola RTOS | `Waker::wake()` |
| Cambio de tarea RTOS | cambio de contexto preemptivo | punto de cesión `.await` |
| Llamada bloqueante HAL | `HAL_SPI_Transmit()` | `spawn_blocking` |
| Mutex | `osMutexAcquire()` | `tokio::sync::Mutex` |
| Notificación de tarea | `osTaskNotify()` | `tokio::sync::Notify` |
| Cola de mensajes | `osMessageQueuePut()` | `tokio::sync::mpsc` |
