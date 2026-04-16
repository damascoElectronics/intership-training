//! Ejemplo 01 — GPIO a través de sysfs
//!
//! Linux expone los pines GPIO a través del sistema de archivos virtual sysfs en
//! /sys/class/gpio/.  Esta es la interfaz "legada" (aún ampliamente utilizada
//! en placas Linux embebidas como Raspberry Pi, Zynq, etc.).
//!
//! La interfaz más nueva es el dispositivo de carácter GPIO (/dev/gpiochipN) y
//! la biblioteca libgpiod, pero sysfs es más simple de entender primero.
//!
//! NOTA: Este ejemplo NO requiere hardware real — muestra el
//! patrón y envuelve las operaciones en RAII para que sean seguras. En una placa
//! real tendrías un pin físico conectado.
//!
//! Ejecutar con:  cargo run --example 01_sysfs_gpio
//!                (fallará de forma controlada sin root / sin hardware GPIO)

use std::io;
use std::path::PathBuf;

/// Envoltorio RAII para un pin GPIO en sysfs.
///
/// En `Drop`, el pin es desexportado (liberado) para que otros procesos puedan usarlo.
pub struct GpioPin {
    number: u32,
    base: PathBuf,
}

impl GpioPin {
    /// Exporta (reclama) un pin GPIO escribiendo su número en /sys/class/gpio/export.
    ///
    /// Devuelve `Err` si el pin ya está exportado o si no tienes permisos.
    /// Es posible que necesites ejecutarlo como root: `sudo ./target/debug/...`
    pub fn export(number: u32) -> io::Result<Self> {
        let export_path = PathBuf::from("/sys/class/gpio/export");
        std::fs::write(&export_path, number.to_string())?;

        // Pequeña demora — las entradas de sysfs tardan un momento en aparecer tras el export
        std::thread::sleep(std::time::Duration::from_millis(50));

        Ok(Self {
            number,
            base: PathBuf::from(format!("/sys/class/gpio/gpio{number}")),
        })
    }

    /// Establece la dirección del pin: "in" o "out".
    pub fn set_direction(&self, direction: &str) -> io::Result<()> {
        std::fs::write(self.base.join("direction"), direction)
    }

    /// Establece el valor de salida: `true` = alto (1), `false` = bajo (0).
    pub fn set_value(&self, high: bool) -> io::Result<()> {
        std::fs::write(self.base.join("value"), if high { "1" } else { "0" })
    }

    /// Lee el valor actual del pin. Devuelve `true` si está en alto.
    pub fn get_value(&self) -> io::Result<bool> {
        let contents = std::fs::read_to_string(self.base.join("value"))?;
        Ok(contents.trim() == "1")
    }

    /// Consulta el archivo de valor repetidamente hasta que cambie o se agote el tiempo.
    ///
    /// Este es un enfoque de polling simple. Para eventos disparados por flanco,
    /// epoll/inotify sobre el archivo "value" de sysfs es más eficiente.
    pub fn wait_for_change(&self, current: bool, timeout_ms: u64) -> io::Result<bool> {
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_millis(timeout_ms);
        loop {
            let now = self.get_value()?;
            if now != current { return Ok(now); }
            if std::time::Instant::now() >= deadline {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "tiempo de espera agotado esperando cambio en GPIO"));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

impl Drop for GpioPin {
    fn drop(&mut self) {
        // Desexporta el pin para que otros procesos puedan usarlo.
        // Ignorar errores: si ya estamos muertos, esto es un mejor esfuerzo.
        let _ = std::fs::write("/sys/class/gpio/unexport", self.number.to_string());
    }
}

fn main() {
    println!("=== GPIO a través de sysfs ===\n");

    // Intentar exportar GPIO 17 (un pin de prueba común en Raspberry Pi).
    // En sistemas sin hardware GPIO o sin root, esto fallará
    // con un error descriptivo.
    match GpioPin::export(17) {
        Ok(pin) => {
            println!("GPIO 17 exportado");
            pin.set_direction("out").expect("establecer dirección");
            println!("Dirección establecida a 'out'");

            pin.set_value(true).expect("establecer valor");
            println!("Puesto en alto (1)");
            let v = pin.get_value().expect("leer valor");
            println!("Lectura de vuelta: {}", if v { "ALTO" } else { "BAJO" });

            pin.set_value(false).expect("establecer valor");
            println!("Puesto en bajo (0)");

            println!("\nEl pin será desexportado al descartarse (RAII).");
            // `pin` se descarta aquí → el desexport ocurre automáticamente
        }
        Err(e) => {
            println!("No se puede exportar GPIO 17: {e}");
            println!("(Esto es lo esperado sin root o sin hardware GPIO.)");
            println!();
            println!("El patrón RAII y la estructura de rutas sysfs son lo que importa:");
            println!("  /sys/class/gpio/export       ← escribir número de pin para reclamar");
            println!("  /sys/class/gpio/gpioN/direction ← escribir 'in' o 'out'");
            println!("  /sys/class/gpio/gpioN/value     ← leer/escribir '0' o '1'");
            println!("  /sys/class/gpio/unexport     ← escribir número de pin para liberar");
        }
    }
}
