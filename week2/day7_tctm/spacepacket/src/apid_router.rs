//! APID-based packet router.
//!
//! Routes raw packet bytes to the appropriate handler channel based on the
//! 11-bit APID in the CCSDS primary header.  Unroutable packets go to an
//! optional default sink, or are returned as [`PacketError::NoRoute`].

use std::collections::HashMap;
use crate::error::PacketError;
use crate::primary_header::CcsdsPrimaryHeader;

/// Routes Space Packets to handler channels by APID.
pub struct ApidRouter<T = Vec<u8>> {
    routes: HashMap<u16, std::sync::mpsc::SyncSender<T>>,
}

impl ApidRouter {
    pub fn new() -> Self {
        Self { routes: HashMap::new() }
    }

    /// Register a channel sender for the given APID.
    ///
    /// If a route already exists for `apid` it is replaced.
    pub fn register(&mut self, apid: u16, tx: std::sync::mpsc::SyncSender<Vec<u8>>) {
        self.routes.insert(apid, tx);
    }

    /// Route `packet_bytes` to the channel registered for its APID.
    ///
    /// Parses only the 6-byte primary header to extract the APID; the full
    /// byte slice is forwarded unchanged.
    ///
    /// # Errors
    /// Returns [`PacketError::BufferTooShort`] if fewer than 6 bytes.
    /// Returns [`PacketError::NoRoute`] if no route is registered for the APID.
    pub fn route(&self, packet_bytes: Vec<u8>) -> Result<(), PacketError> {
        if packet_bytes.len() < 6 {
            return Err(PacketError::BufferTooShort { need: 6, got: packet_bytes.len() });
        }
        let raw: [u8; 6] = packet_bytes[..6].try_into().unwrap();
        let hdr = CcsdsPrimaryHeader::from_bytes(raw)?;
        let apid = hdr.apid();
        match self.routes.get(&apid) {
            Some(tx) => {
                // SyncSender::try_send won't block; if channel is full the packet is dropped.
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
