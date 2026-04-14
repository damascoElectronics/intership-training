# Day 1: Async Foundations with Tokio

## Why Async? Bare-Metal vs. Linux Daemon Land

On your STM32 or ESP32 you probably used one of:
- A **superloop** (`while(1) { poll_uart(); poll_spi(); ... }`)
- An **RTOS** (FreeRTOS tasks + queues)
- **Interrupt handlers** waking tasks

The fundamental problem is the same everywhere: **you have N things that need attention, and you can only do one at a time on one CPU core.** You need a scheduler to multiplex them.

On bare metal you had two tools: interrupts (hardware pushes you) and polling (you pull). On Linux you get a third: **the OS kernel's I/O event system** (`epoll`). Tokio's async runtime is built on top of `epoll`, which means:

- No wasted CPU spinning in a tight poll loop
- The kernel wakes your thread *only* when data arrives
- A single OS thread can manage thousands of concurrent I/O operations
- You write sequential-looking code (`let data = socket.read().await`) that is actually cooperative multitasking under the hood

**Why not just use OS threads?** Each OS thread costs ~8 MB of stack by default. A daemon managing 100 sensor connections would use 800 MB just for stacks. Tokio tasks cost ~hundreds of bytes. The math is obvious.

---

## Tokio Runtime Internals

### The Big Picture

```
Your code (async fns + .await points)
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│                   Tokio Runtime                         │
│                                                         │
│  ┌──────────────┐    ┌──────────────┐                  │
│  │  Worker      │    │  Worker      │  ← OS threads    │
│  │  Thread 0    │    │  Thread 1    │    (default:      │
│  │              │    │              │     num_cpus)     │
│  │  [Task A]    │    │  [Task C]    │                  │
│  │  [Task B]    │    │  [Task D]    │                  │
│  └──────┬───────┘    └──────┬───────┘                  │
│         │                  │                            │
│         └────────┬─────────┘                           │
│                  │  work-stealing queue                 │
│                  ▼                                      │
│  ┌───────────────────────────────┐                     │
│  │         Reactor               │                     │
│  │  (mio library → epoll/kqueue) │                     │
│  │                               │                     │
│  │  Registered fds: socket A,    │                     │
│  │  socket B, timer C, pipe D... │                     │
│  └───────────────────────────────┘                     │
└─────────────────────────────────────────────────────────┘
         │
         ▼
    Linux kernel (epoll_wait)
```

### Work-Stealing Thread Pool

Tokio spawns N OS threads (default: number of CPU cores). Each thread has a **local run queue** of tasks. When a thread's queue is empty, it **steals** tasks from other threads' queues. This means:

- CPU work automatically spreads across cores without you thinking about it
- A task can run on different OS threads between `.await` points (this is why `Send` matters — see below)
- No thread is ever idle while there's work to do

### The Reactor (epoll Integration)

```
                    ┌─────────────────────────────┐
                    │  Your task calls             │
                    │  socket.read().await         │
                    └──────────┬──────────────────┘
                               │
                    ┌──────────▼──────────────────┐
                    │  Future::poll() returns      │
                    │  Poll::Pending               │
                    │  (no data available yet)     │
                    └──────────┬──────────────────┘
                               │ registers Waker with reactor
                    ┌──────────▼──────────────────┐
                    │  Reactor calls               │
                    │  epoll_ctl(ADD, fd, EPOLLIN) │
                    └──────────┬──────────────────┘
                               │ task is parked (uses 0 CPU)
                    ┌──────────▼──────────────────┐
                    │  ... time passes ...         │
                    │  kernel receives TCP packet  │
                    └──────────┬──────────────────┘
                               │
                    ┌──────────▼──────────────────┐
                    │  epoll_wait() returns fd     │
                    │  Reactor calls waker.wake()  │
                    └──────────┬──────────────────┘
                               │ task re-queued
                    ┌──────────▼──────────────────┐
                    │  Future::poll() called again │
                    │  returns Poll::Ready(data)   │
                    └─────────────────────────────┘
```

This is the core of async Rust: `.await` is sugar for "poll this future; if it returns Pending, register a waker and give up the thread; when woken, poll again."

### What `#[tokio::main]` Actually Expands To

```rust
// What you write:
#[tokio::main]
async fn main() { ... }

// What the macro generates (approximately):
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { ... })  // runs your async main on the runtime
}
```

---

## `spawn` vs `spawn_blocking`: The Critical Distinction

This is where your embedded background might trip you up.

**In your RTOS world:** calling `HAL_SPI_Transmit()` blocks the calling task. Other RTOS tasks still run because the RTOS preemptively switches.

**In tokio:** calling a blocking function **blocks the entire OS thread**. That OS thread cannot run any other async task until the blocking call returns. If all worker threads are blocked, new I/O events sit unprocessed.

```
Worker Thread 0: [Task A ──────────────────────────────────────] blocked!
                         ^ calls std::fs::read() here
                           cannot run Task B, C, D until it returns
```

The rule: **never call blocking functions inside async code.**

```rust
// BAD: blocks the worker thread
let data = std::fs::read("/dev/sda")?;   // DON'T
let _    = std::thread::sleep(dur);       // DON'T
let _    = mutex.lock().unwrap();         // careful with contended mutexes

// GOOD: runs blocking work on a dedicated blocking thread pool
let data = tokio::fs::read("/dev/sda").await?;    // async I/O
tokio::time::sleep(dur).await;                     // async sleep
tokio::task::spawn_blocking(|| heavy_cpu_work()).await?;  // offload blocking
```

`spawn_blocking` moves the closure to a separate thread pool that can grow unboundedly (up to 512 threads by default). Those threads are *allowed* to block — they're not async worker threads.

```
Async Worker Pool (fixed, e.g. 8 threads):
  Worker 0:  Task A, Task B, Task C ...   ← async tasks, never block
  Worker 1:  Task D, Task E ...

Blocking Thread Pool (growable, up to 512):
  Blocking 0: std::fs::read(...)          ← can block all day
  Blocking 1: heavy_compression(...)
```

---

## `Send + Sync`: Why the Compiler Tracks This

Between two `.await` points, your task may be moved to a **different OS thread** by the work-stealing scheduler. This means everything your future holds across an `.await` must implement `Send` (safe to move between threads).

```rust
// This DOES NOT compile:
let rc = std::rc::Rc::new(42);   // Rc is !Send (not thread-safe ref count)
do_something().await;             // task could migrate threads here
println!("{}", rc);               // rc is still held, but we might be on a new thread!

// Use Arc instead:
let arc = std::sync::Arc::new(42);  // Arc is Send (atomic ref count)
do_something().await;
println!("{}", arc);                 // fine: Arc can cross thread boundaries
```

The Rust compiler **statically verifies** `Send` bounds at compile time. It's doing the thread-safety analysis that you'd do manually in C, and refusing to compile if it finds a violation.

---

## Structured Concurrency and Task Cancellation

**Unstructured:** fire tasks and hope they finish (the old threading model)
```
main spawns task A → task A spawns task B → main exits → task B keeps running?
```

**Structured:** tasks form a tree; parent outlives children; cancellation propagates
```
main
 ├── task A (heartbeat sender)
 │    └── cancelled when token drops
 └── task B (telemetry collector)
      └── cancelled when token drops
```

`CancellationToken` from `tokio-util` is the idiomatic tool:
```rust
let token = CancellationToken::new();

// Give a clone to each child task
let child_token = token.child_token(); // child cancel propagates from parent
tokio::spawn(async move {
    tokio::select! {
        _ = child_token.cancelled() => { /* clean up and exit */ }
        _ = do_work() => {}
    }
});

// Later, cancel everything
token.cancel();  // all child tokens are also cancelled
```

---

## Graceful Shutdown Pattern

Real daemons need to handle `SIGTERM` (systemd stopping the service) and `SIGINT` (Ctrl+C during development). The pattern:

1. Install signal handler
2. Broadcast cancellation token
3. Wait for tasks to acknowledge (with timeout so we don't hang forever)
4. Flush any buffered state (telemetry, logs)
5. Exit

```
SIGTERM arrives
      │
      ▼
CancellationToken::cancel()
      │
      ├──→ Task A: select! sees cancelled(), sends final telemetry, returns
      ├──→ Task B: select! sees cancelled(), flushes buffer, returns
      └──→ Task C: select! sees cancelled(), closes device, returns
      │
      ▼ (or timeout after 5s if a task hangs)
tokio::join!(handle_a, handle_b, handle_c)
      │
      ▼
process exits cleanly (systemd sees clean exit, no restart)
```

---

## Running the Examples

```bash
# From week1/day1_async_foundations/
cargo run --example 01_basic_runtime
cargo run --example 02_spawn_tasks
cargo run --example 03_channels
cargo run --example 04_select_macro
cargo run --example 05_graceful_shutdown

# Exercises (try to implement before looking at solution)
cargo run --example ex1_heartbeat
cargo run --example ex1_heartbeat_sol

# Run tests
cargo test
```

---

## Key Takeaways

| Concept | Bare Metal Analog | Tokio Equivalent |
|---------|------------------|-----------------|
| Superloop polling | `while(1) { poll_all(); }` | `epoll_wait` in reactor |
| ISR waking a task | HAL interrupt → RTOS queue | `Waker::wake()` |
| RTOS task switch | preemptive context switch | `.await` yield point |
| Blocking HAL call | `HAL_SPI_Transmit()` | `spawn_blocking` |
| Mutex | `osMutexAcquire()` | `tokio::sync::Mutex` |
| Task notification | `osTaskNotify()` | `tokio::sync::Notify` |
| Message queue | `osMessageQueuePut()` | `tokio::sync::mpsc` |
