//! Example 02 — Async UART/Serial I/O with tokio-serial
//!
//! You already know UART from the hardware side (baud rate, parity, stop bits,
//! flow control).  This example shows how to configure and use a serial port
//! from Linux userspace with async Rust.
//!
//! The key is that tokio-serial wraps the file descriptor in tokio's async I/O
//! infrastructure (epoll on Linux), so .read()/.write() yield to the runtime
//! instead of blocking an OS thread.
//!
//! NOTE: Requires a real or virtual serial port.
//! Create a virtual pair with: socat -d -d pty,raw,echo=0 pty,raw,echo=0
//! Then use one of the reported /dev/pts/N paths as PORT_PATH.
//!
//! Run with:  cargo run --example 02_tty_serial

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits};

/// Change this to your actual serial port
const PORT_PATH: &str = "/dev/ttyUSB0";
const BAUD_RATE: u32 = 115200;

#[tokio::main]
async fn main() {
    println!("=== Async Serial I/O ===\n");
    println!("Port:      {PORT_PATH}");
    println!("Baud rate: {BAUD_RATE}");
    println!("Format:    8N1 (8 data bits, no parity, 1 stop bit)");
    println!();

    // Configure the serial port.
    // This mirrors what you'd do in HAL_UART_Init on an STM32 — but from
    // the Linux side of the wire.
    let port = tokio_serial::new(PORT_PATH, BAUD_RATE)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .open_native_async();

    let mut port = match port {
        Ok(p) => p,
        Err(e) => {
            println!("Cannot open {PORT_PATH}: {e}");
            println!();
            println!("Serial port configuration options:");
            println!("  Data bits:    tokio_serial::DataBits::{{Five,Six,Seven,Eight}}");
            println!("  Parity:       tokio_serial::Parity::{{None,Odd,Even}}");
            println!("  Stop bits:    tokio_serial::StopBits::{{One,Two}}");
            println!("  Flow control: tokio_serial::FlowControl::{{None,Software,Hardware}}");
            println!();
            println!("For testing without hardware, create a virtual pair:");
            println!("  socat -d -d pty,raw,echo=0 pty,raw,echo=0");
            println!("Then update PORT_PATH to one of the reported /dev/pts/N paths.");
            return;
        }
    };

    // Send a greeting
    port.write_all(b"Hello from Rust daemon!\n").await.expect("write");
    println!("Sent: 'Hello from Rust daemon!'");

    // Read response lines using a BufReader for line-by-line framing.
    // In practice you'd use a custom codec (see LengthDelimitedCodec in day4)
    // for binary protocols, but lines work well for ASCII debug channels.
    let reader = BufReader::new(port);
    let mut lines = reader.lines();

    println!("Waiting for responses (Ctrl+C to stop)...");
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => println!("Received: '{line}'"),
            Ok(None)       => { println!("Port closed."); break; }
            Err(e)         => { println!("Read error: {e}"); break; }
        }
    }
}
