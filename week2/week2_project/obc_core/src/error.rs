//! Unified error type for the OBC software stack.

/// All errors that can occur within the OBC software stack.
#[derive(Debug, thiserror::Error)]
pub enum OBCError {
    /// An IPC send operation failed (e.g., the peer closed its socket).
    #[error("IPC send failed: {0}")]
    IpcSend(String),

    /// HMAC verification failed for an incoming TC.
    #[error("authentication failed: {0}")]
    AuthFailed(String),

    /// The received TC sequence number falls within the replay window.
    #[error("replay detected: seq={seq}")]
    ReplayDetected { seq: u16 },

    /// Could not parse a packet from raw bytes.
    #[error("packet parse error: {0}")]
    ParseError(String),

    /// A required component reported a non-nominal health state.
    #[error("component {id} is not healthy: {state}")]
    ComponentUnhealthy { id: String, state: String },

    /// Wraps `std::io::Error` for transparent propagation.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
