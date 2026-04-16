//! Ejercicio 1 — Solución: API de sensor de temperatura completamente documentada

#![deny(missing_docs)]

//! Este ejemplo demuestra rustdoc completo para un driver de sensor de hardware.

/// Un driver de sensor de temperatura sysfs de Linux.
///
/// Lee de un archivo sysfs que devuelve la temperatura en milígrados Celsius,
/// aplica un desplazamiento de calibración y valida el resultado.
///
/// # Ejemplos
/// ```no_run
/// use std::io::Write;
/// // Supongamos que tenemos un archivo sysfs real en /sys/class/thermal/thermal_zone0/temp
/// let sensor = TempSensor::new("/sys/class/thermal/thermal_zone0/temp");
/// // En pruebas, usar un archivo temporal:
/// ```
pub struct TempSensor {
    /// Ruta del sistema de archivos al atributo de temperatura sysfs (ej. `/sys/class/thermal/thermal_zone0/temp`).
    pub device_path: String,
    /// Desplazamiento de calibración en milígrados Celsius, añadido a cada lectura.
    /// Usar un valor negativo para compensar el auto-calentamiento del sensor.
    pub calibration_offset_mc: i32,
}

/// Errores que pueden ocurrir al leer de un [`TempSensor`].
#[derive(Debug)]
pub enum TempError {
    /// El archivo de dispositivo sysfs no existe o no se puede abrir.
    DeviceNotFound,
    /// El contenido del archivo no pudo analizarse como un entero.
    ReadError(String),
    /// La lectura calibrada está fuera del rango físicamente plausible
    /// (−273 °C a 200 °C).
    OutOfRange {
        /// El valor fuera de rango en milígrados Celsius.
        value: i32,
    },
}

impl TempSensor {
    /// Crea un nuevo `TempSensor` para la ruta sysfs dada sin desplazamiento de calibración.
    ///
    /// # Ejemplos
    /// ```
    /// let s = TempSensor::new("/tmp/fake_sensor");
    /// assert_eq!(s.calibration_offset_mc, 0);
    /// ```
    pub fn new(device_path: &str) -> Self {
        Self { device_path: device_path.to_owned(), calibration_offset_mc: 0 }
    }

    /// Establece el desplazamiento de calibración y devuelve `self` (patrón constructor).
    ///
    /// Un desplazamiento positivo eleva las lecturas; un desplazamiento negativo las reduce.
    ///
    /// # Ejemplos
    /// ```
    /// let s = TempSensor::new("/tmp/x").with_calibration(-500);
    /// assert_eq!(s.calibration_offset_mc, -500);
    /// ```
    pub fn with_calibration(mut self, offset_mc: i32) -> Self {
        self.calibration_offset_mc = offset_mc;
        self
    }

    /// Lee la temperatura en milígrados Celsius con calibración aplicada.
    ///
    /// # Errores
    /// - [`TempError::DeviceNotFound`] si el archivo sysfs no puede leerse.
    /// - [`TempError::ReadError`] si el contenido del archivo no es un entero válido.
    /// - [`TempError::OutOfRange`] si el valor calibrado está fuera de
    ///   −273 000 mc (cero absoluto) a 200 000 mc (más allá de cualquier rango normal de sensor).
    ///
    /// # Ejemplos
    /// ```
    /// use std::io::Write;
    /// let path = "/tmp/test_sensor_mc";
    /// std::fs::write(path, "25000\n").unwrap();
    /// let sensor = TempSensor::new(path).with_calibration(500);
    /// assert_eq!(sensor.read_mc().unwrap(), 25500);
    /// ```
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

    /// Lee la temperatura en grados Celsius (punto flotante).
    ///
    /// Este es un envoltorio de conveniencia alrededor de [`Self::read_mc`].
    ///
    /// # Errores
    /// Los mismos que [`Self::read_mc`].
    ///
    /// # Ejemplos
    /// ```
    /// let path = "/tmp/test_sensor_c";
    /// std::fs::write(path, "23500\n").unwrap();
    /// let sensor = TempSensor::new(path);
    /// let celsius = sensor.read_celsius().unwrap();
    /// assert!((celsius - 23.5).abs() < 0.001);
    /// ```
    pub fn read_celsius(&self) -> Result<f64, TempError> {
        Ok(self.read_mc()? as f64 / 1000.0)
    }
}

impl std::fmt::Display for TempError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceNotFound => write!(f, "dispositivo no encontrado"),
            Self::ReadError(e) => write!(f, "error de lectura: {e}"),
            Self::OutOfRange { value } => write!(f, "valor {value} mc fuera del rango válido"),
        }
    }
}

fn main() {
    println!("Compilar docs: cargo doc --example ex1_document_api_sol --open");
}
