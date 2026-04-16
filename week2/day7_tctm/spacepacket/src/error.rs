/// Errores que pueden producirse al parsear o construir paquetes CCSDS/PUS.
#[derive(Debug, thiserror::Error)]
pub enum PacketError {
    /// El APID debe estar en el rango 0x000–0x7FE.
    ///
    /// 0x7FF está reservado para el APID de paquete idle (relleno) (CCSDS 133.0-B-2 §4.1.2.3.2).
    #[error("APID {value:#05X} está fuera de rango; debe ser 0x000–0x7FE")]
    InvalidApid { value: u16 },

    /// El contador de secuencia supera el máximo de 14 bits (0x3FFF).
    #[error("el contador de secuencia {value} supera el máximo de 14 bits (0x3FFF)")]
    InvalidSeqCount { value: u16 },

    /// El búfer es demasiado corto para contener una cabecera primaria válida (necesita ≥ 6 bytes).
    #[error("búfer demasiado corto: se necesitan ≥ {need} bytes, se obtuvieron {got}")]
    BufferTooShort { need: usize, got: usize },

    /// Error de CRC: el paquete está corrupto o ha sido manipulado.
    #[error("error de CRC: calculado {computed:#06X}, recibido {received:#06X}")]
    CrcMismatch { computed: u16, received: u16 },

    /// El campo de versión CCSDS es distinto de cero (siempre debe ser 0b000).
    #[error("versión CCSDS desconocida {version}; se esperaba 0")]
    UnknownVersion { version: u8 },

    /// El enrutador no tiene ruta para el APID dado.
    #[error("no hay ruta para APID {apid:#05X}")]
    NoRoute { apid: u16 },
}
