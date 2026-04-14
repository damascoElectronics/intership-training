//! Example 03 — Character devices and ioctl
//!
//! Beyond sysfs, Linux hardware drivers expose themselves as character devices
//! (files in /dev/).  You interact with them via read()/write()/ioctl() system
//! calls.  `ioctl` is the Swiss Army knife: it's a catch-all for device-
//! specific configuration that doesn't fit the file metaphor.
//!
//! This example shows the pattern using the RTC (real-time clock) device
//! available on most Linux systems at /dev/rtc0.
//!
//! Run with:  cargo run --example 03_chardev_ioctl

use nix::ioctl_read;
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

// RTC ioctl: read the current time from the hardware real-time clock.
// The ioctl number is constructed from type (0x70='p') and nr (0x09).
// nix::ioctl_read! generates a safe wrapper.
//
// struct rtc_time matches the kernel's struct rtc_time layout.
#[repr(C)]
#[derive(Default, Debug)]
struct RtcTime {
    tm_sec:   i32,
    tm_min:   i32,
    tm_hour:  i32,
    tm_mday:  i32,
    tm_mon:   i32,   // 0-based (January = 0)
    tm_year:  i32,   // years since 1900
    tm_wday:  i32,
    tm_yday:  i32,
    tm_isdst: i32,
}

// Generate: unsafe fn rtc_rd_time(fd: RawFd, data: *mut RtcTime) -> Result<i32>
ioctl_read!(rtc_rd_time, 0x70, 0x09, RtcTime);

fn main() {
    println!("=== Character device ioctl ===\n");

    // Open /dev/rtc0 in read-only mode, without O_NONBLOCK (blocking is fine here)
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_RDONLY)
        .open("/dev/rtc0");

    match file {
        Ok(f) => {
            let fd = f.as_raw_fd();
            let mut time = RtcTime::default();

            // Call the ioctl — this is unsafe because:
            // 1. We're calling an FFI function (a kernel interface)
            // 2. We're passing a raw pointer to `time`
            // The SAFETY contract: `time` is valid, aligned, and has the correct
            // layout expected by the kernel struct rtc_time.
            match unsafe { rtc_rd_time(fd, &mut time) } {
                Ok(_) => {
                    println!("Hardware RTC time:");
                    println!("  {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                        time.tm_year + 1900,
                        time.tm_mon + 1,
                        time.tm_mday,
                        time.tm_hour,
                        time.tm_min,
                        time.tm_sec,
                    );
                }
                Err(e) => println!("ioctl failed: {e}"),
            }
        }
        Err(e) => {
            println!("Cannot open /dev/rtc0: {e}");
            println!("(May need to: sudo chmod a+r /dev/rtc0)");
            println!();
            println!("The ioctl pattern regardless:");
            println!("  1. Define layout: #[repr(C)] struct matching kernel struct");
            println!("  2. Generate wrapper: nix::ioctl_read! / ioctl_write! / ioctl_readwrite!");
            println!("  3. Open the /dev file");
            println!("  4. Call ioctl in an unsafe block with SAFETY comment");
        }
    }

    println!();
    println!("When you'd use this vs tokio-serial:");
    println!("  tokio-serial: standard UART/serial ports (termios-based)");
    println!("  ioctl directly: custom kernel drivers that expose /dev/mydevice");
    println!("    - SPI devices not handled by spidev");
    println!("    - Custom FPGA register access via UIO driver");
    println!("    - Proprietary hardware with bespoke kernel modules");
}

// Need libc for O_RDONLY constant
use std::os::raw::c_int;
mod libc {
    pub const O_RDONLY: super::c_int = 0;
}
