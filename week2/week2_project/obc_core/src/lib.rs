//! # obc-core — Shared types for the OBC software stack
//!
//! This crate contains types shared across all OBC daemons:
//! [`tc_receiver`], [`obc_router`], [`hk_service`], [`sensor_daemon`], and [`ground_sim`].
//!
//! ## Module overview
//!
//! | Module    | Purpose                                      |
//! |-----------|----------------------------------------------|
//! | [`packet`]  | CCSDS-inspired `SpacePacket` and PUS service IDs |
//! | [`health`]  | Component health state and system aggregation |
//! | [`error`]   | Unified `OBCError` type                      |
//! | [`ipc`]     | Message types for Unix-socket IPC            |

pub mod error;
pub mod health;
pub mod ipc;
pub mod packet;

pub use error::OBCError;
pub use health::{ComponentId, HealthState, SystemHealth};
pub use ipc::{IpcMessage, IpcRequest, IpcResponse};
pub use packet::{PusService, SpacePacket};
