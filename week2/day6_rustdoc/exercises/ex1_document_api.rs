//! Ejercicio 1 — Añadir rustdoc completo a una API sin documentar
//!
//! El módulo de abajo es un "driver de sensor de temperatura" falso sin documentación.
//! Tu tarea: añadir rustdoc a CADA elemento público, incluyendo doc tests.
//!
//! Requisitos:
//!   - Cada pub fn/struct/enum debe tener un comentario ///
//!   - Incluir sección # Ejemplos con un doc test ejecutable
//!   - Incluir # Errores para funciones que devuelven Result
//!   - Incluir # Panics donde corresponda
//!   - Añadir #![deny(missing_docs)] y corregir cualquier violación restante
//!
//! Compilar docs: cargo doc --example ex1_document_api --open
//! Ejecutar pruebas:  cargo test --example ex1_document_api

// TODO: añadir #![deny(missing_docs)] aquí una vez que todos los elementos estén documentados

pub struct TempSensor {
    pub device_path: String,
    pub calibration_offset_mc: i32,
}

#[derive(Debug)]
pub enum TempError {
    DeviceNotFound,
    ReadError(String),
    OutOfRange { value: i32 },
}

impl TempSensor {
    pub fn new(device_path: &str) -> Self {
        Self { device_path: device_path.to_owned(), calibration_offset_mc: 0 }
    }

    pub fn with_calibration(mut self, offset_mc: i32) -> Self {
        self.calibration_offset_mc = offset_mc;
        self
    }

    pub fn read_mc(&self) -> Result<i32, TempError> {
        match std::fs::read_to_string(&self.device_path) {
            Ok(s) => {
                let raw: i32 = s.trim().parse()
                    .map_err(|e: std::num::ParseIntError| TempError::ReadError(e.to_string()))?;
                let calibrated = raw + self.calibration_offset_mc;
                if calibrated < -273_000 || calibrated > 200_000 {
                    return Err(TempError::OutOfRange { value: calibrated });
                }
                Ok(calibrated)
            }
            Err(_) => Err(TempError::DeviceNotFound),
        }
    }

    pub fn read_celsius(&self) -> Result<f64, TempError> {
        Ok(self.read_mc()? as f64 / 1000.0)
    }
}

impl std::fmt::Display for TempError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceNotFound => write!(f, "dispositivo no encontrado"),
            Self::ReadError(e) => write!(f, "error de lectura: {e}"),
            Self::OutOfRange { value } => write!(f, "valor fuera de rango: {value} mc"),
        }
    }
}

fn main() {
    println!("Compilar docs: cargo doc --example ex1_document_api --open");
    println!("Ejecutar pruebas:  cargo test --example ex1_document_api");
}
