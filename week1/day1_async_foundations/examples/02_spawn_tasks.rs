// Día 1, Ejemplo 2: Lanzamiento de Tareas con tokio::spawn
//
// En FreeRTOS creas tareas con xTaskCreate() y se ejecutan de forma independiente.
// tokio::spawn() es el equivalente: crea una nueva tarea async concurrente.
//
// Diferencia clave respecto a FreeRTOS:
// - Las tareas de FreeRTOS se asignan en pila, identificadas por un handle de tarea
// - Las tareas de Tokio son máquinas de estado en heap, devueltas como JoinHandle<T>
// - JoinHandle permite awaitar el resultado o cancelar la tarea
//
// Ejecutar con:
//   cargo run --example 02_spawn_tasks

use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    println!("=== Ejemplo 02: Lanzamiento de Tareas ===\n");

    // --- Parte 1: spawn básico y JoinHandle ---
    demo_basic_spawn().await;

    // --- Parte 2: Esperar resultados de tareas ---
    demo_join_handle().await;

    // --- Parte 3: CancellationToken para apagado limpio ---
    demo_cancellation().await;

    // --- Parte 4: Propagación de panics ---
    demo_panic_handling().await;

    println!("\nListo.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Parte 1: spawn básico
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_basic_spawn() {
    println!("--- Parte 1: spawn básico ---");

    // tokio::spawn() crea una NUEVA tarea que se ejecuta concurrentemente con la
    // tarea actual. La tarea actual NO se pausa — ambas tareas se ejecutan al mismo tiempo
    // (o se entrelazan en un ejecutor monohilo).
    //
    // El closure lanzado debe ser 'static + Send:
    // - 'static: la tarea puede sobrevivir al marco de pila actual, así que no puede
    //   tomar prestadas variables locales (a menos que las muevas)
    // - Send: la tarea puede moverse a cualquier hilo worker en cualquier punto .await
    let handle: JoinHandle<()> = tokio::spawn(async {
        // Esto se ejecuta concurrentemente con la tarea que lanza
        tokio::time::sleep(Duration::from_millis(20)).await;
        println!("  Tarea lanzada 1: despertó tras 20ms");
    });

    let handle2: JoinHandle<()> = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        println!("  Tarea lanzada 2: despertó tras 10ms (¡termina primero aunque se lanzó segunda!)");
    });

    // .await en un JoinHandle espera a que la tarea se complete.
    // Aquí vemos que la tarea 2 termina antes que la tarea 1 aunque lanzamos la tarea 1 primero —
    // porque ambas se ejecutan concurrentemente y la tarea 2 tiene un sleep más corto.
    handle.await.expect("La tarea 1 tuvo un panic");
    handle2.await.expect("La tarea 2 tuvo un panic");

    println!("  Ambas tareas terminaron\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Parte 2: Obtener un valor de retorno de una tarea lanzada
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_join_handle() {
    println!("--- Parte 2: Valores de retorno con JoinHandle ---");

    // JoinHandle<T> es genérico sobre el tipo de retorno de la tarea.
    // Como una tarea de FreeRTOS que escribe su resultado en una variable compartida,
    // pero con seguridad de tipos: no puedes malinterpretar el tipo de retorno.
    let handle: JoinHandle<u64> = tokio::spawn(async {
        // Simular un cómputo que tarda algo (p. ej., leer un sensor)
        tokio::time::sleep(Duration::from_millis(5)).await;
        let reading = 42u64;
        println!("  Worker computó lectura del sensor: {reading}");
        reading // Valor de retorno de la tarea lanzada
    });

    // JoinHandle::await retorna Result<T, JoinError>
    // - Ok(valor): la tarea se completó normalmente, valor es el retorno
    // - Err(join_error): la tarea tuvo un panic o fue cancelada
    let result: Result<u64, tokio::task::JoinError> = handle.await;
    let value = result.expect("La tarea worker tuvo un panic");
    println!("  Tarea principal recibió: {value}\n");

    // Ejecutar múltiples tareas concurrentemente y esperar a TODAS.
    // tokio::join! es la versión macro — conduce todos los futures concurrentemente,
    // retorna cuando TODOS se completan.
    let (a, b, c) = tokio::join!(
        tokio::spawn(async { compute(1).await }),
        tokio::spawn(async { compute(2).await }),
        tokio::spawn(async { compute(3).await }),
    );
    println!(
        "  Resultados en paralelo: {}, {}, {}",
        a.unwrap(),
        b.unwrap(),
        c.unwrap()
    );
    println!();
}

async fn compute(n: u64) -> u64 {
    tokio::time::sleep(Duration::from_millis(5)).await;
    n * n
}

// ─────────────────────────────────────────────────────────────────────────────
// Parte 3: CancellationToken — la forma idiomática de detener tareas
// ─────────────────────────────────────────────────────────────────────────────
//
// En FreeRTOS podrías establecer un global volátil `bool keep_running = false` y
// hacer que las tareas lo comprueben en su bucle. CancellationToken es la versión
// type-safe y async-aware de ese patrón.
//
// Es compartible: clónalo y da un clon a cada tarea.
// Es composable: un child_token() se cancela cuando se cancela el hijo o el padre.

async fn demo_cancellation() {
    println!("--- Parte 3: CancellationToken ---");

    // Crear el token raíz. Cancelarlo cancela todos los clones.
    let token = CancellationToken::new();

    // Clonar el token para dárselo a la tarea lanzada.
    // El clon y el original están vinculados — cancelar cualquiera
    // NO cancela al otro, pero cancelar el token RAÍZ
    // sí cancela los tokens hijo creados con .child_token().
    let task_token = token.clone();

    let handle = tokio::spawn(async move {
        println!("  Tarea en segundo plano: iniciando bucle de sondeo");
        let mut count = 0u32;

        loop {
            // tokio::select! compite múltiples futures.
            // El que termina primero gana; los demás se descartan (cancelan).
            // Aquí competimos: "¿se canceló el token?" vs "¿disparó el temporizador?"
            tokio::select! {
                // Rama de cancelación: el token es "consumido" por esta comprobación.
                // cancelled() retorna un future que se resuelve cuando se llama cancel().
                _ = task_token.cancelled() => {
                    println!("  Tarea en segundo plano: cancelación recibida, deteniendo (count={count})");
                    break;
                }
                // Rama de trabajo: hacer una unidad de trabajo cada 15ms
                _ = tokio::time::sleep(Duration::from_millis(15)) => {
                    count += 1;
                    println!("  Tarea en segundo plano: tick {count}");
                }
            }
        }
    });

    // Dejar que la tarea se ejecute un rato
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Señalar el apagado. Todos los clones de `token` retornarán de .cancelled().
    println!("  Main: cancelando token");
    token.cancel();

    // Esperar a que la tarea reconozca y salga
    handle.await.expect("La tarea en segundo plano tuvo un panic");
    println!("  Tarea salió limpiamente\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Parte 4: Qué ocurre cuando una tarea lanzada tiene un panic
// ─────────────────────────────────────────────────────────────────────────────
//
// A diferencia de FreeRTOS donde un panic de tarea (HardFault) podría colapsar todo el sistema,
// tokio aísla los panics de tareas: el panic se captura en el límite de la tarea y
// se informa como JoinError cuando haces await del handle.
//
// Esto es importante para la resiliencia del daemon: un panic en una tarea no debería
// derribar todo el daemon.

async fn demo_panic_handling() {
    println!("--- Parte 4: Propagación de panics ---");

    let handle = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(1)).await;
        // Este panic se captura en el límite de la tarea, no en el punto de lanzamiento
        panic!("¡Panic simulado del driver de sensor!");
    });

    match handle.await {
        Ok(()) => println!("  Tarea completada normalmente"),
        Err(join_error) if join_error.is_panic() => {
            // El mensaje del panic se captura. Podemos registrarlo y decidir si
            // reiniciar la tarea, alertar a operaciones, o apagar limpiamente.
            println!("  Tarea tuvo panic (capturado en límite): {join_error}");
            println!("  El daemon continúa ejecutándose — solo murió esta tarea");
        }
        Err(join_error) => {
            // Esta rama maneja la cancelación de tareas via handle.abort()
            println!("  Tarea fue cancelada: {join_error}");
        }
    }

    // Mostrar que abortar una tarea también produce un JoinError
    let handle = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(100)).await; // se ejecutaría para siempre
        unreachable!("Debería ser abortado antes de aquí");
    });

    // Abortar la tarea externamente (como matar una tarea de FreeRTOS con vTaskDelete)
    handle.abort();

    match handle.await {
        Ok(()) => println!("  Tarea completada normalmente (improbable tras abort)"),
        Err(e) if e.is_cancelled() => println!("  Tarea fue abortada (como se esperaba)\n"),
        Err(e) => println!("  Inesperado: {e}\n"),
    }
}
