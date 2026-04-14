# Day 2 — Linux I/O

**Theme:** Reading hardware through Linux abstractions — sysfs, tty, character devices, procfs.

In embedded spacecraft software the "sensor" is rarely a register you own; it is a kernel-exported
file or device node. This day teaches you to drive those abstractions from async Rust without
blocking the executor.

---

## Learning Goals

- Read and write sysfs GPIO without any GPIO-specific crate (bare file I/O)
- Open and frame a serial port with `tokio-serial`
- Issue `ioctl` calls through `nix`
- Poll a sysfs value on a timer and emit structured events
- Parse `/proc` files for system telemetry

---

## Examples

| File | What it demonstrates |
|------|----------------------|
| `01_sysfs_gpio.rs` | RAII `GpioPin`, export/unexport, direction, value read/write |
| `02_tty_serial.rs` | `tokio_serial::new()`, baud rate, parity, async line-by-line read |
| `03_chardev_ioctl.rs` | `nix::ioctl_read!`, `#[repr(C)]` for the kernel struct, RTC example |
| `04_sysfs_poll.rs` | Interval-based polling, threshold comparison, typed alert events |
| `05_procfs_reader.rs` | Parse `/proc/uptime`, `/proc/self/status`, `/proc/loadavg`, `/proc/meminfo` |

Run any example (requires a Linux host):
```
cargo run -p day2-linux-io --example 05_procfs_reader
```

---

## Exercises

### Exercise 1 — UART Echo Daemon (`ex1_uart_echo.rs`)

Implement `run_echo_daemon(port: &str, baud: u32)` that:

1. Opens the serial port with `tokio-serial`
2. Reads lines with a `BufReader`
3. Echoes each line back with a `ECHO:` prefix

Solution: `ex1_uart_echo_sol.rs`

### Exercise 2 — Thermal Monitor (`ex2_sysfs_poll.rs`)

Implement `run_monitor(monitor: ThermalMonitor, tx: Sender<ThermalAlert>)` that:

1. Polls the sysfs thermal file at the given interval
2. Sends `ThermalAlert::Warning` when temperature exceeds the warning threshold
3. Sends `ThermalAlert::Critical` when it exceeds the critical threshold

The test harness uses temporary files in place of real sysfs paths — no hardware needed.

Solution: `ex2_sysfs_poll_sol.rs`

---

## Key Concepts

### Why sysfs?

The Linux kernel exports hardware state as a virtual filesystem under `/sys`. GPIO lines,
thermal sensors, power supplies, LEDs — all appear as files. This lets you interact with
hardware using the same file I/O APIs you already know, without kernel modules or C drivers.

### Blocking I/O on async executors

The golden rule: **never call a blocking syscall from within an async task without wrapping
it in `tokio::task::spawn_blocking`**. A blocked thread holds the executor thread, starving
every other task on that thread.

Exception: reads from sysfs are nearly instantaneous (the kernel returns immediately), so
short reads with `tokio::fs::read_to_string` are acceptable in practice. Avoid this shortcut
for real device nodes like `/dev/ttyUSBx` where the kernel may block waiting for data.

### ioctl safety

`nix::ioctl_read!` generates an `unsafe fn`. The SAFETY obligation is:
- The file descriptor must refer to the device type that understands this ioctl number.
- The output pointer must point to a valid, properly-sized, properly-aligned struct.
- The struct layout must exactly match what the kernel writes (`#[repr(C)]`).
