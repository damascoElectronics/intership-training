// Día 1, Ejemplo 4: Macro tokio::select!
//
// select! compite múltiples operaciones async: la que termina primero gana,
// y todas las demás son DESCARTADAS (canceladas). Es el equivalente async de
// POSIX select() o poll(), pero funciona con cualquier future — no solo descriptores de archivo.
//
// Si has escrito un bucle de eventos embedded como:
//   while (1) {
//     if (uart_data_ready()) handle_uart();
//     if (timer_expired()) handle_timer();
//     if (shutdown_requested()) break;
//   }
//
// select! es la versión async idiomática de ese patrón.
//
// Ejecutar con:
//   cargo run --example 04_select_macro

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    println!("=== Ejemplo 04: Macro select! ===\n");

    demo_basic_race().await;
    demo_timeout_pattern().await;
    demo_biased_select().await;
    demo_cancellation_safety().await;

    println!("\nListo.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Parte 1: Carrera básica entre dos operaciones
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_basic_race() {
    println!("--- Parte 1: Carrera Básica ---");

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<&str>(4);
    let token = CancellationToken::new();
    let task_token = token.clone();

    // Simular un comando que llega tras 30ms
    let sender = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        let _ = cmd_tx.send("TC_RESET").await;
    });

    // Este bucle demuestra el patrón central: hacer trabajo útil hasta que BIEN
    // llegue un comando O se solicite un apagado.
    let worker = tokio::spawn(async move {
        let mut ticks = 0u32;
        loop {
            tokio::select! {
                // Rama 1: llegó un comando en el canal
                // cmd_rx.recv() es "cancellation-safe" — si esta rama pierde la
                // carrera, el mensaje permanece en el canal y se recibirá la próxima vez.
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(c) => {
                            println!("  Comando recibido: {c}");
                            break; // Salir del bucle al recibir comando
                        }
                        None => {
                            println!("  Canal de comandos cerrado");
                            break;
                        }
                    }
                }

                // Rama 2: cancelado por señal externa
                _ = task_token.cancelled() => {
                    println!("  Tarea cancelada tras {ticks} ticks");
                    break;
                }

                // Rama 3: tick de trabajo periódico cada 10ms
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    ticks += 1;
                    println!("  Tick de trabajo {ticks}");
                    // Nota: en cada iteración se crea un NUEVO future sleep, así que
                    // el temporizador se reinicia en cada iteración del bucle — esto es intencional.
                }
            }
        }
    });

    worker.await.unwrap();
    sender.await.unwrap();
    println!();
}

// ─────────────────────────────────────────────────────────────────────────────
// Parte 2: Patrón de Timeout
//
// Requisito común: "esperar una respuesta, pero no esperar para siempre."
// En código UART embedded establecerías un temporizador y lo comprobarías en tu bucle de sondeo.
// tokio::time::timeout() envuelve cualquier future con un plazo límite.
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_timeout_pattern() {
    println!("--- Parte 2: Patrón de Timeout ---");

    // Simular un sensor lento que tarda 200ms en responder
    async fn slow_sensor_read() -> f32 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        42.0
    }

    // Simular un sensor rápido que responde enseguida
    async fn fast_sensor_read() -> f32 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        37.5
    }

    // timeout() compite el future contra un plazo.
    // Retorna Ok(valor) si el future termina a tiempo.
    // Retorna Err(Elapsed) si el plazo se alcanza primero.
    let deadline = Duration::from_millis(50);

    match timeout(deadline, slow_sensor_read()).await {
        Ok(val) => println!("  Sensor lento respondió: {val}°C"),
        Err(_elapsed) => {
            println!("  Sensor lento superó el timeout tras {deadline:?} — usando valor obsoleto o por defecto");
        }
    }

    match timeout(deadline, fast_sensor_read()).await {
        Ok(val) => println!("  Sensor rápido respondió: {val}°C"),
        Err(_elapsed) => println!("  Sensor rápido superó el timeout (inesperado)"),
    }

    // Patrón: reintento con timeout, N intentos
    let result = try_with_retries(3, Duration::from_millis(30)).await;
    println!("  Tras reintentos: {:?}", result);
    println!();
}

// Helper de reintentos: intenta N veces, cada una con su propio timeout.
// Tras cada fallo, espera un poco antes de reintentar.
async fn try_with_retries(attempts: u32, per_attempt_timeout: Duration) -> Result<f32, &'static str> {
    // Simula un sensor inestable: falla las primeras dos veces, tiene éxito en la tercera
    static CALL_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    for attempt in 1..=attempts {
        let count = CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let future = async move {
            // Las primeras 2 llamadas son lentas (timeout), la tercera es rápida
            if count < 2 {
                tokio::time::sleep(Duration::from_millis(100)).await;
                99.0f32
            } else {
                tokio::time::sleep(Duration::from_millis(5)).await;
                28.3f32
            }
        };

        match timeout(per_attempt_timeout, future).await {
            Ok(val) => {
                println!("  Intento {attempt}: éxito ({val:.1})");
                return Ok(val);
            }
            Err(_) => {
                println!("  Intento {attempt}: timeout");
                if attempt < attempts {
                    tokio::time::sleep(Duration::from_millis(5)).await; // espera progresiva
                }
            }
        }
    }

    Err("Todos los intentos superaron el timeout")
}

// ─────────────────────────────────────────────────────────────────────────────
// Parte 3: Select Sesgado para Prioridad
//
// Por defecto, cuando múltiples ramas están listas simultáneamente, select! elige
// una al azar. Esto es justo, pero a veces necesitas prioridad:
// "siempre procesar comandos de apagado antes que trabajo regular."
//
// La palabra clave `biased` hace que select! compruebe las ramas de arriba a abajo.
// Si la primera rama está lista, siempre gana independientemente de las demás.
//
// Caso de uso real: telemetría de mantenimiento vs procesamiento de telecomandos.
// En sistemas espaciales, los TC tienen prioridad sobre HK — el select sesgado expresa esto.
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_biased_select() {
    println!("--- Parte 3: Select Sesgado (Prioridad) ---");

    let (tc_tx, mut tc_rx) = mpsc::channel::<&str>(8); // Telecomandos de alta prioridad
    let (hk_tx, mut hk_rx) = mpsc::channel::<&str>(8); // Mantenimiento de baja prioridad

    // Saturar ambos canales con mensajes
    for i in 0..3 {
        tc_tx.send(format!("TC-{i}").leak()).await.unwrap();
        hk_tx.send(format!("HK-{i}").leak()).await.unwrap();
    }

    // Procesar durante varias iteraciones, mostrando que TC obtiene prioridad
    for _ in 0..6 {
        tokio::select! {
            // `biased` hace esto determinista: ramas comprobadas de arriba a abajo.
            // La rama TC se comprueba PRIMERO. Si hay un TC esperando Y un HK esperando,
            // el TC siempre gana. Solo cuando la cola TC está vacía se procesa HK.
            biased;

            // Prioridad 1: Procesar telecomando
            tc = tc_rx.recv() => {
                if let Some(cmd) = tc {
                    println!("  [PRIORIDAD ALTA] TC procesado: {cmd}");
                } else {
                    break;
                }
            }

            // Prioridad 2: Procesar mantenimiento (solo cuando no hay TC pendiente)
            hk = hk_rx.recv() => {
                if let Some(pkt) = hk {
                    println!("  [PRIORIDAD BAJA]  HK procesado: {pkt}");
                } else {
                    break;
                }
            }

            // Comprobar siempre el apagado (pero menor prioridad que TC)
            else => break,
        }
    }

    println!("  Observa: todos los TC procesados antes de cualquier HK (orden sesgado)\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Parte 4: Seguridad de Cancelación
//
// Cuando una rama pierde la carrera en select!, el future en esa rama es
// DESCARTADO — lo que significa que la operación async es cancelada a mitad.
//
// Esto solo es seguro si el future es "cancellation-safe":
// no deja ningún recurso externo en un estado inconsistente al ser descartado.
//
// Seguro en select!:
// - channel recv() (el mensaje permanece en el canal)
// - CancellationToken::cancelled()
// - tokio::time::sleep()
//
// NO seguro en select! sin cuidado:
// - Escritura en un archivo (puede quedar parcialmente escrito)
// - Un protocolo multi-paso donde has enviado pero aún no recibido el reconocimiento
//
// La solución para futures inseguros: usa un flag o wrapper que se complete atómicamente,
// o usa tokio::task::spawn() para la parte insegura y haz join del handle.
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_cancellation_safety() {
    println!("--- Parte 4: Seguridad de Cancelación ---");

    let (tx, mut rx) = mpsc::channel::<u32>(4);

    // Demostrar que recv() es seguro: si la rama es cancelada mientras espera,
    // el mensaje NO se pierde — permanece en el canal para la próxima vez.
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.send(42).await.unwrap();
    });

    let mut received = false;
    let mut iterations = 0;

    // Ejecutar un bucle select donde el temporizador dispara primero varias iteraciones,
    // y eventualmente llega el mensaje del canal. Porque recv() es
    // cancellation-safe, el mensaje se preserva entre iteraciones.
    while !received && iterations < 20 {
        tokio::select! {
            val = rx.recv() => {
                println!("  Mensaje recibido: {:?}", val);
                received = true;
            }
            _ = tokio::time::sleep(Duration::from_millis(10)) => {
                // El future recv() fue descartado (cancelado) aquí cada iteración.
                // Pero el mensaje sigue esperando en el buffer del canal.
                iterations += 1;
                println!("  Temporizador disparó (iter {iterations}), future recv cancelado, mensaje preservado");
            }
        }
    }

    // Ejemplo de lo que NO hacer: future inseguro en select!
    // Conceptualmente (no ejecutado aquí — solo un comentario de código):
    //
    //   select! {
    //       _ = write_config_to_flash() => {}  // PELIGROSO: puede quedar parcialmente escrito
    //       _ = shutdown_signal() => {}         // si esto gana, la escritura a flash se cancela!
    //   }
    //
    // La solución: envolver la operación para que se complete totalmente o no comience,
    // o usar tokio::task::spawn() para la parte insegura y hacer join del handle.

    println!("  (Demostración de seguridad de cancelación completa)\n");
}
