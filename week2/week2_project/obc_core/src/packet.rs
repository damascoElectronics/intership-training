//! Tipos de paquetes espaciales (CCSDS 133.0-B-2 simplificado).

use serde::{Deserialize, Serialize};

/// Un Space Packet simplificado para uso en IPC.
///
/// En un sistema real, esto se construiría sobre el crate `spacepacket` completo
/// introducido en el Día 7. Aquí mantenemos una estructura plana y propia que serializa
/// limpiamente con `bincode`.
///
/// # Telecomando vs Telemetría
///
/// La convención usada en toda esta pila es:
/// * **TC (enlace ascendente)** – `apid` tiene el bit 12 activado (0x1xxx).
/// * **TM (enlace descendente)** – `apid` tiene el bit 12 desactivado (0x0xxx).
///
/// Esto refleja el bit de tipo de paquete CCSDS (bit 4 del primer octeto de la cabecera
/// primaria) pero codificado en el campo APID por simplicidad.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacePacket {
    /// Identificador de Proceso de Aplicación (campo CCSDS de 11 bits, almacenado como u16).
    pub apid: u16,
    /// Contador de secuencia de origen (se reinicia en 0x3FFF según CCSDS).
    pub seq_count: u16,
    /// Tipo de servicio PUS.
    pub service: u8,
    /// Subtipo de servicio PUS.
    pub subservice: u8,
    /// Identificador de origen (proceso/aplicación que creó el paquete).
    pub source_id: u16,
    /// Tiempo transcurrido de misión en milisegundos desde el epoch.
    pub timestamp_ms: u64,
    /// Datos de aplicación (payload).
    pub data: Vec<u8>,
    /// Etiqueta de autenticación HMAC-SHA256, presente solo en paquetes TC desde tierra.
    pub hmac: Option<[u8; 32]>,
}

impl SpacePacket {
    /// Crea un nuevo paquete de telecomando.
    ///
    /// El APID se almacena con el bit 12 activado para marcarlo como TC.
    pub fn new_tc(
        apid: u16,
        seq_count: u16,
        service: u8,
        subservice: u8,
        data: Vec<u8>,
    ) -> Self {
        Self {
            apid: apid | 0x1000, // activar bit marcador de TC
            seq_count,
            service,
            subservice,
            source_id: 0,
            timestamp_ms: timestamp_now_ms(),
            data,
            hmac: None,
        }
    }

    /// Crea un nuevo paquete de telemetría.
    ///
    /// El APID se almacena con el bit 12 desactivado para marcarlo como TM.
    pub fn new_tm(
        apid: u16,
        seq_count: u16,
        service: u8,
        subservice: u8,
        data: Vec<u8>,
    ) -> Self {
        Self {
            apid: apid & !0x1000, // desactivar bit marcador de TC
            seq_count,
            service,
            subservice,
            source_id: 1, // fuente OBC
            timestamp_ms: timestamp_now_ms(),
            data,
            hmac: None,
        }
    }

    /// Devuelve `true` si este paquete es un telecomando (enlace ascendente).
    #[inline]
    pub fn is_tc(&self) -> bool {
        self.apid & 0x1000 != 0
    }

    /// Devuelve el APID sin el bit marcador de TC.
    #[inline]
    pub fn bare_apid(&self) -> u16 {
        self.apid & 0x0FFF
    }
}

/// Devuelve la hora UNIX actual en milisegundos.
///
/// Regresa a 0 en plataformas donde `SystemTime` no está disponible.
fn timestamp_now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Identificadores de servicio PUS-C usados en esta pila.
///
/// Consultar `reference/pus_service_catalog.md` para la tabla completa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PusService {
    /// Servicio 1 – Verificación de TC
    TcVerification = 1,
    /// Servicio 3 – Housekeeping
    Housekeeping = 3,
    /// Servicio 5 – Reporte de Eventos
    Event = 5,
    /// Servicio 17 – Operaciones a Bordo (ping/pong)
    Test = 17,
}

impl PusService {
    /// Intenta convertir un número de servicio en bruto a un [`PusService`].
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::TcVerification),
            3 => Some(Self::Housekeeping),
            5 => Some(Self::Event),
            17 => Some(Self::Test),
            _ => None,
        }
    }
}
