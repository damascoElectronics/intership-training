//! Tipos de error para el codec de tramas CCSDS.
//!
//! Todas las operaciones falibles en este crate devuelven [`CcsdsError`].
//! Usar un único enum de error (en lugar de cadenas ad-hoc) permite a los llamadores
//! hacer coincidencia de patrones en modos de fallo específicos — importante para la
//! monitorización de telemetría de naves espaciales donde diferentes errores requieren respuestas distintas.

use thiserror::Error;

/// Errores que pueden ocurrir al trabajar con tramas CCSDS.
///
/// # Notas de Diseño
///
/// Cada variante lleva el valor inválido que causó el error para que
/// la telemetría de diagnóstico (TM 1,8 "TC Acceptance Failure") pueda incluir
/// el valor infractor en sus datos de parámetros. Este es un requisito de PUS-C:
/// los informes de error deben ser trazables a una causa específica.
///
/// # Ejemplos
///
/// ```rust
/// use day6_rustdoc::frame::CcsdsPrimaryHeader;
/// use day6_rustdoc::error::CcsdsError;
///
/// let result = CcsdsPrimaryHeader::new_tc(0xFFFF, 0, 4);
/// match result {
///     Err(CcsdsError::InvalidApid { value }) => {
///         assert_eq!(value, 0xFFFF);
///     }
///     _ => panic!("se esperaba error InvalidApid"),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Error)]
pub enum CcsdsError {
    /// El valor APID supera el máximo de 11 bits (`0x7FF`).
    ///
    /// CCSDS 133.0-B-2, Sección 4.1.2.3.1 limita los APIDs a 11 bits (2048 valores).
    /// El valor `0x7FF` (2047) es el *APID de paquete inactivo*, reservado para paquetes de relleno
    /// transmitidos cuando no hay datos reales que enviar. Los APIDs `0x000`–`0x7FE` son
    /// asignables a procesos de aplicación.
    ///
    /// Si se devuelve este error, la cabecera no fue creada y no se escribió ninguna memoria.
    #[error("APID inválido {value:#05X}: debe estar en el rango 0x000..=0x7FE")]
    InvalidApid {
        /// El valor APID inválido que fue rechazado.
        value: u16,
    },

    /// El conteo de secuencia supera el máximo de 14 bits (`0x3FFF`).
    ///
    /// CCSDS 133.0-B-2, Sección 4.1.2.5 asigna 14 bits al conteo de secuencia del paquete.
    /// El valor máximo es 16383 (`0x3FFF`). El contador se desborda a 0 después de alcanzar el máximo.
    ///
    /// En la práctica este error no debería ocurrir si los llamadores usan
    /// [`crate::frame::CcsdsPrimaryHeader::next_seq_count`] para avanzar el contador,
    /// porque esa función aplica la máscara de 14 bits automáticamente.
    #[error("conteo de secuencia inválido {value:#06X}: debe estar en el rango 0..=0x3FFF")]
    InvalidSeqCount {
        /// El valor de conteo de secuencia inválido que fue rechazado.
        value: u16,
    },

    /// Un buffer de bytes presentado para decodificación es demasiado corto.
    ///
    /// Una cabecera primaria CCSDS tiene exactamente 6 octetos. Si [`crate::codec::decode`]
    /// recibe un slice más corto que 6 bytes, se devuelve este error.
    ///
    /// El campo `expected` siempre será `6` para una decodificación de cabecera primaria;
    /// se incluye para compatibilidad futura si se añaden estructuras más grandes.
    #[error("buffer demasiado corto: se obtuvieron {got} bytes, se necesitan al menos {expected}")]
    BufferTooShort {
        /// Número de bytes realmente disponibles.
        got: usize,
        /// Número mínimo de bytes requeridos.
        expected: usize,
    },

    /// El campo de versión en una cabecera decodificada no es cero.
    ///
    /// CCSDS 133.0-B-2, Sección 4.1.2.1 define el Número de Versión del Paquete como
    /// siempre `0b000` para el estándar actual. Un valor distinto de cero indica
    /// ya sea un paquete corrupto o un estándar futuro que este codec no soporta.
    #[error("versión CCSDS no soportada {version}: solo se soporta la versión 0")]
    UnsupportedVersion {
        /// El número de versión distinto de cero encontrado en la cabecera.
        version: u8,
    },
}
