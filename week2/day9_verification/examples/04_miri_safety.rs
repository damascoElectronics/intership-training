//! Example 04 — What Miri catches (and what it can't)
//!
//! Miri is a Rust interpreter that detects Undefined Behaviour at runtime.
//! It catches things the compiler cannot prove safe at compile time:
//!   - Reading uninitialized memory
//!   - Use-after-free
//!   - Out-of-bounds pointer arithmetic
//!   - Invalid pointer provenance
//!
//! Install and run:
//!   rustup component add miri
//!   cargo +nightly miri run --example 04_miri_safety
//!
//! Miri will find the UB in the UNSAFE section and report it clearly.
//! The SAFE section demonstrates the correct patterns.

fn main() {
    println!("=== Miri Safety Patterns ===\n");

    safe_patterns();
    println!();
    println!("To check for UB in unsafe code, run under Miri:");
    println!("  cargo +nightly miri run --example 04_miri_safety");
    println!("  cargo +nightly miri test");
}

fn safe_patterns() {
    // ── Pattern 1: bounds checking ────────────────────────────────────────────
    let data = vec![1u8, 2, 3, 4, 5, 6];

    // Safe: Rust checks bounds at runtime
    let slice = &data[1..4];
    println!("Safe slice: {slice:?}");

    // Also safe: get() returns Option
    let item = data.get(10);
    println!("Out-of-bounds get(): {item:?} (None, not UB)");

    // ── Pattern 2: initialized memory ─────────────────────────────────────────
    // Safe: MaybeUninit for manual initialization
    use std::mem::MaybeUninit;
    let mut buffer: [MaybeUninit<u8>; 4] = MaybeUninit::uninit_array();
    for (i, b) in buffer.iter_mut().enumerate() {
        b.write(i as u8 * 10);
    }
    // SAFETY: all elements were initialized above
    let initialized: [u8; 4] = unsafe { MaybeUninit::array_assume_init(buffer) };
    println!("Properly initialized: {initialized:?}");

    // ── Pattern 3: pointer provenance ─────────────────────────────────────────
    // Safe: derive pointer from a valid reference
    let mut value: u32 = 42;
    let ptr: *mut u32 = &mut value;
    // SAFETY: ptr was derived from &mut value which is valid and aligned
    unsafe { *ptr = 100; }
    println!("Pointer write via valid provenance: {value}");

    // ── Pattern 4: FFI buffer passing ─────────────────────────────────────────
    // Safe pattern: pass pointer + length together, don't compute offsets manually
    let mut out_buf = vec![0u8; 16];
    fill_buffer_safe(out_buf.as_mut_ptr(), out_buf.len());
    println!("Buffer filled via safe FFI pattern: {out_buf:?}");
}

/// Safe FFI-style buffer fill: receives ptr + len, stays within bounds.
///
/// # Safety
/// `ptr` must be valid for `len` writes, aligned to u8 (trivially true).
unsafe fn fill_buffer_safe(ptr: *mut u8, len: usize) {
    for i in 0..len {
        // SAFETY: i < len, so ptr.add(i) is within the allocated buffer
        unsafe { ptr.add(i).write(i as u8); }
    }
}

// ── What Miri would catch (commented out to avoid actually running UB) ─────────

/*
fn ub_examples() {
    // ❌ UB 1: reading uninitialized memory
    let x: u32 = unsafe { std::mem::uninitialized() };
    println!("{x}"); // Miri: "using uninitialized data"

    // ❌ UB 2: out-of-bounds access
    let v = vec![1u8, 2, 3];
    let ptr = v.as_ptr();
    let _bad = unsafe { *ptr.add(10) }; // Miri: "out-of-bounds pointer"

    // ❌ UB 3: use after free
    let b = Box::new(42u32);
    let ptr = Box::into_raw(b);
    drop(unsafe { Box::from_raw(ptr) }); // free
    let _bad = unsafe { *ptr };          // Miri: "use-after-free"
}
*/
