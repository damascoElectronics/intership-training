// Day 1, Example 2: Spawning Tasks with tokio::spawn
//
// In FreeRTOS you create tasks with xTaskCreate() and they run independently.
// tokio::spawn() is the equivalent: it creates a new concurrent async task.
//
// Key difference from FreeRTOS:
// - FreeRTOS tasks are stack-allocated, identified by a task handle
// - Tokio tasks are heap-allocated state machines, returned as JoinHandle<T>
// - JoinHandle lets you await the result or cancel the task
//
// Run with:
//   cargo run --example 02_spawn_tasks

use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    println!("=== Example 02: Spawning Tasks ===\n");

    // --- Part 1: Basic spawn and JoinHandle ---
    demo_basic_spawn().await;

    // --- Part 2: Awaiting task results ---
    demo_join_handle().await;

    // --- Part 3: CancellationToken for clean shutdown ---
    demo_cancellation().await;

    // --- Part 4: Panic propagation ---
    demo_panic_handling().await;

    println!("\nDone.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Part 1: Basic spawn
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_basic_spawn() {
    println!("--- Part 1: Basic spawn ---");

    // tokio::spawn() creates a NEW task that runs concurrently with the current
    // task. The current task does NOT pause — both tasks run at the same time
    // (or interleave on a single-threaded executor).
    //
    // The spawned closure must be 'static + Send:
    // - 'static: the task might outlive the current stack frame, so it can't
    //   borrow local variables (unless you move them in)
    // - Send: the task can be moved to any worker thread at any .await point
    let handle: JoinHandle<()> = tokio::spawn(async {
        // This runs concurrently with the spawning task
        tokio::time::sleep(Duration::from_millis(20)).await;
        println!("  Spawned task 1: woke up after 20ms");
    });

    let handle2: JoinHandle<()> = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        println!("  Spawned task 2: woke up after 10ms (runs first despite spawning second!)");
    });

    // .await on a JoinHandle waits for the task to complete.
    // Here we see task 2 finish before task 1 even though we spawned task 1 first —
    // because both run concurrently and task 2 has a shorter sleep.
    handle.await.expect("Task 1 panicked");
    handle2.await.expect("Task 2 panicked");

    println!("  Both tasks finished\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Part 2: Getting a return value from a spawned task
// ─────────────────────────────────────────────────────────────────────────────

async fn demo_join_handle() {
    println!("--- Part 2: JoinHandle return values ---");

    // JoinHandle<T> is generic over the task's return type.
    // Like a FreeRTOS task that writes its result to a shared variable,
    // but type-safe: you can't misinterpret the return type.
    let handle: JoinHandle<u64> = tokio::spawn(async {
        // Simulate a computation that takes some time (e.g., reading a sensor)
        tokio::time::sleep(Duration::from_millis(5)).await;
        let reading = 42u64;
        println!("  Worker computed sensor reading: {reading}");
        reading // Return value from the spawned task
    });

    // JoinHandle::await returns Result<T, JoinError>
    // - Ok(value): task completed normally, value is the return value
    // - Err(join_error): task panicked or was cancelled
    let result: Result<u64, tokio::task::JoinError> = handle.await;
    let value = result.expect("Worker task panicked");
    println!("  Main task received: {value}\n");

    // Run multiple tasks concurrently and wait for ALL of them.
    // tokio::join! is the macro version — drives all futures concurrently,
    // returns when ALL complete.
    let (a, b, c) = tokio::join!(
        tokio::spawn(async { compute(1).await }),
        tokio::spawn(async { compute(2).await }),
        tokio::spawn(async { compute(3).await }),
    );
    println!(
        "  Parallel results: {}, {}, {}",
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
// Part 3: CancellationToken — the idiomatic way to stop tasks
// ─────────────────────────────────────────────────────────────────────────────
//
// In FreeRTOS you might set a volatile global `bool keep_running = false` and
// have tasks check it in their loop. CancellationToken is the type-safe
// async-aware version of that pattern.
//
// It's shareable: clone it and give one clone to each task.
// It's composable: a child_token() cancels when either the child or parent cancels.

async fn demo_cancellation() {
    println!("--- Part 3: CancellationToken ---");

    // Create the root token. Cancelling this cancels all clones.
    let token = CancellationToken::new();

    // Clone the token to give to the spawned task.
    // The clone and original are linked — cancelling either one
    // does NOT cancel the other, but cancelling the ROOT token
    // does cancel child tokens created with .child_token().
    let task_token = token.clone();

    let handle = tokio::spawn(async move {
        println!("  Background task: starting polling loop");
        let mut count = 0u32;

        loop {
            // tokio::select! races multiple futures.
            // Whichever completes first wins; the others are dropped (cancelled).
            // Here we race: "did the token get cancelled?" vs "did the timer fire?"
            tokio::select! {
                // The cancellation branch: notice the token is "consumed" by this check.
                // cancelled() returns a future that resolves when cancel() is called.
                _ = task_token.cancelled() => {
                    println!("  Background task: cancellation received, stopping (count={count})");
                    break;
                }
                // The work branch: do one unit of work per 15ms interval
                _ = tokio::time::sleep(Duration::from_millis(15)) => {
                    count += 1;
                    println!("  Background task: tick {count}");
                }
            }
        }
    });

    // Let the task run for a bit
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Signal shutdown. All clones of `token` will now return from .cancelled().
    println!("  Main: cancelling token");
    token.cancel();

    // Wait for the task to acknowledge and exit
    handle.await.expect("Background task panicked");
    println!("  Task exited cleanly\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Part 4: What happens when a spawned task panics
// ─────────────────────────────────────────────────────────────────────────────
//
// Unlike FreeRTOS where a task panic (HardFault) might crash the whole system,
// tokio isolates task panics: the panic is caught at the task boundary and
// reported as a JoinError when you await the handle.
//
// This is important for daemon resilience: a panic in one task shouldn't
// bring down the entire daemon.

async fn demo_panic_handling() {
    println!("--- Part 4: Panic propagation ---");

    let handle = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(1)).await;
        // This panic is caught at the task boundary, not at the spawning site
        panic!("Simulated sensor driver panic!");
    });

    match handle.await {
        Ok(()) => println!("  Task completed normally"),
        Err(join_error) if join_error.is_panic() => {
            // The panic message is captured. We can log it and decide whether
            // to restart the task, alert operations, or shut down gracefully.
            println!("  Task panicked (caught at boundary): {join_error}");
            println!("  Daemon continues running — only this task died");
        }
        Err(join_error) => {
            // This branch handles task cancellation via handle.abort()
            println!("  Task was cancelled: {join_error}");
        }
    }

    // Show that aborting a task also produces a JoinError
    let handle = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(100)).await; // would run forever
        unreachable!("Should be aborted before here");
    });

    // Abort the task externally (like killing a FreeRTOS task with vTaskDelete)
    handle.abort();

    match handle.await {
        Ok(()) => println!("  Task completed normally (unlikely after abort)"),
        Err(e) if e.is_cancelled() => println!("  Task was aborted (as expected)\n"),
        Err(e) => println!("  Unexpected: {e}\n"),
    }
}
