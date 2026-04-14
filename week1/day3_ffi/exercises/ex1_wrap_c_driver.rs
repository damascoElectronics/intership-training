//! Exercise 1 — Wrap a C hardware driver in a safe Rust API
//!
//! Given a fake C ADC (Analog-to-Digital Converter) driver, implement a safe
//! Rust RAII wrapper that handles the open/close lifecycle automatically.
//!
//! Run tests:  cargo test --example ex1_wrap_c_driver

#![allow(dead_code, unused_variables)]

use std::ffi::CString;
use std::os::raw::{c_char, c_int};

// ── Fake C driver declarations ────────────────────────────────────────────────
// In a real project these would come from a C library compiled via build.rs.
// Here we provide Rust stubs that simulate the C interface.

/// Simulated C ADC driver (normally declared in a C header file).
mod ffi {
    use std::os::raw::{c_char, c_int};
    use std::sync::atomic::{AtomicBool, Ordering};

    static OPEN: AtomicBool = AtomicBool::new(false);

    /// Opens the ADC device. Returns a handle (>0) or -1 on error.
    pub unsafe extern "C" fn adc_open(device_path: *const c_char) -> c_int {
        if device_path.is_null() { return -1; }
        let path = unsafe { std::ffi::CStr::from_ptr(device_path) }.to_str().unwrap_or("");
        if path == "/dev/adc0" {
            OPEN.store(true, Ordering::SeqCst);
            42 // fake file descriptor
        } else {
            -1
        }
    }

    /// Reads one ADC sample. Returns 0 on success, fills *value. Returns -1 on error.
    pub unsafe extern "C" fn adc_read(handle: c_int, value: *mut u16) -> c_int {
        if handle != 42 || value.is_null() { return -1; }
        unsafe { *value = 2048; } // midscale 12-bit value
        0
    }

    /// Closes the ADC handle.
    pub unsafe extern "C" fn adc_close(handle: c_int) {
        if handle == 42 {
            OPEN.store(false, Ordering::SeqCst);
        }
    }
}

// ── Your implementation ───────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AdcError {
    #[error("failed to open ADC device: {path}")]
    OpenFailed { path: String },
    #[error("read failed")]
    ReadFailed,
    #[error("invalid device path contains null byte")]
    NullByte(#[from] std::ffi::NulError),
}

/// A safe RAII handle to a C ADC driver.
///
/// The device is closed automatically when this struct is dropped.
pub struct AdcHandle {
    // TODO: add a field to store the raw C file descriptor (i32)
    // TODO: add a field to store the device path (for error messages)
    _private: (), // remove this when you add your fields
}

impl AdcHandle {
    /// Opens the ADC device at `path`.
    ///
    /// # Errors
    /// Returns [`AdcError::OpenFailed`] if the C `adc_open` call returns -1.
    pub fn open(path: &str) -> Result<Self, AdcError> {
        todo!(
            "1. Convert path to CString (returns Err on embedded null bytes)
             2. Call ffi::adc_open(cstr.as_ptr()) in an unsafe block
             3. If result < 0: return Err(AdcError::OpenFailed)
             4. Return Ok(Self {{ handle: result, path: path.to_owned() }})"
        )
    }

    /// Reads one ADC sample (12-bit, 0–4095).
    pub fn read(&self) -> Result<u16, AdcError> {
        todo!(
            "1. Declare: let mut value: u16 = 0;
             2. Call ffi::adc_read(self.handle, &mut value as *mut u16) in unsafe
             3. If result != 0: return Err(AdcError::ReadFailed)
             4. Return Ok(value)"
        )
    }
}

impl Drop for AdcHandle {
    fn drop(&mut self) {
        // TODO: call ffi::adc_close(self.handle) in unsafe
        todo!("close the handle")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_valid_device() {
        let handle = AdcHandle::open("/dev/adc0").expect("should open");
        let value = handle.read().expect("should read");
        assert_eq!(value, 2048); // our fake ADC always returns midscale
    }

    #[test]
    fn open_invalid_device_returns_err() {
        let result = AdcHandle::open("/dev/nonexistent");
        assert!(result.is_err());
        assert!(matches!(result, Err(AdcError::OpenFailed { .. })));
    }

    #[test]
    fn raii_drop_closes_handle() {
        // After drop, opening the same device should succeed again
        // (our fake driver tracks open state)
        {
            let _handle = AdcHandle::open("/dev/adc0").unwrap();
        } // dropped here
        // Should be able to open again
        let _handle2 = AdcHandle::open("/dev/adc0").expect("should reopen after drop");
    }
}

fn main() {
    println!("Run tests with: cargo test --example ex1_wrap_c_driver");
}
