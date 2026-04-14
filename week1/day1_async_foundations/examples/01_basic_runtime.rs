// Day 1, Example 1: Basic Tokio Runtime
//
// This example shows what #[tokio::main] actually does under the hood,
// and introduces the basic async fn / .await pattern.
//
// If you've used FreeRTOS: think of the tokio runtime as the RTOS kernel,
// and async functions as tasks. The key difference is that tokio tasks
// are *cooperative* — they yield control voluntarily at .await points,
// rather than being preempted by a timer interrupt.
//
// Run with:
//   cargo run --example 01_basic_runtime

// The #[tokio::main] attribute macro transforms our async fn main() into
// a regular fn main() that builds a Tokio runtime and blocks on it.
//
// Expanding the macro manually:
//
//   #[tokio::main]
//   async fn main() { ... }
//
// becomes approximately:
//
//   fn main() {
//       tokio::runtime::Builder::new_multi_thread()
//           .enable_all()   // enables both time and I/O drivers
//           .build()
//           .expect("Failed to build Tokio runtime")
//           .block_on(async {
//               // your async main body here
//           })
//   }
//
// `block_on` means: run this future to completion on the current OS thread,
// using this runtime's reactor to handle I/O events. It is a synchronous
// call — it does not return until the future completes.
//
// For single-threaded use (e.g., embedded-like environments, testing),
// you can use #[tokio::main(flavor = "current_thread")] which avoids
// spawning extra OS threads entirely.
#[tokio::main]
async fn main() {
    println!("=== Example 01: Basic Runtime ===\n");

    // Calling an async function does NOT run it immediately.
    // It returns a Future — a description of work to be done.
    // Only when you .await it does the runtime drive it to completion.
    //
    // This is analogous to configuring a DMA transfer on STM32:
    // HAL_SPI_Transmit_DMA() sets things up but doesn't block.
    // The transfer happens asynchronously; you get a callback or interrupt.
    // Here, .await is that "wait for completion" point.
    let result = compute_something(10).await;
    println!("compute_something(10) returned: {}", result);

    // async blocks create an anonymous future inline.
    // Useful for one-off async work without defining a named function.
    let inline_result = async {
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        42u32
    }
    .await;
    println!("Inline async block result: {}", inline_result);

    // Demonstrate that async functions compose naturally.
    // Each .await is a potential yield point where other tasks can run.
    // On bare metal this would be your cooperative scheduler yield.
    let total = add_async(compute_something(3).await, compute_something(7).await).await;
    println!("add_async(3+7 computed) = {}", total);

    // tokio::time::sleep yields the current task for the given duration.
    // Unlike std::thread::sleep, this does NOT block the OS thread —
    // other tasks continue running while we wait.
    // The reactor registers a timer and wakes us when it fires.
    println!("\nSleeping 50ms (non-blocking — other tasks could run)...");
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    println!("Woke up after 50ms");

    println!("\nDone.");
}

// An async function is syntactic sugar for a function returning
// impl Future<Output = T>. The compiler rewrites your sequential
// code into a state machine (similar to how you'd write a non-blocking
// state machine by hand in embedded C, but the compiler does it for you).
//
// Each .await point is a state transition: the future stores all the
// local variables it needs across that point, then suspends.
async fn compute_something(input: u32) -> u32 {
    // Simulate I/O latency (like waiting for a sensor response over UART).
    // In real code this would be actual async I/O, not a sleep.
    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;

    // The compiler captures `input` across the .await above —
    // it's stored in the state machine, not on the call stack.
    input * input
}

async fn add_async(a: u32, b: u32) -> u32 {
    // Not all async functions need to .await anything.
    // Being async just means the caller can .await us uniformly.
    a + b
}
