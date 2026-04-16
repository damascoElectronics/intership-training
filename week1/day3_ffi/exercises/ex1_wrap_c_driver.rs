//! Ejercicio 1 — Envolver un controlador de hardware C en una API segura de Rust
//!
//! Dado un controlador falso de ADC (Convertidor Analógico-Digital) en C, implementa un
//! envoltorio RAII seguro en Rust que gestione el ciclo de vida open/close automáticamente.
//!
//! Ejecutar pruebas:  cargo test --example ex1_wrap_c_driver

#![allow(dead_code, unused_variables)]

use std::ffi::CString;
use std::os::raw::{c_char, c_int};

// ── Declaraciones del controlador C falso ────────────────────────────────────────────────
// En un proyecto real estas vendrían de una biblioteca C compilada a través de build.rs.
// Aquí proporcionamos stubs de Rust que simulan la interfaz C.

/// Controlador ADC de C simulado (normalmente declarado en un archivo de cabecera C).
mod ffi {
    use std::os::raw::{c_char, c_int};
    use std::sync::atomic::{AtomicBool, Ordering};

    static OPEN: AtomicBool = AtomicBool::new(false);

    /// Abre el dispositivo ADC. Devuelve un handle (>0) o -1 en caso de error.
    pub unsafe extern "C" fn adc_open(device_path: *const c_char) -> c_int {
        if device_path.is_null() { return -1; }
        let path = unsafe { std::ffi::CStr::from_ptr(device_path) }.to_str().unwrap_or("");
        if path == "/dev/adc0" {
            OPEN.store(true, Ordering::SeqCst);
            42 // descriptor de archivo falso
        } else {
            -1
        }
    }

    /// Lee una muestra ADC. Devuelve 0 en éxito, rellena *value. Devuelve -1 en error.
    pub unsafe extern "C" fn adc_read(handle: c_int, value: *mut u16) -> c_int {
        if handle != 42 || value.is_null() { return -1; }
        unsafe { *value = 2048; } // valor de punto medio 12-bit
        0
    }

    /// Cierra el handle ADC.
    pub unsafe extern "C" fn adc_close(handle: c_int) {
        if handle == 42 {
            OPEN.store(false, Ordering::SeqCst);
        }
    }
}

// ── Tu implementación ───────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AdcError {
    #[error("falló al abrir el dispositivo ADC: {path}")]
    OpenFailed { path: String },
    #[error("lectura fallida")]
    ReadFailed,
    #[error("la ruta del dispositivo contiene un byte nulo inválido")]
    NullByte(#[from] std::ffi::NulError),
}

/// Un handle RAII seguro para un controlador ADC de C.
///
/// El dispositivo se cierra automáticamente cuando se descarta este struct.
pub struct AdcHandle {
    // TODO: agregar un campo para almacenar el descriptor de archivo C crudo (i32)
    // TODO: agregar un campo para almacenar la ruta del dispositivo (para mensajes de error)
    _private: (), // eliminar esto cuando agregues tus campos
}

impl AdcHandle {
    /// Abre el dispositivo ADC en `path`.
    ///
    /// # Errores
    /// Devuelve [`AdcError::OpenFailed`] si la llamada C `adc_open` devuelve -1.
    pub fn open(path: &str) -> Result<Self, AdcError> {
        todo!(
            "1. Convertir path a CString (devuelve Err en bytes nulos embebidos)
             2. Llamar ffi::adc_open(cstr.as_ptr()) en un bloque unsafe
             3. Si result < 0: devolver Err(AdcError::OpenFailed)
             4. Devolver Ok(Self {{ handle: result, path: path.to_owned() }})"
        )
    }

    /// Lee una muestra ADC (12-bit, 0–4095).
    pub fn read(&self) -> Result<u16, AdcError> {
        todo!(
            "1. Declarar: let mut value: u16 = 0;
             2. Llamar ffi::adc_read(self.handle, &mut value as *mut u16) en unsafe
             3. Si result != 0: devolver Err(AdcError::ReadFailed)
             4. Devolver Ok(value)"
        )
    }
}

impl Drop for AdcHandle {
    fn drop(&mut self) {
        // TODO: llamar ffi::adc_close(self.handle) en unsafe
        todo!("cerrar el handle")
    }
}

// ── Pruebas ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_valid_device() {
        let handle = AdcHandle::open("/dev/adc0").expect("debería abrirse");
        let value = handle.read().expect("debería leer");
        assert_eq!(value, 2048); // nuestro ADC falso siempre devuelve el punto medio
    }

    #[test]
    fn open_invalid_device_returns_err() {
        let result = AdcHandle::open("/dev/nonexistent");
        assert!(result.is_err());
        assert!(matches!(result, Err(AdcError::OpenFailed { .. })));
    }

    #[test]
    fn raii_drop_closes_handle() {
        // Después del drop, abrir el mismo dispositivo debería tener éxito de nuevo
        // (nuestro controlador falso rastrea el estado abierto)
        {
            let _handle = AdcHandle::open("/dev/adc0").unwrap();
        } // descartado aquí
        // Debería poder abrirse de nuevo
        let _handle2 = AdcHandle::open("/dev/adc0").expect("debería reabrirse después del drop");
    }
}

fn main() {
    println!("Ejecutar pruebas con: cargo test --example ex1_wrap_c_driver");
}
