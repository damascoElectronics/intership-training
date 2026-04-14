//! Exercise 1 — Async UART echo daemon
//!
//! Implement an async daemon that echoes every line received over serial
//! with an "ECHO: " prefix.
//!
//! NOTE: Requires a virtual serial port pair:
//!   socat -d -d pty,raw,echo=0 pty,raw,echo=0
//! Use the two /dev/pts/N paths as PORT and the other end for testing.

#![allow(dead_code, unused_variables)]

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub const PORT_PATH: &str = "/dev/ttyUSB0";
pub const BAUD_RATE: u32  = 115_200;

/// Opens a serial port and echoes every received line back with "ECHO: " prefix.
/// Returns when the port is closed or an error occurs.
pub async fn run_echo_daemon(port_path: &str, baud_rate: u32) -> Result<(), Box<dyn std::error::Error>> {
    todo!(
        "1. Open port with tokio_serial::new(port_path, baud_rate).open_native_async()
         2. Wrap in BufReader
         3. Read lines in a loop with next_line()
         4. For each line: write 'ECHO: <line>\\n' back to the port
         5. Log each exchange with eprintln! or tracing"
    )
}

#[cfg(test)]
mod tests {
    // Note: testing serial I/O requires hardware or a virtual pair.
    // The acceptance test is: run the daemon on one pty, send a line
    // from the other pty, observe "ECHO: <line>" comes back.
    #[test]
    fn placeholder() {
        // Manual test procedure:
        // 1. socat -d -d pty,raw,echo=0 pty,raw,echo=0
        // 2. cargo run --example ex1_uart_echo (update PORT_PATH)
        // 3. In another terminal: echo "hello" > /dev/pts/N
        // 4. cat /dev/pts/N should show "ECHO: hello"
    }
}

#[tokio::main]
async fn main() {
    println!("Starting UART echo on {PORT_PATH} at {BAUD_RATE} baud");
    if let Err(e) = run_echo_daemon(PORT_PATH, BAUD_RATE).await {
        eprintln!("Error: {e}");
    }
}
