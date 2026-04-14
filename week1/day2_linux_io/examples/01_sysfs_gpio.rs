//! Example 01 — GPIO via sysfs
//!
//! Linux exposes GPIO pins through the sysfs virtual filesystem at
//! /sys/class/gpio/.  This is the "legacy" interface (still widely used
//! on embedded Linux boards like Raspberry Pi, Zynq, etc.).
//!
//! The newer interface is the GPIO character device (/dev/gpiochipN) and
//! the libgpiod library, but sysfs is simpler to understand first.
//!
//! NOTE: This example does NOT require actual hardware — it shows the
//! pattern and wraps the operations in RAII so they are safe.  On a real
//! board you would have a physical pin attached.
//!
//! Run with:  cargo run --example 01_sysfs_gpio
//!            (will fail gracefully without root / without GPIO hardware)

use std::io;
use std::path::PathBuf;

/// RAII wrapper for a sysfs GPIO pin.
///
/// On `Drop`, the pin is unexported (released) so other processes can use it.
pub struct GpioPin {
    number: u32,
    base: PathBuf,
}

impl GpioPin {
    /// Exports (claims) a GPIO pin by writing its number to /sys/class/gpio/export.
    ///
    /// Returns `Err` if the pin is already exported or if you lack permission.
    /// You may need to run as root: `sudo ./target/debug/...`
    pub fn export(number: u32) -> io::Result<Self> {
        let export_path = PathBuf::from("/sys/class/gpio/export");
        std::fs::write(&export_path, number.to_string())?;

        // Small delay — sysfs entries take a moment to appear after export
        std::thread::sleep(std::time::Duration::from_millis(50));

        Ok(Self {
            number,
            base: PathBuf::from(format!("/sys/class/gpio/gpio{number}")),
        })
    }

    /// Sets the pin direction: "in" or "out".
    pub fn set_direction(&self, direction: &str) -> io::Result<()> {
        std::fs::write(self.base.join("direction"), direction)
    }

    /// Sets the output value: `true` = high (1), `false` = low (0).
    pub fn set_value(&self, high: bool) -> io::Result<()> {
        std::fs::write(self.base.join("value"), if high { "1" } else { "0" })
    }

    /// Reads the current pin value.  Returns `true` if high.
    pub fn get_value(&self) -> io::Result<bool> {
        let contents = std::fs::read_to_string(self.base.join("value"))?;
        Ok(contents.trim() == "1")
    }

    /// Polls the value file repeatedly until it changes or timeout.
    ///
    /// This is a simple polling approach.  For edge-triggered events,
    /// epoll/inotify on the sysfs "value" file is more efficient.
    pub fn wait_for_change(&self, current: bool, timeout_ms: u64) -> io::Result<bool> {
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_millis(timeout_ms);
        loop {
            let now = self.get_value()?;
            if now != current { return Ok(now); }
            if std::time::Instant::now() >= deadline {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "timeout waiting for GPIO change"));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

impl Drop for GpioPin {
    fn drop(&mut self) {
        // Unexport the pin so other processes can use it.
        // Ignore errors: if we're already dead, this is best-effort.
        let _ = std::fs::write("/sys/class/gpio/unexport", self.number.to_string());
    }
}

fn main() {
    println!("=== GPIO via sysfs ===\n");

    // Try to export GPIO 17 (a common test pin on Raspberry Pi).
    // On systems without GPIO hardware or without root, this will fail
    // with a descriptive error.
    match GpioPin::export(17) {
        Ok(pin) => {
            println!("Exported GPIO 17");
            pin.set_direction("out").expect("set direction");
            println!("Set direction to 'out'");

            pin.set_value(true).expect("set value");
            println!("Set high (1)");
            let v = pin.get_value().expect("read value");
            println!("Read back: {}", if v { "HIGH" } else { "LOW" });

            pin.set_value(false).expect("set value");
            println!("Set low (0)");

            println!("\nPin will be unexported on drop (RAII).");
            // `pin` is dropped here → unexport happens automatically
        }
        Err(e) => {
            println!("Cannot export GPIO 17: {e}");
            println!("(This is expected without root or GPIO hardware.)");
            println!();
            println!("The RAII pattern and sysfs path structure are what matter:");
            println!("  /sys/class/gpio/export       ← write pin number to claim");
            println!("  /sys/class/gpio/gpioN/direction ← write 'in' or 'out'");
            println!("  /sys/class/gpio/gpioN/value     ← read/write '0' or '1'");
            println!("  /sys/class/gpio/unexport     ← write pin number to release");
        }
    }
}
