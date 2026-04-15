// Day 1, Example 3: Tokio Channels
//
// Channels are the async equivalent of FreeRTOS message queues (xQueueSend /
// xQueueReceive), but type-safe and with multiple flavors for different patterns.
//
// Tokio provides three channel types. Choosing the wrong one leads to bugs or
// performance issues — read the comparison table below carefully.
//
// Run with:
//   cargo run --example 03_channels

use std::time::Duration;
use tokio::sync::{broadcast, mpsc, watch};

// ─────────────────────────────────────────────────────────────────────────────
// Channel Comparison Table
// ─────────────────────────────────────────────────────────────────────────────
//
// ┌──────────────┬──────────────────┬──────────────────────────────────────────┐
// │ Channel Type │ Producers/Consumers│ Semantics                             │
// ├──────────────┼──────────────────┼──────────────────────────────────────────┤
// │ mpsc         │ Many Tx, one Rx  │ Every message consumed by ONE receiver  │
// │              │                  │ Bounded (backpressure) or unbounded      │
// │              │                  │ Use for: work queues, telemetry pipeline │
// ├──────────────┼──────────────────┼──────────────────────────────────────────┤
// │ broadcast    │ One or Many Tx,  │ Every message delivered to ALL receivers │
// │              │ many Rx          │ Receivers can lag (ring buffer overflow) │
// │              │                  │ Use for: system events, mode changes      │
// ├──────────────┼──────────────────┼──────────────────────────────────────────┤
// │ watch        │ One Tx, many Rx  │ Only LATEST value is kept               │
// │              │                  │ Old values discarded when new one arrives│
// │              │                  │ Use for: current state (mode, setpoint)  │
// └──────────────┴──────────────────┴──────────────────────────────────────────┘
//
// There is also oneshot: exactly one message, one producer, one consumer.
// Use for: request-response patterns, returning a result to a caller.

#[derive(Debug, Clone)]
struct TelemetryPacket {
    sensor_id: u8,
    value: f32,
    timestamp_ms: u64,
}

#[derive(Debug, Clone)]
enum HealthEvent {
    SensorOnline(u8),
    SensorOffline(u8),
    OverTemperature { zone: u8, temp_c: f32 },
}

#[derive(Debug, Clone, PartialEq)]
enum SystemMode {
    Nominal,
    SafeMode,
    Emergency,
}

#[tokio::main]
async fn main() {
    println!("=== Example 03: Channels ===\n");

    demo_mpsc().await;
    demo_broadcast().await;
    demo_watch().await;

    println!("\nDone.");
}

// ─────────────────────────────────────────────────────────────────────────────
// mpsc: Multi-Producer Single-Consumer
//
// Classic use case: multiple sensor tasks each sending telemetry to a single
// aggregator task. Each packet is consumed exactly once — no duplication.
//
// This is equivalent to a FreeRTOS queue where multiple tasks push, one pops.
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_mpsc() {
    println!("--- mpsc: Telemetry Aggregation ---");

    // Bounded channel: buffer up to 32 packets.
    // If the buffer is full, send().await will block (backpressure).
    // This is important: it prevents a fast producer from overwhelming a slow consumer.
    // In embedded terms: it's flow control, like UART hardware flow control but in software.
    let (tx, mut rx) = mpsc::channel::<TelemetryPacket>(32);

    // Spawn two sensor tasks, each with a clone of the sender.
    // mpsc::Sender is cheap to clone — all clones share the same underlying channel.
    let tx1 = tx.clone();
    let sensor1 = tokio::spawn(async move {
        for i in 0..3 {
            let packet = TelemetryPacket {
                sensor_id: 1,
                value: 23.5 + i as f32 * 0.1,
                timestamp_ms: i * 100,
            };
            // send() returns Err if the receiver has been dropped (channel closed).
            // In a real daemon you'd handle this as a shutdown signal.
            if tx1.send(packet).await.is_err() {
                println!("  Sensor 1: channel closed, stopping");
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        println!("  Sensor 1: done sending");
    });

    let tx2 = tx.clone();
    let sensor2 = tokio::spawn(async move {
        for i in 0..3 {
            let packet = TelemetryPacket {
                sensor_id: 2,
                value: 3.3 - i as f32 * 0.05,
                timestamp_ms: i * 100 + 50,
            };
            if tx2.send(packet).await.is_err() {
                println!("  Sensor 2: channel closed, stopping");
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        println!("  Sensor 2: done sending");
    });

    // Drop the original sender. The channel stays open as long as ANY sender clone exists.
    // When the LAST sender is dropped, the receiver gets None from recv() — channel closed.
    drop(tx);

    // Aggregator task: receive all packets until the channel closes.
    // In real code this would be building a telemetry report or writing to a database.
    let aggregator = tokio::spawn(async move {
        let mut total = 0usize;
        // recv() returns None when all Senders have been dropped
        while let Some(pkt) = rx.recv().await {
            println!(
                "  Aggregator received: sensor={} value={:.2} ts={}ms",
                pkt.sensor_id, pkt.value, pkt.timestamp_ms
            );
            total += 1;
        }
        println!("  Aggregator: channel closed, received {total} total packets");
    });

    tokio::join!(sensor1, sensor2, aggregator).0.unwrap();
    println!();
}

// ─────────────────────────────────────────────────────────────────────────────
// broadcast: Publish Health Events to Multiple Subscribers
//
// Use when multiple independent consumers ALL need to see EVERY event.
// Example: a health monitor publishes events; both the logger and the
// command handler need to react to them independently.
//
// Unlike mpsc, broadcast uses a ring buffer. If a receiver is slow and
// the buffer fills up, old messages are overwritten. The receiver gets
// RecvError::Lagged(n) indicating it missed n messages.
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_broadcast() {
    println!("--- broadcast: Health Event Distribution ---");

    // Ring buffer capacity: 16 events.
    // When full, the oldest unread message is overwritten.
    let (tx, _) = broadcast::channel::<HealthEvent>(16);

    // Each subscriber gets its own receiver by calling .subscribe().
    // Receivers are independent: one slow receiver doesn't block others.
    let mut rx_logger = tx.subscribe();
    let mut rx_cmd_handler = tx.subscribe();

    // Logger: records all events
    let logger = tokio::spawn(async move {
        loop {
            match rx_logger.recv().await {
                Ok(event) => println!("  Logger: {:?}", event),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    println!("  Logger: WARN — missed {n} events (buffer overrun)");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    println!("  Logger: channel closed, stopping");
                    break;
                }
            }
        }
    });

    // Command handler: only reacts to critical events
    let cmd_handler = tokio::spawn(async move {
        loop {
            match rx_cmd_handler.recv().await {
                Ok(HealthEvent::OverTemperature { zone, temp_c }) => {
                    println!("  CmdHandler: ALERT! Zone {zone} over-temperature at {temp_c:.1}°C — initiating safe mode");
                }
                Ok(HealthEvent::SensorOnline(id)) => {
                    println!("  CmdHandler: Sensor {id} online — nominal");
                }
                Ok(HealthEvent::SensorOffline(id)) => {
                    println!("  CmdHandler: Sensor {id} offline — degraded mode");
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    println!("  CmdHandler: missed {n} events");
                }
            }
        }
    });

    // Publish events. Both logger and cmd_handler receive ALL of them.
    let events = vec![
        HealthEvent::SensorOnline(1),
        HealthEvent::SensorOnline(2),
        HealthEvent::OverTemperature {
            zone: 3,
            temp_c: 95.7,
        },
        HealthEvent::SensorOffline(1),
    ];

    for event in events {
        // send() returns the number of receivers that received the message.
        // Returns Err if there are no receivers (all dropped).
        let receivers = tx.send(event).expect("No receivers");
        println!("  Publisher: sent to {receivers} receivers");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Drop sender to close channel
    drop(tx);
    tokio::join!(logger, cmd_handler).0.unwrap();
    println!();
}

// ─────────────────────────────────────────────────────────────────────────────
// watch: Track the Latest State
//
// Use when you care about the CURRENT value, not every historical update.
// Example: system operating mode. If the mode changes twice before a task
// checks it, the task only needs to know the current mode — not the history.
//
// This is like a shared global variable, but:
// - Write is atomic and consistent (no torn reads)
// - Readers can efficiently wait for the value to change (changed().await)
// - Multiple readers, one writer
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_watch() {
    println!("--- watch: System Mode State ---");

    // Initial value is SystemMode::Nominal
    let (tx, rx) = watch::channel(SystemMode::Nominal);

    // Task that reacts to mode changes (e.g., telemetry task adjusting rate)
    let mut rx1 = rx.clone();
    let telemetry_task = tokio::spawn(async move {
        loop {
            // changed() waits until the value has changed since we last checked.
            // It's efficient: no polling, the sender notifies us directly.
            if rx1.changed().await.is_err() {
                println!("  Telemetry: watch channel closed");
                break;
            }
            // borrow() gives a reference to the current value (not ownership).
            // The read lock is held while the Ref exists — drop it quickly.
            let mode = rx1.borrow().clone();
            match &mode {
                SystemMode::Nominal => println!("  Telemetry: nominal mode — normal rate"),
                SystemMode::SafeMode => println!("  Telemetry: safe mode — reduced rate"),
                SystemMode::Emergency => println!("  Telemetry: EMERGENCY — critical data only"),
            }
        }
    });

    // Another task that also tracks mode
    let mut rx2 = rx.clone();
    let power_task = tokio::spawn(async move {
        loop {
            if rx2.changed().await.is_err() {
                break;
            }
            let mode = rx2.borrow().clone();
            if mode == SystemMode::SafeMode || mode == SystemMode::Emergency {
                println!("  Power: entering low-power mode");
            } else {
                println!("  Power: nominal power");
            }
        }
    });

    // Mode controller: simulate state transitions
    tokio::time::sleep(Duration::from_millis(10)).await;

    println!("  Controller: transitioning to SafeMode");
    tx.send(SystemMode::SafeMode).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    println!("  Controller: transitioning to Emergency");
    tx.send(SystemMode::Emergency).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    println!("  Controller: returning to Nominal");
    tx.send(SystemMode::Nominal).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Closing the sender (drop) causes changed() to return Err, ending tasks
    drop(tx);
    tokio::join!(telemetry_task, power_task).0.unwrap();
    println!();
}
