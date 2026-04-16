//! # obc-core — Tipos compartidos para la pila de software OBC
//!
//! Este crate contiene los tipos compartidos entre todos los daemons OBC:
//! [`tc_receiver`], [`obc_router`], [`hk_service`], [`sensor_daemon`] y [`ground_sim`].
//!
//! ## Resumen de módulos
//!
//! | Módulo    | Propósito                                            |
//! |-----------|------------------------------------------------------|
//! | [`packet`]  | `SpacePacket` inspirado en CCSDS e IDs de servicio PUS |
//! | [`health`]  | Estado de salud de componentes y agregación del sistema |
//! | [`error`]   | Tipo unificado `OBCError`                            |
//! | [`ipc`]     | Tipos de mensajes para IPC mediante sockets Unix     |

pub mod error;
pub mod health;
pub mod ipc;
pub mod packet;

pub use error::OBCError;
pub use health::{ComponentId, HealthState, SystemHealth};
pub use ipc::{IpcMessage, IpcRequest, IpcResponse};
pub use packet::{PusService, SpacePacket};
