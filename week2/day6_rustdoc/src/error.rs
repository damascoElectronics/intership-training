//! Error types for the CCSDS frame codec.
//!
//! All fallible operations in this crate return [`CcsdsError`].
//! Using a single error enum (rather than ad-hoc strings) allows callers
//! to pattern-match on specific failure modes — important for spacecraft
//! telemetry monitoring where different errors require different responses.

use thiserror::Error;

/// Errors that can occur when working with CCSDS frames.
///
/// # Design Notes
///
/// Each variant carries the invalid value that caused the error so that
/// diagnostic telemetry (TM 1,8 "TC Acceptance Failure") can include
/// the offending value in its parameter data. This is a PUS-C requirement:
/// error reports must be traceable to a specific cause.
///
/// # Examples
///
/// ```rust
/// use day6_rustdoc::frame::CcsdsPrimaryHeader;
/// use day6_rustdoc::error::CcsdsError;
///
/// let result = CcsdsPrimaryHeader::new_tc(0xFFFF, 0, 4);
/// match result {
///     Err(CcsdsError::InvalidApid { value }) => {
///         assert_eq!(value, 0xFFFF);
///     }
///     _ => panic!("expected InvalidApid error"),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Error)]
pub enum CcsdsError {
    /// The APID value exceeds the 11-bit maximum (`0x7FF`).
    ///
    /// CCSDS 133.0-B-2, Section 4.1.2.3.1 limits APIDs to 11 bits (2048 values).
    /// The value `0x7FF` (2047) is the *idle packet APID*, reserved for fill packets
    /// transmitted when there is no real data to send. APIDs `0x000`–`0x7FE` are
    /// allocatable to application processes.
    ///
    /// If this error is returned, the header was not created and no memory was written.
    #[error("invalid APID {value:#05X}: must be in range 0x000..=0x7FE")]
    InvalidApid {
        /// The invalid APID value that was rejected.
        value: u16,
    },

    /// The sequence count exceeds the 14-bit maximum (`0x3FFF`).
    ///
    /// CCSDS 133.0-B-2, Section 4.1.2.5 allocates 14 bits to the packet
    /// sequence count. The maximum value is 16383 (`0x3FFF`). The counter
    /// wraps around to 0 after reaching the maximum.
    ///
    /// In practice this error should not occur if callers use
    /// [`crate::frame::CcsdsPrimaryHeader::next_seq_count`] to advance the counter,
    /// because that function applies the 14-bit mask automatically.
    #[error("invalid sequence count {value:#06X}: must be in range 0..=0x3FFF")]
    InvalidSeqCount {
        /// The invalid sequence count value that was rejected.
        value: u16,
    },

    /// A byte buffer presented for decoding is too short.
    ///
    /// A CCSDS primary header is exactly 6 octets. If [`crate::codec::decode`]
    /// is given a slice shorter than 6 bytes, this error is returned.
    ///
    /// The `expected` field will always be `6` for a primary header decode;
    /// it is included for forward-compatibility if larger structures are added.
    #[error("buffer too short: got {got} bytes, need at least {expected}")]
    BufferTooShort {
        /// Number of bytes actually available.
        got: usize,
        /// Minimum number of bytes required.
        expected: usize,
    },

    /// The version field in a decoded header is not zero.
    ///
    /// CCSDS 133.0-B-2, Section 4.1.2.1 defines the Packet Version Number as
    /// always `0b000` for the current standard. A non-zero value indicates
    /// either a corrupt packet or a future standard that this codec does not
    /// support.
    #[error("unsupported CCSDS version {version}: only version 0 is supported")]
    UnsupportedVersion {
        /// The non-zero version number found in the header.
        version: u8,
    },
}
