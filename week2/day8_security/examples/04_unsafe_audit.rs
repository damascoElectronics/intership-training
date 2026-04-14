//! Example 04 — The unsafe audit: 5 categories with SAFETY contracts
//!
//! `unsafe` doesn't mean "dangerous code" — it means "code where the
//! programmer takes responsibility for invariants the compiler can't verify."
//!
//! For aerospace code review, every `unsafe` block MUST have a SAFETY comment
//! explaining what invariant holds and why.
//!
//! Run with:  cargo run --example 04_unsafe_audit

#![allow(unused_unsafe)]

use std::sync::atomic::{AtomicU32, Ordering};

fn main() {
    println!("=== The 5 Categories of unsafe ===\n");
    demo_raw_pointer();
    demo_ffi_call();
    demo_static_mut();
    demo_unsafe_trait();
    println!("All examples ran without issues.");
    println!("\nAudit checklist for every unsafe block:");
    println!("  1. What invariant must hold? (memory validity, aliasing, init)");
    println!("  2. What enforces that invariant? (type system, API contract, review)");
    println!("  3. What would break the invariant? (document it)");
    println!("  4. Is there a safe alternative? (prefer it if so)");
}

// ── Category 1: Raw pointer dereference ──────────────────────────────────────

fn demo_raw_pointer() {
    let mut value: u32 = 42;
    let ptr: *mut u32 = &mut value;

    // SAFETY: `ptr` was obtained from &mut value on the line above.
    // It points to a valid, properly aligned u32 that is live for the
    // duration of this function.  No other reference to `value` exists
    // while `ptr` is in use (borrow checker would normally prevent this,
    // but we're using raw pointers here deliberately to show the pattern).
    unsafe { *ptr = 100; }
    println!("Category 1 (raw ptr): value = {value}");
}

// ── Category 2: FFI call ──────────────────────────────────────────────────────

extern "C" {
    // Declaring a C function that doesn't actually exist — we link against libc
    // which has strlen. In a real project this would be your hardware driver.
    fn strlen(s: *const std::os::raw::c_char) -> usize;
}

fn demo_ffi_call() {
    let s = c"Hello, spacecraft";
    let len = unsafe {
        // SAFETY: `s.as_ptr()` points to a valid null-terminated C string literal.
        // The `c""` literal guarantees null termination and static lifetime.
        // strlen only reads memory; it does not write or free.
        strlen(s.as_ptr())
    };
    println!("Category 2 (FFI):     strlen = {len}");
}

// ── Category 3: static mut ────────────────────────────────────────────────────
// static mut is the most dangerous category — data races are UB.
// Prefer AtomicXxx or Mutex for shared state.

static COUNTER_UNSAFE: AtomicU32 = AtomicU32::new(0);

// If we absolutely must use static mut (e.g., no-std with no allocator):
static mut INIT_BUFFER: [u8; 64] = [0u8; 64];

fn demo_static_mut() {
    // Safe alternative: use atomics (no unsafe needed)
    COUNTER_UNSAFE.fetch_add(1, Ordering::SeqCst);
    println!("Category 3 (static):  counter = {}", COUNTER_UNSAFE.load(Ordering::SeqCst));

    // When static mut is unavoidable (single-threaded init):
    // SAFETY: This function is only called once, during initialization,
    // before any other threads start.  No other code accesses INIT_BUFFER
    // concurrently.  After init, INIT_BUFFER is read-only.
    unsafe {
        INIT_BUFFER[0] = 0xA5; // classic embedded "stack painted" value
    }
    let first = unsafe { INIT_BUFFER[0] };
    println!("Category 3 (static):  INIT_BUFFER[0] = 0x{first:02X}");
}

// ── Category 4: Implementing an unsafe trait ─────────────────────────────────

/// A trait that promises the type can be safely sent across an FFI boundary.
///
/// # Safety
/// The implementor guarantees the type has `#[repr(C)]` layout and contains
/// no Rust-specific types (references, Box, Vec, etc.) that would be invalid
/// in C.
unsafe trait FfiSafe {}

#[repr(C)]
struct SensorReading {
    timestamp_ms: u64,
    temperature_mc: i32,
    pressure_pa: u32,
}

// SAFETY: SensorReading is #[repr(C)] and contains only primitive integers.
// It can be safely passed across the FFI boundary.
unsafe impl FfiSafe for SensorReading {}

fn demo_unsafe_trait() {
    let r = SensorReading { timestamp_ms: 1000, temperature_mc: 25_000, pressure_pa: 101325 };
    println!("Category 4 (trait):   SensorReading {{ ts={}, temp={}mc, p={}Pa }}",
             r.timestamp_ms, r.temperature_mc, r.pressure_pa);
}
