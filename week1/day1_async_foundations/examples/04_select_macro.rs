// Day 1, Example 4: tokio::select! Macro
//
// select! races multiple async operations: whichever completes first wins,
// and all others are DROPPED (cancelled). This is the async equivalent of
// POSIX select() or poll(), but works with any future — not just file descriptors.
//
// If you've written an embedded event loop like:
//   while (1) {
//     if (uart_data_ready()) handle_uart();
//     if (timer_expired()) handle_timer();
//     if (shutdown_requested()) break;
//   }
//
// select! is the idiomatic async version of that pattern.
//
// Run with:
//   cargo run --example 04_select_macro

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    println!("=== Example 04: select! Macro ===\n");

    demo_basic_race().await;
    demo_timeout_pattern().await;
    demo_biased_select().await;
    demo_cancellation_safety().await;

    println!("\nDone.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Part 1: Basic race between two operations
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_basic_race() {
    println!("--- Part 1: Basic Race ---");

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<&str>(4);
    let token = CancellationToken::new();
    let task_token = token.clone();

    // Simulate a command arriving after 30ms
    let sender = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        let _ = cmd_tx.send("TC_RESET").await;
    });

    // This loop demonstrates the core pattern: do useful work until EITHER
    // a command arrives OR a shutdown is requested.
    let worker = tokio::spawn(async move {
        let mut ticks = 0u32;
        loop {
            tokio::select! {
                // Branch 1: a command arrived on the channel
                // cmd_rx.recv() is "cancellation-safe" — if this branch loses the
                // race, the message stays in the channel and will be received next time.
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(c) => {
                            println!("  Received command: {c}");
                            break; // Exit loop on command
                        }
                        None => {
                            println!("  Command channel closed");
                            break;
                        }
                    }
                }

                // Branch 2: cancelled by external signal
                _ = task_token.cancelled() => {
                    println!("  Task cancelled after {ticks} ticks");
                    break;
                }

                // Branch 3: periodic 10ms work tick
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    ticks += 1;
                    println!("  Work tick {ticks}");
                    // Note: each iteration, a NEW sleep future is created, so
                    // the timer resets on every loop iteration — this is intentional.
                }
            }
        }
    });

    worker.await.unwrap();
    sender.await.unwrap();
    println!();
}

// ─────────────────────────────────────────────────────────────────────────────
// Part 2: Timeout Pattern
//
// Common requirement: "wait for a response, but don't wait forever."
// In embedded UART code you'd set a timer and check it in your polling loop.
// tokio::time::timeout() wraps any future with a deadline.
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_timeout_pattern() {
    println!("--- Part 2: Timeout Pattern ---");

    // Simulate a slow sensor that takes 200ms to respond
    async fn slow_sensor_read() -> f32 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        42.0
    }

    // Simulate a fast sensor that responds quickly
    async fn fast_sensor_read() -> f32 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        37.5
    }

    // timeout() races the future against a deadline.
    // Returns Ok(value) if the future completes in time.
    // Returns Err(Elapsed) if the deadline hits first.
    let deadline = Duration::from_millis(50);

    match timeout(deadline, slow_sensor_read()).await {
        Ok(val) => println!("  Slow sensor responded: {val}°C"),
        Err(_elapsed) => {
            println!("  Slow sensor timed out after {deadline:?} — using stale value or default");
        }
    }

    match timeout(deadline, fast_sensor_read()).await {
        Ok(val) => println!("  Fast sensor responded: {val}°C"),
        Err(_elapsed) => println!("  Fast sensor timed out (unexpected)"),
    }

    // Pattern: retry with timeout, N attempts
    let result = try_with_retries(3, Duration::from_millis(30)).await;
    println!("  After retries: {:?}", result);
    println!();
}

// Retry helper: try N times, each with its own timeout.
// After each failure, wait a bit before retrying.
async fn try_with_retries(attempts: u32, per_attempt_timeout: Duration) -> Result<f32, &'static str> {
    // Simulates a flaky sensor: fails first two times, succeeds on third
    static CALL_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    for attempt in 1..=attempts {
        let count = CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let future = async move {
            // First 2 calls are slow (timeout), third is fast
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
                println!("  Attempt {attempt}: success ({val:.1})");
                return Ok(val);
            }
            Err(_) => {
                println!("  Attempt {attempt}: timed out");
                if attempt < attempts {
                    tokio::time::sleep(Duration::from_millis(5)).await; // back-off
                }
            }
        }
    }

    Err("All attempts timed out")
}

// ─────────────────────────────────────────────────────────────────────────────
// Part 3: Biased Select for Priority
//
// By default, when multiple branches are ready simultaneously, select! picks
// one at random. This is fair, but sometimes you need priority:
// "always process shutdown commands before regular work."
//
// The `biased` keyword makes select! check branches top-to-bottom.
// If the first branch is ready, it always wins regardless of others.
//
// Real use case: housekeeping telemetry vs telecommand processing.
// In space systems, TCs have priority over HK — biased select expresses this.
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_biased_select() {
    println!("--- Part 3: Biased Select (Priority) ---");

    let (tc_tx, mut tc_rx) = mpsc::channel::<&str>(8); // High-priority telecommands
    let (hk_tx, mut hk_rx) = mpsc::channel::<&str>(8); // Low-priority housekeeping

    // Flood both channels with messages
    for i in 0..3 {
        tc_tx.send(format!("TC-{i}").leak()).await.unwrap();
        hk_tx.send(format!("HK-{i}").leak()).await.unwrap();
    }

    // Process for a few iterations, showing TC gets priority
    for _ in 0..6 {
        tokio::select! {
            // `biased` makes this deterministic: branches checked top-to-bottom.
            // The TC branch is checked FIRST. If a TC is waiting AND an HK is
            // waiting, the TC always wins. Only when TC queue is empty does
            // HK get processed.
            biased;

            // Priority 1: Process telecommand
            tc = tc_rx.recv() => {
                if let Some(cmd) = tc {
                    println!("  [HIGH PRIORITY] Processed TC: {cmd}");
                } else {
                    break;
                }
            }

            // Priority 2: Process housekeeping (only when no TC pending)
            hk = hk_rx.recv() => {
                if let Some(pkt) = hk {
                    println!("  [LOW PRIORITY]  Processed HK: {pkt}");
                } else {
                    break;
                }
            }

            // Always check shutdown (but lower priority than TC)
            else => break,
        }
    }

    println!("  Notice: all TCs processed before any HK (biased ordering)\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Part 4: Cancellation Safety
//
// When a branch loses the race in select!, the future in that branch is
// DROPPED — which means the async operation is cancelled mid-flight.
//
// This is only safe if the future is "cancellation-safe":
// it doesn't leave any external resource in an inconsistent state when dropped.
//
// Safe in select!:
// - channel recv() (message stays in channel)
// - CancellationToken::cancelled()
// - tokio::time::sleep()
//
// NOT safe in select! without care:
// - Writing to a file (may be partially written)
// - A multi-step protocol where you've sent but not yet received acknowledgment
//
// The fix for unsafe futures: use a flag or wrapper that completes atomically.
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_cancellation_safety() {
    println!("--- Part 4: Cancellation Safety ---");

    let (tx, mut rx) = mpsc::channel::<u32>(4);

    // Demonstrate that recv() is safe: if the branch is cancelled mid-wait,
    // the message is NOT lost — it stays in the channel for next time.
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.send(42).await.unwrap();
    });

    let mut received = false;
    let mut iterations = 0;

    // Run a select loop where the timer fires first for several iterations,
    // then eventually the channel message arrives. Because recv() is
    // cancellation-safe, the message is preserved across iterations.
    while !received && iterations < 20 {
        tokio::select! {
            val = rx.recv() => {
                println!("  Message received: {:?}", val);
                received = true;
            }
            _ = tokio::time::sleep(Duration::from_millis(10)) => {
                // The recv() future was dropped (cancelled) here each iteration.
                // But the message is still waiting in the channel buffer.
                iterations += 1;
                println!("  Timer fired (iter {iterations}), recv future cancelled, message preserved");
            }
        }
    }

    // Example of what NOT to do: unsafe future in select!
    // Conceptually (not run here — just a code comment):
    //
    //   select! {
    //       _ = write_config_to_flash() => {}  // DANGEROUS: may be partially written
    //       _ = shutdown_signal() => {}         // if this wins, flash write is cancelled!
    //   }
    //
    // The fix: wrap the operation so it either completes fully or not at all,
    // or use tokio::task::spawn() for the unsafe part and join the handle.

    println!("  (Cancellation safety demonstration complete)\n");
}
