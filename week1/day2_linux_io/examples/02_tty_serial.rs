//! Ejemplo 02 — I/O UART/Serie Asíncrono con tokio-serial
//!
//! Ya conoces UART del lado del hardware (velocidad de baudios, paridad, bits de parada,
//! control de flujo). Este ejemplo muestra cómo configurar y usar un puerto serie
//! desde el espacio de usuario de Linux con async Rust.
//!
//! La clave es que tokio-serial envuelve el descriptor de archivo en la infraestructura
//! de I/O asíncrono de tokio (epoll en Linux), por lo que .read()/.write() ceden al runtime
//! en lugar de bloquear un hilo del SO.
//!
//! NOTA: Requiere un puerto serie real o virtual.
//! Crea un par virtual con: socat -d -d pty,raw,echo=0 pty,raw,echo=0
//! Luego usa una de las rutas /dev/pts/N reportadas como PORT_PATH.
//!
//! Ejecutar con:  cargo run --example 02_tty_serial

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits};

/// Cambia esto a tu puerto serie real
const PORT_PATH: &str = "/dev/ttyUSB0";
const BAUD_RATE: u32 = 115200;

#[tokio::main]
async fn main() {
    println!("=== I/O Serie Asíncrono ===\n");
    println!("Puerto:          {PORT_PATH}");
    println!("Velocidad baud:  {BAUD_RATE}");
    println!("Formato:         8N1 (8 bits de datos, sin paridad, 1 bit de parada)");
    println!();

    // Configurar el puerto serie.
    // Esto refleja lo que harías en HAL_UART_Init en un STM32 — pero desde
    // el lado Linux del cable.
    let port = tokio_serial::new(PORT_PATH, BAUD_RATE)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .open_native_async();

    let mut port = match port {
        Ok(p) => p,
        Err(e) => {
            println!("No se puede abrir {PORT_PATH}: {e}");
            println!();
            println!("Opciones de configuración del puerto serie:");
            println!("  Bits de datos:    tokio_serial::DataBits::{{Five,Six,Seven,Eight}}");
            println!("  Paridad:          tokio_serial::Parity::{{None,Odd,Even}}");
            println!("  Bits de parada:   tokio_serial::StopBits::{{One,Two}}");
            println!("  Control de flujo: tokio_serial::FlowControl::{{None,Software,Hardware}}");
            println!();
            println!("Para probar sin hardware, crea un par virtual:");
            println!("  socat -d -d pty,raw,echo=0 pty,raw,echo=0");
            println!("Luego actualiza PORT_PATH a una de las rutas /dev/pts/N reportadas.");
            return;
        }
    };

    // Enviar un saludo
    port.write_all(b"Hello from Rust daemon!\n").await.expect("escritura");
    println!("Enviado: 'Hello from Rust daemon!'");

    // Leer líneas de respuesta usando un BufReader para entramar línea a línea.
    // En la práctica usarías un codec personalizado (ver LengthDelimitedCodec en el día 4)
    // para protocolos binarios, pero las líneas funcionan bien para canales de depuración ASCII.
    let reader = BufReader::new(port);
    let mut lines = reader.lines();

    println!("Esperando respuestas (Ctrl+C para detener)...");
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => println!("Recibido: '{line}'"),
            Ok(None)       => { println!("Puerto cerrado."); break; }
            Err(e)         => { println!("Error de lectura: {e}"); break; }
        }
    }
}
