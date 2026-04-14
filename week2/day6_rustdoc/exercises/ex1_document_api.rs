//! Exercise 1 — Add complete rustdoc to an undocumented API
//!
//! The module below is a fake "temperature sensor driver" with no documentation.
//! Your task: add rustdoc to EVERY public item, including doc tests.
//!
//! Requirements:
//!   - Every pub fn/struct/enum must have a /// comment
//!   - Include # Examples section with a runnable doc test
//!   - Include # Errors for functions returning Result
//!   - Include # Panics where applicable
//!   - Add #![deny(missing_docs)] and fix any remaining violations
//!
//! Build docs: cargo doc --example ex1_document_api --open
//! Run tests:  cargo test --example ex1_document_api

// TODO: add #![deny(missing_docs)] here once all items are documented

pub struct TempSensor {
    pub device_path: String,
    pub calibration_offset_mc: i32,
}

#[derive(Debug)]
pub enum TempError {
    DeviceNotFound,
    ReadError(String),
    OutOfRange { value: i32 },
}

impl TempSensor {
    pub fn new(device_path: &str) -> Self {
        Self { device_path: device_path.to_owned(), calibration_offset_mc: 0 }
    }

    pub fn with_calibration(mut self, offset_mc: i32) -> Self {
        self.calibration_offset_mc = offset_mc;
        self
    }

    pub fn read_mc(&self) -> Result<i32, TempError> {
        match std::fs::read_to_string(&self.device_path) {
            Ok(s) => {
                let raw: i32 = s.trim().parse()
                    .map_err(|e: std::num::ParseIntError| TempError::ReadError(e.to_string()))?;
                let calibrated = raw + self.calibration_offset_mc;
                if calibrated < -273_000 || calibrated > 200_000 {
                    return Err(TempError::OutOfRange { value: calibrated });
                }
                Ok(calibrated)
            }
            Err(_) => Err(TempError::DeviceNotFound),
        }
    }

    pub fn read_celsius(&self) -> Result<f64, TempError> {
        Ok(self.read_mc()? as f64 / 1000.0)
    }
}

impl std::fmt::Display for TempError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceNotFound => write!(f, "device not found"),
            Self::ReadError(e) => write!(f, "read error: {e}"),
            Self::OutOfRange { value } => write!(f, "value out of range: {value} mc"),
        }
    }
}

fn main() {
    println!("Build docs: cargo doc --example ex1_document_api --open");
    println!("Run tests:  cargo test --example ex1_document_api");
}
