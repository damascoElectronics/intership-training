//! Ejercicio 1 — Solución

#![allow(dead_code)]

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_serial::SerialPortBuilderExt;

pub const PORT_PATH: &str = "/dev/ttyUSB0";
pub const BAUD_RATE: u32  = 115_200;

pub async fn run_echo_daemon(port_path: &str, baud_rate: u32) -> Result<(), Box<dyn std::error::Error>> {
    let port = tokio_serial::new(port_path, baud_rate).open_native_async()?;
    let mut writer = port.try_clone()?;
    let reader = BufReader::new(port);
    let mut lines = reader.lines();

    eprintln!("Daemon de eco UART ejecutándose en {port_path}");

    while let Some(line) = lines.next_line().await? {
        eprintln!("  RX: '{line}'");
        let response = format!("ECHO: {line}\n");
        writer.write_all(response.as_bytes()).await?;
        eprintln!("  TX: '{}'", response.trim());
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run_echo_daemon(PORT_PATH, BAUD_RATE).await {
        eprintln!("Error: {e}");
    }
}
