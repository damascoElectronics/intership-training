//! Exercise 1 — Solution: Fully documented temperature sensor API

#![deny(missing_docs)]

//! This example demonstrates complete rustdoc for a hardware sensor driver.

/// A Linux sysfs temperature sensor driver.
///
/// Reads from a sysfs file that returns temperature in millidegrees Celsius,
/// applies a calibration offset, and validates the result.
///
/// # Examples
/// ```no_run
/// use std::io::Write;
/// // Pretend we have a real sysfs file at /sys/class/thermal/thermal_zone0/temp
/// let sensor = TempSensor::new("/sys/class/thermal/thermal_zone0/temp");
/// // In testing, use a temp file:
/// ```
pub struct TempSensor {
    /// Filesystem path to the sysfs temperature attribute (e.g. `/sys/class/thermal/thermal_zone0/temp`).
    pub device_path: String,
    /// Calibration offset in millidegrees Celsius, added to every reading.
    /// Use a negative value to compensate for sensor self-heating.
    pub calibration_offset_mc: i32,
}

/// Errors that can occur when reading from a [`TempSensor`].
#[derive(Debug)]
pub enum TempError {
    /// The sysfs device file does not exist or cannot be opened.
    DeviceNotFound,
    /// The file content could not be parsed as an integer.
    ReadError(String),
    /// The calibrated reading is outside the physically plausible range
    /// (−273 °C to 200 °C).
    OutOfRange {
        /// The out-of-range value in millidegrees Celsius.
        value: i32,
    },
}

impl TempSensor {
    /// Creates a new `TempSensor` for the given sysfs path with no calibration offset.
    ///
    /// # Examples
    /// ```
    /// let s = TempSensor::new("/tmp/fake_sensor");
    /// assert_eq!(s.calibration_offset_mc, 0);
    /// ```
    pub fn new(device_path: &str) -> Self {
        Self { device_path: device_path.to_owned(), calibration_offset_mc: 0 }
    }

    /// Sets the calibration offset and returns `self` (builder pattern).
    ///
    /// A positive offset raises readings; a negative offset lowers them.
    ///
    /// # Examples
    /// ```
    /// let s = TempSensor::new("/tmp/x").with_calibration(-500);
    /// assert_eq!(s.calibration_offset_mc, -500);
    /// ```
    pub fn with_calibration(mut self, offset_mc: i32) -> Self {
        self.calibration_offset_mc = offset_mc;
        self
    }

    /// Reads the temperature in millidegrees Celsius with calibration applied.
    ///
    /// # Errors
    /// - [`TempError::DeviceNotFound`] if the sysfs file cannot be read.
    /// - [`TempError::ReadError`] if the file content is not a valid integer.
    /// - [`TempError::OutOfRange`] if the calibrated value is outside
    ///   −273 000 mc (absolute zero) to 200 000 mc (beyond any normal sensor range).
    ///
    /// # Examples
    /// ```
    /// use std::io::Write;
    /// let path = "/tmp/test_sensor_mc";
    /// std::fs::write(path, "25000\n").unwrap();
    /// let sensor = TempSensor::new(path).with_calibration(500);
    /// assert_eq!(sensor.read_mc().unwrap(), 25500);
    /// ```
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

    /// Reads the temperature as degrees Celsius (floating point).
    ///
    /// This is a convenience wrapper around [`Self::read_mc`].
    ///
    /// # Errors
    /// Same as [`Self::read_mc`].
    ///
    /// # Examples
    /// ```
    /// let path = "/tmp/test_sensor_c";
    /// std::fs::write(path, "23500\n").unwrap();
    /// let sensor = TempSensor::new(path);
    /// let celsius = sensor.read_celsius().unwrap();
    /// assert!((celsius - 23.5).abs() < 0.001);
    /// ```
    pub fn read_celsius(&self) -> Result<f64, TempError> {
        Ok(self.read_mc()? as f64 / 1000.0)
    }
}

impl std::fmt::Display for TempError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceNotFound => write!(f, "device not found"),
            Self::ReadError(e) => write!(f, "read error: {e}"),
            Self::OutOfRange { value } => write!(f, "value {value} mc out of valid range"),
        }
    }
}

fn main() {
    println!("Build docs: cargo doc --example ex1_document_api_sol --open");
}
