// Day 1, Exercise 1: Heartbeat Task
//
// Implement a periodic heartbeat task that:
// - Sends a HeartbeatMsg on a channel every `interval_ms` milliseconds
// - Stops cleanly when a CancellationToken is cancelled
//
// This is a pattern you'll use constantly in embedded Linux daemons:
// a watchdog heartbeat, a telemetry beacon, a keep-alive ping.
//
// Run with:
//   cargo run --example ex1_heartbeat
//
// Test with:
//   cargo test --example ex1_heartbeat

use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

// Configuration for the heartbeat task.
// Keeping config in a struct makes it easy to change at runtime or load from file.
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    pub interval_ms: u64,
    pub subsystem_id: u8,
}

// The message type sent on every heartbeat.
#[derive(Debug, Clone, PartialEq)]
pub struct HeartbeatMsg {
    pub subsystem_id: u8,
    pub sequence_number: u32,
    pub timestamp_ms: u64, // milliseconds since task start (for testing)
}

// Type alias for clarity — we'll use this in multiple places
pub type HeartbeatSender = mpsc::Sender<HeartbeatMsg>;
pub type HeartbeatReceiver = mpsc::Receiver<HeartbeatMsg>;

// ─────────────────────────────────────────────────────────────────────────────
// TODO 1: Implement the heartbeat_task function
//
// This function should:
// 1. Use tokio::time::interval() to create a periodic timer
//    (Hint: tokio::time::interval(Duration::from_millis(config.interval_ms)))
// 2. On each tick: send a HeartbeatMsg with incrementing sequence_number
//    and the current timestamp
// 3. Use tokio::select! to race between the interval tick and cancellation
// 4. When the token is cancelled: log a message and return
// 5. If sending fails (channel closed): treat as shutdown and return
//
// The sequence_number should start at 1 and increment on each heartbeat.
// The timestamp_ms should be time elapsed since the task started
// (use tokio::time::Instant for this).
// ─────────────────────────────────────────────────────────────────────────────
pub async fn heartbeat_task(
    config: HeartbeatConfig,
    tx: HeartbeatSender,
    token: CancellationToken,
) {
    // TODO: implement this function
    let _ = (config, tx, token); // remove this when you implement the function
    todo!("Implement the heartbeat_task function")
}

// ─────────────────────────────────────────────────────────────────────────────
// TODO 2: Implement main
//
// In main:
// 1. Create a HeartbeatConfig with interval_ms = 100 and subsystem_id = 42
// 2. Create an mpsc channel with capacity 8
// 3. Create a CancellationToken
// 4. Spawn heartbeat_task as a tokio task (don't forget to move the config,
//    tx, and token clone into the spawned task)
// 5. Receive 3 heartbeats from the channel, printing each one
// 6. Cancel the token
// 7. Await the task handle to confirm it exits cleanly
// 8. Try to receive one more heartbeat — verify the channel is now empty
//    (recv() should return None or a timeout)
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    println!("=== Exercise 1: Heartbeat Task ===\n");

    // TODO: implement main

    println!("\nExercise complete!");
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
//
// These tests verify your implementation without needing hardware.
// Run with: cargo test --example ex1_heartbeat
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    // Test: 3 heartbeats arrive at ~50ms intervals within 500ms total
    #[tokio::test]
    async fn test_three_heartbeats_arrive() {
        let config = HeartbeatConfig {
            interval_ms: 50,
            subsystem_id: 7,
        };
        let (tx, mut rx) = mpsc::channel::<HeartbeatMsg>(8);
        let token = CancellationToken::new();

        let handle = tokio::spawn(heartbeat_task(config.clone(), tx, token.clone()));

        // We should receive 3 heartbeats within 500ms (3 * 50ms = 150ms, with margin)
        let mut received = Vec::new();
        for _ in 0..3 {
            let msg = timeout(Duration::from_millis(500), rx.recv())
                .await
                .expect("Timed out waiting for heartbeat")
                .expect("Channel closed unexpectedly");
            received.push(msg);
        }

        // Verify subsystem_id is correct in all messages
        for msg in &received {
            assert_eq!(
                msg.subsystem_id,
                config.subsystem_id,
                "subsystem_id should match config"
            );
        }

        // Verify sequence numbers are monotonically increasing
        for (i, msg) in received.iter().enumerate() {
            assert_eq!(
                msg.sequence_number,
                (i + 1) as u32,
                "sequence_number should start at 1 and increment"
            );
        }

        token.cancel();
        handle.await.expect("heartbeat_task panicked");
    }

    // Test: no more heartbeats arrive after cancellation
    #[tokio::test]
    async fn test_stops_after_cancellation() {
        let config = HeartbeatConfig {
            interval_ms: 50,
            subsystem_id: 1,
        };
        let (tx, mut rx) = mpsc::channel::<HeartbeatMsg>(8);
        let token = CancellationToken::new();

        let handle = tokio::spawn(heartbeat_task(config, tx, token.clone()));

        // Receive first heartbeat to confirm task is running
        let first = timeout(Duration::from_millis(500), rx.recv())
            .await
            .expect("Timed out waiting for first heartbeat")
            .expect("Channel closed unexpectedly");
        assert_eq!(first.sequence_number, 1);

        // Cancel the task
        token.cancel();

        // Task should exit promptly
        timeout(Duration::from_millis(200), handle)
            .await
            .expect("Task did not exit within 200ms after cancellation")
            .expect("Task panicked");

        // After task exits, no more messages should arrive
        // recv() should return None (sender dropped) or timeout
        match timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(None) => {} // Channel closed — perfect, task dropped its sender
            Ok(Some(msg)) => {
                // A few extra messages are acceptable (task may send one more before seeing cancellation)
                println!("  Note: received one extra msg after cancel: {msg:?} (acceptable)");
            }
            Err(_) => {} // Timeout — also acceptable if task kept sender alive briefly
        }
    }
}
