//! Exercise 1 — Solution

#![allow(dead_code)]

use std::ffi::CString;
use std::os::raw::{c_char, c_int};

mod ffi {
    use std::os::raw::{c_char, c_int};
    use std::sync::atomic::{AtomicBool, Ordering};
    static OPEN: AtomicBool = AtomicBool::new(false);

    pub unsafe extern "C" fn adc_open(device_path: *const c_char) -> c_int {
        if device_path.is_null() { return -1; }
        let path = unsafe { std::ffi::CStr::from_ptr(device_path) }.to_str().unwrap_or("");
        if path == "/dev/adc0" { OPEN.store(true, Ordering::SeqCst); 42 } else { -1 }
    }

    pub unsafe extern "C" fn adc_read(handle: c_int, value: *mut u16) -> c_int {
        if handle != 42 || value.is_null() { return -1; }
        unsafe { *value = 2048; }
        0
    }

    pub unsafe extern "C" fn adc_close(handle: c_int) {
        if handle == 42 { OPEN.store(false, Ordering::SeqCst); }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AdcError {
    #[error("failed to open ADC device: {path}")]
    OpenFailed { path: String },
    #[error("read failed")]
    ReadFailed,
    #[error("invalid device path: {0}")]
    NullByte(#[from] std::ffi::NulError),
}

pub struct AdcHandle {
    handle: c_int,
    path: String,
}

impl AdcHandle {
    pub fn open(path: &str) -> Result<Self, AdcError> {
        let cpath = CString::new(path)?; // propagates NulError
        let fd = unsafe { ffi::adc_open(cpath.as_ptr()) };
        if fd < 0 {
            return Err(AdcError::OpenFailed { path: path.to_owned() });
        }
        Ok(Self { handle: fd, path: path.to_owned() })
    }

    pub fn read(&self) -> Result<u16, AdcError> {
        let mut value: u16 = 0;
        let ret = unsafe { ffi::adc_read(self.handle, &mut value as *mut u16) };
        if ret != 0 { return Err(AdcError::ReadFailed); }
        Ok(value)
    }
}

impl Drop for AdcHandle {
    fn drop(&mut self) {
        // SAFETY: self.handle is a valid handle obtained from adc_open.
        // We call this exactly once (in Drop), so no double-close.
        unsafe { ffi::adc_close(self.handle); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_valid_device() {
        let handle = AdcHandle::open("/dev/adc0").expect("should open");
        assert_eq!(handle.read().unwrap(), 2048);
    }

    #[test]
    fn open_invalid_device_returns_err() {
        assert!(matches!(AdcHandle::open("/dev/nonexistent"), Err(AdcError::OpenFailed { .. })));
    }

    #[test]
    fn raii_drop_closes_handle() {
        { let _h = AdcHandle::open("/dev/adc0").unwrap(); }
        let _h2 = AdcHandle::open("/dev/adc0").expect("reopen after drop");
    }
}

fn main() {
    println!("Run tests with: cargo test --example ex1_wrap_c_driver_sol");
}
