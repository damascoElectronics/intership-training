//! Daemon TC Receiver — verifica la autenticación HMAC y la protección contra repetición
//! antes de reenviar telecomandos al router OBC.
//!
//! Frontera de seguridad: este proceso acepta bytes de una fuente no confiable
//! (enlace RF simulado) y solo reenvía paquetes que superan la autenticación.
//! NO tiene conocimiento de actuadores ni subsistemas — solo puede reenviar al
//! socket del router.
//!
//! # Arquitectura
//! ```text
//! [Ground Sim] ──(UnixSocket)──► [tc_receiver] ──(UnixSocket)──► [obc_router]
//!   bytes TC con HMAC             verificar HMAC                  analizar y enrutar
//!                                 comprobar ventana de repetición
//!                                 reenviar solo TCs válidos
//! ```

use hmac::{Hmac, Mac};
use obc_core::{IpcMessage, SpacePacket};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info, warn};

const TC_UPLINK_SOCKET: &str = "/tmp/obc_tc_uplink.sock";
const ROUTER_SOCKET: &str = "/tmp/obc_router.sock";

/// Clave HMAC-SHA256 compartida entre la estación terrestre y el OBC.
/// En un sistema real, esto se cargaría desde almacenamiento seguro, no se codificaría directamente.
const HMAC_KEY: &[u8] = b"spacecraft-tc-key-change-in-prod";

type HmacSha256 = Hmac<Sha256>;

/// Protección contra repetición con ventana deslizante (ventana de 64 paquetes).
struct ReplayWindow {
    last_seq: u16,
    /// Máscara de bits: el bit N activado significa que seq (last_seq − N) ya fue visto.
    window: u64,
    initialized: bool,
}

impl ReplayWindow {
    fn new() -> Self {
        Self { last_seq: 0, window: 0, initialized: false }
    }

    fn check_and_advance(&mut self, seq: u16) -> Result<(), &'static str> {
        if !self.initialized {
            self.last_seq = seq;
            self.window = 1;
            self.initialized = true;
            return Ok(());
        }

        // Calcular distancia desde last_seq (aritmética de 14 bits)
        let diff = (seq as i32 - self.last_seq as i32).rem_euclid(0x4000);

        if diff == 0 {
            return Err("repetición exacta");
        } else if diff < 64 {
            // Nuevo paquete en la ventana por delante — avanzar
            self.window = (self.window << diff) | 1;
            self.last_seq = seq;
        } else if diff > 0x3FC0 {
            // Paquete antiguo (detrás en la ventana)
            let back = (0x4000 - diff) as usize;
            if back >= 64 {
                return Err("demasiado antiguo");
            }
            if self.window & (1u64 << back) != 0 {
                return Err("repetición dentro de la ventana");
            }
            self.window |= 1u64 << back;
        } else {
            // Muy por delante — hueco en la secuencia (aceptable, avanzar ventana)
            self.window = 1;
            self.last_seq = seq;
        }
        Ok(())
    }
}

fn verify_hmac(packet_bytes: &[u8]) -> Result<&[u8], &'static str> {
    if packet_bytes.len() < 32 {
        return Err("paquete demasiado corto para HMAC");
    }
    let (payload, claimed_mac) = packet_bytes.split_at(packet_bytes.len() - 32);
    let mut mac = HmacSha256::new_from_slice(HMAC_KEY).expect("HMAC acepta claves de cualquier tamaño");
    mac.update(payload);
    let computed: [u8; 32] = mac.finalize().into_bytes().into();
    if computed.ct_eq(claimed_mac).into() {
        Ok(payload)
    } else {
        Err("HMAC no coincide")
    }
}

async fn forward_to_router(payload: &[u8]) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(ROUTER_SOCKET).await?;
    let len = payload.len() as u32;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(payload).await?;
    Ok(())
}

async fn handle_connection(
    mut stream: UnixStream,
    replay: &mut ReplayWindow,
    accepted: &mut u64,
    rejected: &mut u64,
) {
    loop {
        // Leer mensaje con prefijo de longitud
        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).await.is_err() {
            break; // conexión cerrada
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 65536 {
            warn!("paquete demasiado grande ({len} bytes), cerrando conexión");
            break;
        }

        let mut buf = vec![0u8; len];
        if stream.read_exact(&mut buf).await.is_err() {
            break;
        }

        // 1. Verificar HMAC
        let payload = match verify_hmac(&buf) {
            Ok(p) => p,
            Err(reason) => {
                warn!("rechazado: {reason}");
                *rejected += 1;
                continue;
            }
        };

        // 2. Deserializar para obtener el contador de secuencia para la verificación de repetición
        let pkt: SpacePacket = match bincode::serde::decode_from_slice(payload, bincode::config::standard()) {
            Ok((p, _)) => p,
            Err(e) => {
                warn!("rechazado: falló la deserialización: {e}");
                *rejected += 1;
                continue;
            }
        };

        // 3. Protección contra repetición
        if let Err(reason) = replay.check_and_advance(pkt.seq_count) {
            warn!("rechazado: {reason} (seq={})", pkt.seq_count);
            *rejected += 1;
            continue;
        }

        // 4. Reenviar payload verificado al router
        match forward_to_router(payload).await {
            Ok(()) => {
                info!("TC reenviado apid=0x{:03X} seq={} svc={}/{}",
                      pkt.apid, pkt.seq_count, pkt.service, pkt.subservice);
                *accepted += 1;
            }
            Err(e) => {
                error!("fallo al reenviar al router: {e}");
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter("info")
        .init();

    // Eliminar archivo de socket obsoleto
    let _ = std::fs::remove_file(TC_UPLINK_SOCKET);
    let listener = UnixListener::bind(TC_UPLINK_SOCKET)?;
    info!("Receptor TC escuchando en {TC_UPLINK_SOCKET}");
    info!("Reenviando TCs verificados a {ROUTER_SOCKET}");

    let mut replay = ReplayWindow::new();
    let mut accepted = 0u64;
    let mut rejected = 0u64;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                info!("nueva conexión desde la estación terrestre");
                handle_connection(stream, &mut replay, &mut accepted, &mut rejected).await;
                info!("conexión cerrada: aceptados={accepted} rechazados={rejected}");
            }
            Err(e) => error!("error de aceptación: {e}"),
        }
    }
}
