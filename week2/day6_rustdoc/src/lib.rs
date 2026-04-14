//! # day6-rustdoc — CCSDS Frame Codec
//!
//! A demonstration crate for the **Week 2, Day 6** rustdoc training module.
//!
//! This crate implements a minimal CCSDS Space Packet primary header codec —
//! enough real spacecraft domain code to make the documentation examples
//! meaningful, while remaining small enough to read in one sitting.
//!
//! ## Purpose
//!
//! This crate exists to show:
//! - How to write doc comments that meet aerospace documentation standards
//! - How doc tests work and why they matter
//! - How `#![deny(missing_docs)]` enforces documentation coverage
//!
//! ## Modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`frame`] | [`frame::CcsdsPrimaryHeader`] type and field accessors |
//! | [`error`] | [`error::CcsdsError`] error variants |
//! | [`codec`] | [`codec::encode`] / [`codec::decode`] byte serialization |
//!
//! ## Quick Start
//!
//! ```rust
//! use day6_rustdoc::frame::CcsdsPrimaryHeader;
//! use day6_rustdoc::codec::{encode, decode};
//!
//! // Build a TC header for APID 0x100, sequence count 0, data length 4
//! let hdr = CcsdsPrimaryHeader::new_tc(0x100, 0, 4)?;
//! assert!(hdr.is_tc());
//! assert_eq!(hdr.apid(), 0x100);
//!
//! // Round-trip: encode to bytes and decode back
//! let bytes = encode(&hdr);
//! let decoded = decode(&bytes)?;
//! assert_eq!(decoded, hdr);
//! # Ok::<(), day6_rustdoc::error::CcsdsError>(())
//! ```
//!
//! ## References
//!
//! - CCSDS 133.0-B-2: *Space Packet Protocol*, Blue Book
//! - ECSS-E-ST-70-41C: *Packet Utilisation Standard (PUS-C)*

// Deny missing docs on all public items.
//
// WHY: In a library crate used by other teams (ground software, OBSW, test harness),
// every undocumented public item is a gap in the interface contract. Making this a
// compile error — not just a lint warning — ensures documentation coverage can never
// silently regress.
#![deny(missing_docs)]

pub mod codec;
pub mod error;
pub mod frame;
