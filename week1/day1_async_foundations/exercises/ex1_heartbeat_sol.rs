//! Exercise 1 — Solution: Heartbeat task with cancellation

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
                // Shutdown requested — exit cleanly
                break;
            }
            _ = tokio::time::sleep(interval) => {
                sequence += 1;
                let msg = HeartbeatMsg { sequence, timestamp: Instant::now() };
                if tx.send(msg).await.is_err() {
                    // Receiver dropped — no point continuing
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

    // Receive exactly 3 heartbeats
    for expected_seq in 1..=3u32 {
        let msg = tokio::time::timeout(
            Duration::from_millis(500),
            rx.recv(),
        )
        .await
        .expect("timed out")
        .expect("channel closed");
        assert_eq!(msg.sequence, expected_seq);
    }

    // Cancel and verify no more arrive within 200ms
    token.cancel();
    let result = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await;
    // Either timeout (Ok(Err(..)) or the channel closed — both mean no more heartbeats
    assert!(result.is_err() || result.unwrap().is_none());
}

#[tokio::main]
async fn main() {
    println!("Run tests with: cargo test --example ex1_heartbeat_sol");
}
