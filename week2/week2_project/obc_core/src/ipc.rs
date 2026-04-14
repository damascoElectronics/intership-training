//! IPC message types for inter-process communication over Unix domain sockets.
//!
//! All messages are serialised with `bincode` (length-prefixed) so that
//! framing is handled uniformly across all daemons.

use crate::health::SystemHealth;
use crate::packet::SpacePacket;
use serde::{Deserialize, Serialize};

/// A message sent between OBC daemons over Unix domain sockets.
///
/// All daemons speak this envelope format.  The inner payload carries
/// either a space packet, a health query/report, or a shutdown signal.
#[derive(Debug, Serialize, Deserialize)]
pub enum IpcMessage {
    /// A space packet (TC or TM) being forwarded between daemons.
    Packet(SpacePacket),
    /// Request a health report from the receiving daemon.
    HealthQuery {
        /// Name of the requesting component.
        requester: String,
    },
    /// Response to a [`IpcMessage::HealthQuery`].
    HealthReport(SystemHealth),
    /// Instructs the receiver to shut down gracefully.
    Shutdown,
}

/// A request in a request-response IPC exchange.
///
/// The `id` field is used to correlate requests with responses when
/// multiple in-flight requests are possible.
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcRequest {
    /// Monotonically increasing request identifier (per sender).
    pub id: u32,
    /// The actual request payload.
    pub payload: IpcMessage,
}

/// A response to an [`IpcRequest`].
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    /// Echoes the [`IpcRequest::id`] so the caller can match the response.
    pub request_id: u32,
    /// The response payload.
    pub payload: IpcMessage,
}
