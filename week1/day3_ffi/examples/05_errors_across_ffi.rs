//! Example 05 — Error handling across the FFI boundary
//!
//! C functions return error codes (0/-1/errno). Rust wants Result<T, E>.
//! The bridge between them requires careful thought:
//!   1. Map C error codes to Rust error types
//!   2. Panics MUST NOT cross the FFI boundary (undefined behaviour)
//!   3. Use std::panic::catch_unwind in extern "C" callbacks
//!
//! Run with:  cargo run --example 05_errors_across_ffi

use std::panic;

// ── C error code → Rust Result ────────────────────────────────────────────────

/// Error codes returned by our C "sensor driver" (see ccsds_framer.c for context)
#[derive(Debug, thiserror::Error)]
pub enum FfiError {
    #[error("operation succeeded")]
    // Not actually an error, but shows the code-mapping pattern
    Success,
    #[error("invalid argument (C errno EINVAL)")]
    InvalidArgument,
    #[error("device not ready")]
    NotReady,
    #[error("unknown C error code: {0}")]
    Unknown(i32),
}

impl FfiError {
    pub fn from_c_code(code: i32) -> Result<(), Self> {
        match code {
            0 => Ok(()),
            -1 => Err(Self::InvalidArgument),
            -2 => Err(Self::NotReady),
            other => Err(Self::Unknown(other)),
        }
    }
}

/// Wrapper that maps a C return code to Result<(), FfiError>
fn call_c_function_safely(code: i32) -> Result<(), FfiError> {
    FfiError::from_c_code(code)
}

// ── catch_unwind at FFI boundary ──────────────────────────────────────────────

/// A Rust callback that will be called from C code.
/// ANY panic here would be UB — the C stack doesn't know about Rust panics.
/// We MUST catch it.
///
/// # Safety
/// Called from C; must not unwind.
#[no_mangle]
pub extern "C" fn rust_callback_safe(value: i32) -> i32 {
    // catch_unwind converts a panic into a Result, preventing it from crossing
    // the FFI boundary (which would be undefined behaviour).
    match panic::catch_unwind(|| {
        // Simulate code that might panic
        if value < 0 {
            panic!("negative value not allowed: {value}");
        }
        value * 2
    }) {
        Ok(result) => result,
        Err(_) => {
            // The panic was caught — return an error sentinel to C
            -1
        }
    }
}

// ── C string / null pointer safety ───────────────────────────────────────────

use std::ffi::CStr;
use std::os::raw::c_char;

/// Safely converts a C string pointer to a Rust &str.
/// Returns None if the pointer is null or the bytes aren't valid UTF-8.
///
/// # Safety
/// `ptr` must be null or point to a null-terminated byte string.
unsafe fn c_str_to_rust<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: caller guarantees ptr is valid and null-terminated
    let cstr = unsafe { CStr::from_ptr(ptr) };
    cstr.to_str().ok()
}

fn main() {
    println!("=== Error handling across FFI ===\n");

    // 1. Map C error codes to Rust Results
    println!("C error code → Rust Result:");
    for code in [0, -1, -2, -99] {
        match call_c_function_safely(code) {
            Ok(()) => println!("  code={code:3} → Ok(())"),
            Err(e) => println!("  code={code:3} → Err({e})"),
        }
    }

    // 2. Demonstrate panic catching at FFI boundary
    println!("\ncatch_unwind at FFI boundary:");
    for value in [5, -3, 10] {
        let result = rust_callback_safe(value);
        println!("  rust_callback_safe({value:3}) → {result}");
    }
    println!("  (negative values cause a panic internally, but it's caught — no UB)");

    // 3. Null pointer safety
    println!("\nC string safety:");
    let valid = c"Hello from C";
    let result = unsafe { c_str_to_rust(valid.as_ptr()) };
    println!("  valid C string:    {:?}", result);
    let result = unsafe { c_str_to_rust(std::ptr::null()) };
    println!("  null pointer:      {:?}", result);

    println!("\nKey rules:");
    println!("  1. C returns i32 error codes → map to Rust Result<T, E>");
    println!("  2. extern \"C\" functions → NEVER let panics propagate (UB)");
    println!("  3. Use std::panic::catch_unwind in callbacks called from C");
    println!("  4. Always check for null before dereferencing C pointers");
}
