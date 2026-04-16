//! Ejercicio 1 — Daemon de eco UART asíncrono
//!
//! Implementa un daemon asíncrono que repite cada línea recibida por serie
//! con el prefijo "ECHO: ".
//!
//! NOTA: Requiere un par de puertos serie virtuales:
//!   socat -d -d pty,raw,echo=0 pty,raw,echo=0
//! Usa las dos rutas /dev/pts/N como PORT y el otro extremo para pruebas.

#![allow(dead_code, unused_variables)]

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub const PORT_PATH: &str = "/dev/ttyUSB0";
pub const BAUD_RATE: u32  = 115_200;

/// Abre un puerto serie y repite cada línea recibida de vuelta con el prefijo "ECHO: ".
/// Retorna cuando el puerto se cierra o ocurre un error.
pub async fn run_echo_daemon(port_path: &str, baud_rate: u32) -> Result<(), Box<dyn std::error::Error>> {
    todo!(
        "1. Abrir puerto con tokio_serial::new(port_path, baud_rate).open_native_async()
         2. Envolver en BufReader
         3. Leer líneas en un bucle con next_line()
         4. Por cada línea: escribir 'ECHO: <línea>\\n' de vuelta al puerto
         5. Registrar cada intercambio con eprintln! o tracing"
    )
}

#[cfg(test)]
mod tests {
    // Nota: probar I/O serie requiere hardware o un par virtual.
    // La prueba de aceptación es: ejecutar el daemon en un pty, enviar una línea
    // desde el otro pty, observar que "ECHO: <línea>" regresa.
    #[test]
    fn placeholder() {
        // Procedimiento de prueba manual:
        // 1. socat -d -d pty,raw,echo=0 pty,raw,echo=0
        // 2. cargo run --example ex1_uart_echo (actualizar PORT_PATH)
        // 3. En otra terminal: echo "hello" > /dev/pts/N
        // 4. cat /dev/pts/N debe mostrar "ECHO: hello"
    }
}

#[tokio::main]
async fn main() {
    println!("Iniciando eco UART en {PORT_PATH} a {BAUD_RATE} baudios");
    if let Err(e) = run_echo_daemon(PORT_PATH, BAUD_RATE).await {
        eprintln!("Error: {e}");
    }
}
