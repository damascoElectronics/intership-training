//! Enrutador de paquetes basado en APID.
//!
//! Enruta bytes de paquetes en bruto al canal manejador apropiado basándose en el
//! APID de 11 bits de la cabecera primaria CCSDS. Los paquetes no enrutables van a
//! un sumidero predeterminado opcional, o se devuelven como [`PacketError::NoRoute`].

use std::collections::HashMap;
use crate::error::PacketError;
use crate::primary_header::CcsdsPrimaryHeader;

/// Enruta Paquetes Espaciales a canales manejadores por APID.
pub struct ApidRouter<T = Vec<u8>> {
    routes: HashMap<u16, std::sync::mpsc::SyncSender<T>>,
}

impl ApidRouter {
    pub fn new() -> Self {
        Self { routes: HashMap::new() }
    }

    /// Registra un emisor de canal para el APID dado.
    ///
    /// Si ya existe una ruta para `apid`, se reemplaza.
    pub fn register(&mut self, apid: u16, tx: std::sync::mpsc::SyncSender<Vec<u8>>) {
        self.routes.insert(apid, tx);
    }

    /// Enruta `packet_bytes` al canal registrado para su APID.
    ///
    /// Parsea únicamente la cabecera primaria de 6 bytes para extraer el APID;
    /// la porción completa de bytes se reenvía sin modificaciones.
    ///
    /// # Errores
    /// Devuelve [`PacketError::BufferTooShort`] si hay menos de 6 bytes.
    /// Devuelve [`PacketError::NoRoute`] si no hay ruta registrada para el APID.
    pub fn route(&self, packet_bytes: Vec<u8>) -> Result<(), PacketError> {
        if packet_bytes.len() < 6 {
            return Err(PacketError::BufferTooShort { need: 6, got: packet_bytes.len() });
        }
        let raw: [u8; 6] = packet_bytes[..6].try_into().unwrap();
        let hdr = CcsdsPrimaryHeader::from_bytes(raw)?;
        let apid = hdr.apid();
        match self.routes.get(&apid) {
            Some(tx) => {
                // SyncSender::try_send no bloquea; si el canal está lleno, el paquete se descarta.
                let _ = tx.try_send(packet_bytes);
                Ok(())
            }
            None => Err(PacketError::NoRoute { apid }),
        }
    }
}

impl Default for ApidRouter {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pus_tc::PusTelecommand;

    #[test]
    fn routes_by_apid() {
        let mut router = ApidRouter::new();
        let (tx100, rx100) = std::sync::mpsc::sync_channel(8);
        let (tx200, rx200) = std::sync::mpsc::sync_channel(8);
        router.register(0x100, tx100);
        router.register(0x200, tx200);

        let pkt1 = PusTelecommand::new(0x100, 0, 17, 1, 0, vec![]).unwrap().to_bytes();
        let pkt2 = PusTelecommand::new(0x200, 0, 3, 129, 0, vec![]).unwrap().to_bytes();

        router.route(pkt1).unwrap();
        router.route(pkt2).unwrap();

        assert!(rx100.try_recv().is_ok());
        assert!(rx200.try_recv().is_ok());
    }

    #[test]
    fn no_route_returns_error() {
        let router = ApidRouter::new();
        let pkt = PusTelecommand::new(0x001, 0, 17, 1, 0, vec![]).unwrap().to_bytes();
        assert!(matches!(router.route(pkt), Err(PacketError::NoRoute { .. })));
    }
}
