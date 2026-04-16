//! Tipos de mensajes IPC para la comunicación entre procesos mediante sockets de dominio Unix.
//!
//! Todos los mensajes se serializan con `bincode` (con prefijo de longitud) para que
//! el enmarcado se maneje de forma uniforme en todos los daemons.

use crate::health::SystemHealth;
use crate::packet::SpacePacket;
use serde::{Deserialize, Serialize};

/// Un mensaje enviado entre daemons OBC mediante sockets de dominio Unix.
///
/// Todos los daemons usan este formato de sobre. El payload interno transporta
/// un paquete espacial, una consulta/informe de salud o una señal de apagado.
#[derive(Debug, Serialize, Deserialize)]
pub enum IpcMessage {
    /// Un paquete espacial (TC o TM) siendo reenviado entre daemons.
    Packet(SpacePacket),
    /// Solicitar un informe de salud al daemon receptor.
    HealthQuery {
        /// Nombre del componente solicitante.
        requester: String,
    },
    /// Respuesta a un [`IpcMessage::HealthQuery`].
    HealthReport(SystemHealth),
    /// Indica al receptor que se apague de forma ordenada.
    Shutdown,
}

/// Una solicitud en un intercambio IPC de solicitud-respuesta.
///
/// El campo `id` se usa para correlacionar solicitudes con respuestas cuando
/// es posible que haya múltiples solicitudes en vuelo.
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcRequest {
    /// Identificador de solicitud monótonamente creciente (por remitente).
    pub id: u32,
    /// El payload real de la solicitud.
    pub payload: IpcMessage,
}

/// Una respuesta a un [`IpcRequest`].
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    /// Repite el [`IpcRequest::id`] para que el llamante pueda correlacionar la respuesta.
    pub request_id: u32,
    /// El payload de la respuesta.
    pub payload: IpcMessage,
}
