//! Ejemplo 03 — Dispositivos de carácter e ioctl
//!
//! Más allá de sysfs, los drivers de hardware de Linux se exponen como dispositivos de carácter
//! (archivos en /dev/).  Interactúas con ellos mediante llamadas al sistema read()/write()/ioctl().
//! `ioctl` es la navaja suiza: es un comodín para configuración específica del dispositivo
//! que no encaja en la metáfora de archivo.
//!
//! Este ejemplo muestra el patrón usando el dispositivo RTC (reloj de tiempo real)
//! disponible en la mayoría de sistemas Linux en /dev/rtc0.
//!
//! Ejecutar con:  cargo run --example 03_chardev_ioctl

use nix::ioctl_read;
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

// ioctl del RTC: leer la hora actual del reloj de tiempo real del hardware.
// El número de ioctl se construye a partir del tipo (0x70='p') y nr (0x09).
// nix::ioctl_read! genera un envoltorio seguro.
//
// struct rtc_time coincide con el layout de struct rtc_time del kernel.
#[repr(C)]
#[derive(Default, Debug)]
struct RtcTime {
    tm_sec:   i32,
    tm_min:   i32,
    tm_hour:  i32,
    tm_mday:  i32,
    tm_mon:   i32,   // basado en 0 (Enero = 0)
    tm_year:  i32,   // años desde 1900
    tm_wday:  i32,
    tm_yday:  i32,
    tm_isdst: i32,
}

// Genera: unsafe fn rtc_rd_time(fd: RawFd, data: *mut RtcTime) -> Result<i32>
ioctl_read!(rtc_rd_time, 0x70, 0x09, RtcTime);

fn main() {
    println!("=== ioctl de dispositivo de carácter ===\n");

    // Abrir /dev/rtc0 en modo solo lectura, sin O_NONBLOCK (el bloqueo está bien aquí)
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_RDONLY)
        .open("/dev/rtc0");

    match file {
        Ok(f) => {
            let fd = f.as_raw_fd();
            let mut time = RtcTime::default();

            // Llamar al ioctl — esto es unsafe porque:
            // 1. Estamos llamando una función FFI (una interfaz del kernel)
            // 2. Estamos pasando un puntero crudo a `time`
            // El contrato de SAFETY: `time` es válido, alineado y tiene el layout
            // correcto esperado por el struct rtc_time del kernel.
            match unsafe { rtc_rd_time(fd, &mut time) } {
                Ok(_) => {
                    println!("Hora del RTC del hardware:");
                    println!("  {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                        time.tm_year + 1900,
                        time.tm_mon + 1,
                        time.tm_mday,
                        time.tm_hour,
                        time.tm_min,
                        time.tm_sec,
                    );
                }
                Err(e) => println!("ioctl falló: {e}"),
            }
        }
        Err(e) => {
            println!("No se puede abrir /dev/rtc0: {e}");
            println!("(Puede ser necesario: sudo chmod a+r /dev/rtc0)");
            println!();
            println!("El patrón ioctl de todas formas:");
            println!("  1. Definir layout: #[repr(C)] struct coincidiendo con el struct del kernel");
            println!("  2. Generar envoltorio: nix::ioctl_read! / ioctl_write! / ioctl_readwrite!");
            println!("  3. Abrir el archivo /dev");
            println!("  4. Llamar ioctl en un bloque unsafe con comentario SAFETY");
        }
    }

    println!();
    println!("Cuándo usar esto vs tokio-serial:");
    println!("  tokio-serial: puertos UART/serie estándar (basados en termios)");
    println!("  ioctl directamente: drivers del kernel personalizados que exponen /dev/mydevice");
    println!("    - Dispositivos SPI no manejados por spidev");
    println!("    - Acceso a registros FPGA personalizados vía driver UIO");
    println!("    - Hardware propietario con módulos del kernel a medida");
}

// Se necesita libc para la constante O_RDONLY
use std::os::raw::c_int;
mod libc {
    pub const O_RDONLY: super::c_int = 0;
}
