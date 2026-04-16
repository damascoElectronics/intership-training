// Día 1, Ejercicio 1: Tarea Heartbeat
//
// Implementa una tarea de heartbeat periódico que:
// - Envía un HeartbeatMsg en un canal cada `interval_ms` milisegundos
// - Se detiene limpiamente cuando se cancela un CancellationToken
//
// Este es un patrón que usarás constantemente en daemons Linux embedded:
// un heartbeat de watchdog, una baliza de telemetría, un ping keep-alive.
//
// Ejecutar con:
//   cargo run --example ex1_heartbeat
//
// Probar con:
//   cargo test --example ex1_heartbeat

use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

// Configuración para la tarea heartbeat.
// Mantener la configuración en un struct facilita cambiarla en tiempo de ejecución o cargarla desde archivo.
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    pub interval_ms: u64,
    pub subsystem_id: u8,
}

// El tipo de mensaje enviado en cada heartbeat.
#[derive(Debug, Clone, PartialEq)]
pub struct HeartbeatMsg {
    pub subsystem_id: u8,
    pub sequence_number: u32,
    pub timestamp_ms: u64, // milisegundos desde el inicio de la tarea (para pruebas)
}

// Alias de tipo para claridad — lo usaremos en múltiples lugares
pub type HeartbeatSender = mpsc::Sender<HeartbeatMsg>;
pub type HeartbeatReceiver = mpsc::Receiver<HeartbeatMsg>;

// ─────────────────────────────────────────────────────────────────────────────
// TODO 1: Implementa la función heartbeat_task
//
// Esta función debe:
// 1. Usar tokio::time::interval() para crear un temporizador periódico
//    (Pista: tokio::time::interval(Duration::from_millis(config.interval_ms)))
// 2. En cada tick: enviar un HeartbeatMsg con sequence_number incremental
//    y el timestamp actual
// 3. Usar tokio::select! para competir entre el tick del intervalo y la cancelación
// 4. Cuando el token se cancele: registrar un mensaje y retornar
// 5. Si el envío falla (canal cerrado): tratar como apagado y retornar
//
// El sequence_number debe comenzar en 1 e incrementarse en cada heartbeat.
// El timestamp_ms debe ser el tiempo transcurrido desde que inició la tarea
// (usa tokio::time::Instant para esto).
// ─────────────────────────────────────────────────────────────────────────────
pub async fn heartbeat_task(
    config: HeartbeatConfig,
    tx: HeartbeatSender,
    token: CancellationToken,
) {
    // TODO: implementa esta función
    let _ = (config, tx, token); // elimina esto cuando implementes la función
    todo!("Implementa la función heartbeat_task")
}

// ─────────────────────────────────────────────────────────────────────────────
// TODO 2: Implementa main
//
// En main:
// 1. Crear un HeartbeatConfig con interval_ms = 100 y subsystem_id = 42
// 2. Crear un canal mpsc con capacidad 8
// 3. Crear un CancellationToken
// 4. Lanzar heartbeat_task como tarea tokio (no olvides mover config,
//    tx y el clon del token a la tarea lanzada)
// 5. Recibir 3 heartbeats del canal, imprimiendo cada uno
// 6. Cancelar el token
// 7. Awaitar el handle de la tarea para confirmar que sale limpiamente
// 8. Intentar recibir un heartbeat más — verificar que el canal está vacío
//    (recv() debe retornar None o un timeout)
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    println!("=== Ejercicio 1: Tarea Heartbeat ===\n");

    // TODO: implementa main

    println!("\n¡Ejercicio completado!");
}

// ─────────────────────────────────────────────────────────────────────────────
// Pruebas
//
// Estas pruebas verifican tu implementación sin necesitar hardware.
// Ejecutar con: cargo test --example ex1_heartbeat
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    // Prueba: 3 heartbeats llegan a intervalos de ~50ms dentro de 500ms en total
    #[tokio::test]
    async fn test_three_heartbeats_arrive() {
        let config = HeartbeatConfig {
            interval_ms: 50,
            subsystem_id: 7,
        };
        let (tx, mut rx) = mpsc::channel::<HeartbeatMsg>(8);
        let token = CancellationToken::new();

        let handle = tokio::spawn(heartbeat_task(config.clone(), tx, token.clone()));

        // Debemos recibir 3 heartbeats dentro de 500ms (3 * 50ms = 150ms, con margen)
        let mut received = Vec::new();
        for _ in 0..3 {
            let msg = timeout(Duration::from_millis(500), rx.recv())
                .await
                .expect("Timeout esperando heartbeat")
                .expect("Canal cerrado inesperadamente");
            received.push(msg);
        }

        // Verificar que subsystem_id es correcto en todos los mensajes
        for msg in &received {
            assert_eq!(
                msg.subsystem_id,
                config.subsystem_id,
                "subsystem_id debe coincidir con la configuración"
            );
        }

        // Verificar que los números de secuencia son monótonamente crecientes
        for (i, msg) in received.iter().enumerate() {
            assert_eq!(
                msg.sequence_number,
                (i + 1) as u32,
                "sequence_number debe comenzar en 1 e incrementarse"
            );
        }

        token.cancel();
        handle.await.expect("heartbeat_task tuvo un panic");
    }

    // Prueba: no llegan más heartbeats tras la cancelación
    #[tokio::test]
    async fn test_stops_after_cancellation() {
        let config = HeartbeatConfig {
            interval_ms: 50,
            subsystem_id: 1,
        };
        let (tx, mut rx) = mpsc::channel::<HeartbeatMsg>(8);
        let token = CancellationToken::new();

        let handle = tokio::spawn(heartbeat_task(config, tx, token.clone()));

        // Recibir el primer heartbeat para confirmar que la tarea está ejecutándose
        let first = timeout(Duration::from_millis(500), rx.recv())
            .await
            .expect("Timeout esperando el primer heartbeat")
            .expect("Canal cerrado inesperadamente");
        assert_eq!(first.sequence_number, 1);

        // Cancelar la tarea
        token.cancel();

        // La tarea debe salir rápidamente
        timeout(Duration::from_millis(200), handle)
            .await
            .expect("La tarea no salió dentro de 200ms tras la cancelación")
            .expect("La tarea tuvo un panic");

        // Tras la salida de la tarea, no deben llegar más mensajes
        // recv() debe retornar None (sender descartado) o timeout
        match timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(None) => {} // Canal cerrado — perfecto, la tarea descartó su sender
            Ok(Some(msg)) => {
                // Unos pocos mensajes extra son aceptables (la tarea puede enviar uno más antes de ver la cancelación)
                println!("  Nota: recibido un msg extra tras cancel: {msg:?} (aceptable)");
            }
            Err(_) => {} // Timeout — también aceptable si la tarea mantuvo el sender brevemente
        }
    }
}
