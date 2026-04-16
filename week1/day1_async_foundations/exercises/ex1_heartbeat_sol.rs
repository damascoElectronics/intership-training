//! Ejercicio 1 — Solución: Tarea heartbeat con cancelación

#![allow(dead_code)]

use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct HeartbeatMsg {
    pub sequence: u32,
    pub timestamp: Instant,
}

#[derive(Clone)]
pub struct HeartbeatConfig {
    pub component: String,
    pub interval_ms: u64,
}

pub async fn heartbeat_task(
    config: HeartbeatConfig,
    tx: mpsc::Sender<HeartbeatMsg>,
    token: CancellationToken,
) {
    let interval = Duration::from_millis(config.interval_ms);
    let mut sequence = 0u32;
    loop {
        tokio::select! {
            _ = token.cancelled() => {
                // Apagado solicitado — salir limpiamente
                break;
            }
            _ = tokio::time::sleep(interval) => {
                sequence += 1;
                let msg = HeartbeatMsg { sequence, timestamp: Instant::now() };
                if tx.send(msg).await.is_err() {
                    // Receptor descartado — no tiene sentido continuar
                    break;
                }
            }
        }
    }
}

#[tokio::test]
async fn heartbeat_sends_three_then_cancels() {
    let config = HeartbeatConfig { component: "test".into(), interval_ms: 50 };
    let (tx, mut rx) = mpsc::channel(8);
    let token = CancellationToken::new();
    let child = token.child_token();

    tokio::spawn(heartbeat_task(config, tx, child));

    // Recibir exactamente 3 heartbeats
    for expected_seq in 1..=3u32 {
        let msg = tokio::time::timeout(
            Duration::from_millis(500),
            rx.recv(),
        )
        .await
        .expect("timeout")
        .expect("canal cerrado");
        assert_eq!(msg.sequence, expected_seq);
    }

    // Cancelar y verificar que no llegan más dentro de 200ms
    token.cancel();
    let result = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await;
    // Timeout (Ok(Err(..)) o canal cerrado — ambos significan que no hay más heartbeats
    assert!(result.is_err() || result.unwrap().is_none());
}

#[tokio::main]
async fn main() {
    println!("Ejecutar pruebas con: cargo test --example ex1_heartbeat_sol");
}
