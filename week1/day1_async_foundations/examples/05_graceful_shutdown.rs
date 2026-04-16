// Día 1, Ejemplo 5: Apagado Gracioso
//
// Todo daemon en producción debe manejar SIGTERM (systemd deteniendo el servicio)
// y SIGINT (Ctrl+C en desarrollo). Si no lo haces, el proceso es matado con
// SIGKILL tras un timeout, dejando el hardware en un estado indefinido.
//
// En bare metal podrías usar una interrupción GPIO o watchdog. En Linux:
// - systemd envía SIGTERM al detener un servicio
// - Tienes `TimeoutStopSec` segundos para salir, luego recibes SIGKILL
// - Una salida limpia (código 0) le dice a systemd "parada saludable"
// - Una salida sucia (crash/SIGKILL) activa las políticas de reinicio
//
// El patrón implementado aquí se usa en software real de tierra espacial:
// 1. Instalar manejadores de señal para SIGTERM + SIGINT
// 2. Difundir apagado via CancellationToken
// 3. Cada tarea cierra, vacía datos, libera hardware
// 4. Main espera con un timeout duro (no colgar para siempre)
// 5. Salida limpia
//
// Ejecutar con:
//   cargo run --example 05_graceful_shutdown
// Luego presiona Ctrl+C para disparar el apagado gracioso.
// O envía: kill -SIGTERM <pid>

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

// Representa telemetría en buffer que debe vaciarse al apagar.
// En un sistema real podrían ser paquetes CCSDS encolados.
struct TelemetryBuffer {
    packets: Vec<String>,
}

impl TelemetryBuffer {
    fn new() -> Self {
        Self { packets: Vec::new() }
    }

    fn push(&mut self, pkt: String) {
        self.packets.push(pkt);
    }

    async fn flush(&mut self) {
        // Simular escritura de telemetría en buffer a un archivo o envío por socket
        if self.packets.is_empty() {
            println!("  Flush: buffer vacío, nada que hacer");
            return;
        }
        println!("  Flush: escribiendo {} paquetes en buffer...", self.packets.len());
        tokio::time::sleep(Duration::from_millis(20)).await; // simular I/O
        println!("  Flush: completado");
        self.packets.clear();
    }
}

#[tokio::main]
async fn main() {
    println!("=== Ejemplo 05: Apagado Gracioso ===");
    println!("Presiona Ctrl+C para disparar el apagado gracioso\n");

    // El token de cancelación raíz. Lo cancelamos para iniciar el apagado.
    // Cada tarea obtiene un clon — todas ven la cancelación simultáneamente.
    let shutdown_token = CancellationToken::new();

    // Un canal para recolectar telemetría de todas las tareas.
    // Acotado: si el receptor no puede seguir el ritmo, los senders bloquean (contrapresión).
    let (telem_tx, telem_rx) = mpsc::channel::<String>(64);

    // ── Lanzar tareas worker ────────────────────────────────────────────────

    let heartbeat_handle = tokio::spawn(heartbeat_task(
        shutdown_token.clone(),
        telem_tx.clone(),
    ));

    let sensor_handle = tokio::spawn(sensor_task(
        shutdown_token.clone(),
        telem_tx.clone(),
    ));

    let telem_handle = tokio::spawn(telemetry_aggregator(
        shutdown_token.clone(),
        telem_rx,
    ));

    // Descartar nuestra copia de telem_tx para que el agregador sepa cuándo todos los productores salen
    drop(telem_tx);

    // ── Instalar manejadores de señal ──────────────────────────────────────
    //
    // tokio::signal::ctrl_c() es un manejador Ctrl+C multiplataforma.
    // En Unix, instala un manejador SIGINT.
    //
    // Para SIGTERM (la señal que envía systemd), necesitamos la API específica de Unix.
    // Usar tokio::signal::unix::signal() requiere la característica "signal" de tokio.

    #[cfg(unix)]
    let shutdown_reason = {
        use tokio::signal::unix::{signal, SignalKind};

        // Instalar manejadores tanto para SIGTERM como para SIGINT.
        // signal() retorna un stream — hacemos recv() para esperar la señal.
        let mut sigterm = signal(SignalKind::terminate())
            .expect("Falló la instalación del manejador SIGTERM");
        let mut sigint = signal(SignalKind::interrupt())
            .expect("Falló la instalación del manejador SIGINT");

        // También configurar un apagado automático en 3 segundos para propósitos de demo
        // (para que el ejemplo termine sin requerir Ctrl+C manual)
        let auto_shutdown = tokio::time::sleep(Duration::from_secs(3));

        // Carrera: el que llegue primero dispara el apagado.
        // En un daemon real omitirías la rama auto_shutdown.
        tokio::select! {
            _ = sigterm.recv() => "SIGTERM",
            _ = sigint.recv()  => "SIGINT (Ctrl+C)",
            _ = auto_shutdown  => "apagado-automático (modo demo)",
        }
    };

    // En plataformas no-Unix, solo usar ctrl_c
    #[cfg(not(unix))]
    let shutdown_reason = {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => "SIGINT (Ctrl+C)",
            _ = tokio::time::sleep(Duration::from_secs(3)) => "apagado-automático (modo demo)",
        }
    };

    // ── Iniciar apagado gracioso ───────────────────────────────────────────

    println!("\n[APAGADO] Recibido: {shutdown_reason}");
    println!("[APAGADO] Difundiendo cancelación a todas las tareas...");

    // Esta única llamada notifica a TODAS las tareas que tienen un clon de este token.
    // Cada una completará su unidad de trabajo actual y luego saldrá.
    shutdown_token.cancel();

    // ── Esperar a que todas las tareas salgan (con timeout duro) ──────────
    //
    // El TimeoutStopSec por defecto de systemd es 90s. Damos 5s a nuestras tareas.
    // Si una tarea cuelga (p. ej., bloqueada en hardware), no queremos bloquear para siempre.

    println!("[APAGADO] Esperando que las tareas salgan (timeout de 5s)...");

    let all_tasks = async {
        // Unir todos los handles de tareas. Si alguna tuvo panic, propagar el error.
        let _ = heartbeat_handle.await;
        let _ = sensor_handle.await;
        let _ = telem_handle.await;
    };

    match timeout(Duration::from_secs(5), all_tasks).await {
        Ok(()) => println!("[APAGADO] Todas las tareas salieron limpiamente"),
        Err(_) => {
            println!("[APAGADO] ADVERTENCIA: Algunas tareas no salieron dentro del timeout");
            println!("[APAGADO] Procediendo de todas formas (serán matadas al salir el proceso)");
        }
    }

    println!("[APAGADO] Daemon detenido limpiamente. Hasta luego.");
    // process::exit(0) se llama implícitamente — systemd ve código de salida 0
}

// ─────────────────────────────────────────────────────────────────────────────
// Tareas worker: cada una sigue el mismo patrón
//   - hacer trabajo en un bucle
//   - comprobar el token de cancelación
//   - al cancelar: vaciar/limpiar, luego retornar
// ─────────────────────────────────────────────────────────────────────────────

async fn heartbeat_task(token: CancellationToken, tx: mpsc::Sender<String>) {
    let mut seq = 0u32;
    println!("[heartbeat] iniciado");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                // Apagado solicitado. Enviar un "heartbeat de apagado" final si es posible.
                let final_hb = format!("HB seq={seq} status=SHUTDOWN");
                // try_send no bloquea — si el canal está lleno, lo omitimos
                let _ = tx.try_send(final_hb);
                println!("[heartbeat] detenido (enviados {seq} heartbeats)");
                return;
            }
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                seq += 1;
                let hb = format!("HB seq={seq} status=NOMINAL");
                println!("[heartbeat] tick {seq}");
                if tx.send(hb).await.is_err() {
                    println!("[heartbeat] canal de telemetría cerrado, deteniendo");
                    return;
                }
            }
        }
    }
}

async fn sensor_task(token: CancellationToken, tx: mpsc::Sender<String>) {
    let mut buffer = TelemetryBuffer::new();
    println!("[sensor] iniciado");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                // Al apagar: vaciar lo que esté en buffer antes de salir.
                // Esto es crítico — en un sistema espacial, no se pueden perder datos en vuelo.
                println!("[sensor] apagado recibido, vaciando buffer...");
                buffer.flush().await;
                println!("[sensor] detenido");
                return;
            }
            _ = tokio::time::sleep(Duration::from_millis(300)) => {
                // Recopilar lectura del sensor en buffer
                let pkt = format!("TM temp={:.1}", 20.0 + (rand_f32() * 5.0));
                buffer.push(pkt.clone());
                println!("[sensor] en buffer: {pkt}");

                // Vaciar cuando el buffer tiene suficientes paquetes
                if buffer.packets.len() >= 3 {
                    buffer.flush().await;
                    // En código real, los datos vaciados irían a algún lugar (archivo, socket)
                    // Aquí solo enviamos una notificación
                    let _ = tx.try_send("Flush TM completado".into());
                }
            }
        }
    }
}

async fn telemetry_aggregator(token: CancellationToken, mut rx: mpsc::Receiver<String>) {
    let mut total_received = 0usize;
    println!("[telem] iniciado");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                // Drenar mensajes restantes en el canal antes de detenerse.
                // Tras la cancelación, los productores pueden enviar algunos mensajes finales.
                // Les damos una pequeña ventana para terminar.
                println!("[telem] apagado — drenando canal...");
                // Usar close() + bucle de drenado para procesar mensajes restantes
                rx.close(); // dejar de aceptar nuevos envíos
                while let Ok(msg) = rx.try_recv() {
                    println!("[telem] msg final: {msg}");
                    total_received += 1;
                }
                println!("[telem] detenido (total recibido: {total_received})");
                return;
            }
            msg = rx.recv() => {
                match msg {
                    Some(pkt) => {
                        total_received += 1;
                        // En código real: escribir en base de datos, reenviar a estación terrestre, etc.
                    }
                    None => {
                        // Todos los senders descartados — canal cerrado
                        println!("[telem] canal cerrado, deteniendo (total: {total_received})");
                        return;
                    }
                }
            }
        }
    }
}

// Float pseudo-aleatorio mínimo para propósitos de demo (sin dependencia de rand)
fn rand_f32() -> f32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 1000) as f32 / 1000.0
}
