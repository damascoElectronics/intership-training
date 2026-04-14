//! CCSDS Space Packet primary header type and field accessors.
//!
//! This module provides [`CcsdsPrimaryHeader`], a zero-copy representation
//! of the 6-octet CCSDS primary header defined in CCSDS 133.0-B-2.

use crate::error::CcsdsError;

/// A CCSDS Space Packet primary header.
///
/// The primary header is 6 octets (48 bits) with the following bit layout:
///
/// ```text
/// Word   Bits    Field
/// ────────────────────────────────────────────────────────────
/// 0-1    15-13   Packet Version Number (always 0b000)
/// 0-1    12      Packet Type          (0 = TM,  1 = TC)
/// 0-1    11      Secondary Header Flag (1 = present)
/// 0-1    10-0    Application Process Identifier (APID)
/// 2-3    15-14   Sequence Flags        (0b11 = standalone)
/// 2-3    13-0    Packet Sequence Count (14-bit, per-APID)
/// 4-5    15-0    Packet Data Length    (data_octets - 1)
/// ────────────────────────────────────────────────────────────
/// ```
///
/// The raw bytes are stored in big-endian order as mandated by CCSDS.
///
/// # Invariants
///
/// A value of this type always satisfies:
/// - `version == 0` (bits \[15:13\] of word 0)
/// - `apid <= 0x7FE` (bits \[10:0\] of word 0; 0x7FF is idle APID)
/// - `seq_count <= 0x3FFF` (bits \[13:0\] of word 1)
///
/// These invariants are checked in all constructors, so any `CcsdsPrimaryHeader`
/// in existence is guaranteed to be valid.
///
/// # References
/// - CCSDS 133.0-B-2, Section 4.1 — *Space Packet Primary Header*
#[derive(Debug, Clone, PartialEq)]
pub struct CcsdsPrimaryHeader {
    // Store the raw 6-byte representation.
    //
    // WHY raw bytes instead of individual fields?
    //   1. Zero-copy: we can cast DMA buffers directly (with from_bytes).
    //   2. Serialisation is trivial: to_bytes() is a single copy.
    //   3. The struct is exactly the wire size — no padding surprises.
    raw: [u8; 6],
}

impl CcsdsPrimaryHeader {
    // ── Internal helpers ──────────────────────────────────────────────────

    /// Returns the 16-bit word at byte offset 0 (big-endian).
    #[inline]
    fn word0(&self) -> u16 {
        u16::from_be_bytes([self.raw[0], self.raw[1]])
    }

    /// Returns the 16-bit word at byte offset 2 (big-endian).
    #[inline]
    fn word1(&self) -> u16 {
        u16::from_be_bytes([self.raw[2], self.raw[3]])
    }

    /// Returns the 16-bit word at byte offset 4 (big-endian).
    #[inline]
    fn word2(&self) -> u16 {
        u16::from_be_bytes([self.raw[4], self.raw[5]])
    }

    // ── Constructors ──────────────────────────────────────────────────────

    /// Creates a new Telecommand (TC) primary header.
    ///
    /// Sets packet_type = 1 (TC), secondary_header_flag = 1 (present),
    /// and sequence_flags = 0b11 (standalone packet — no segmentation).
    ///
    /// # Arguments
    ///
    /// - `apid`: Application Process Identifier, range `0x000..=0x7FE`.
    ///   Value `0x7FF` is reserved as the idle APID and is therefore rejected.
    /// - `seq_count`: Packet sequence count in `0..=0x3FFF`.
    ///   Callers should use [`Self::next_seq_count`] to advance the counter
    ///   correctly across wraps.
    /// - `data_len`: Length of the *packet data field* in octets.
    ///   The header stores `data_len - 1` per the CCSDS spec (see §4.1.3).
    ///   A value of `0` means the packet data field is 1 octet long.
    ///
    /// # Errors
    ///
    /// - [`CcsdsError::InvalidApid`] if `apid > 0x7FE`.
    /// - [`CcsdsError::InvalidSeqCount`] if `seq_count > 0x3FFF`.
    ///
    /// # Examples
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tc(0x100, 1, 10)?;
    /// assert_eq!(hdr.apid(), 0x100);
    /// assert_eq!(hdr.seq_count(), 1);
    /// assert_eq!(hdr.data_len(), 10);
    /// assert!(hdr.is_tc());
    /// assert!(!hdr.is_tm());
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn new_tc(apid: u16, seq_count: u16, data_len: u16) -> Result<Self, CcsdsError> {
        Self::new_inner(
            /*packet_type=*/ 1,
            /*sec_hdr=*/ 1,
            apid,
            seq_count,
            data_len,
        )
    }

    /// Creates a new Telemetry (TM) primary header.
    ///
    /// Sets packet_type = 0 (TM), secondary_header_flag = 1 (present),
    /// and sequence_flags = 0b11 (standalone packet).
    ///
    /// # Arguments
    ///
    /// See [`Self::new_tc`] — the arguments are identical.
    ///
    /// # Errors
    ///
    /// - [`CcsdsError::InvalidApid`] if `apid > 0x7FE`.
    /// - [`CcsdsError::InvalidSeqCount`] if `seq_count > 0x3FFF`.
    ///
    /// # Examples
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tm(0x200, 42, 128)?;
    /// assert!(hdr.is_tm());
    /// assert!(!hdr.is_tc());
    /// assert_eq!(hdr.apid(), 0x200);
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn new_tm(apid: u16, seq_count: u16, data_len: u16) -> Result<Self, CcsdsError> {
        Self::new_inner(
            /*packet_type=*/ 0,
            /*sec_hdr=*/ 1,
            apid,
            seq_count,
            data_len,
        )
    }

    /// Shared construction logic.
    ///
    /// Private: callers must use [`Self::new_tc`] or [`Self::new_tm`].
    fn new_inner(
        packet_type: u16,
        sec_hdr: u16,
        apid: u16,
        seq_count: u16,
        data_len: u16,
    ) -> Result<Self, CcsdsError> {
        // Validate APID: must fit in 11 bits and not be the idle APID (0x7FF).
        //
        // WHY reject 0x7FF? Per CCSDS 133.0-B-2 §4.1.2.3.2, the idle packet
        // APID is reserved for fill packets. Creating a "real" header with the
        // idle APID would make it indistinguishable from fill and could cause
        // receivers to silently discard it.
        if apid > 0x7FE {
            return Err(CcsdsError::InvalidApid { value: apid });
        }

        // Validate sequence count: must fit in 14 bits.
        if seq_count > 0x3FFF {
            return Err(CcsdsError::InvalidSeqCount { value: seq_count });
        }

        // Build Word 0:
        //   [15:13] version = 0b000
        //   [12]    packet_type
        //   [11]    secondary header flag
        //   [10:0]  APID
        //
        // WHY explicit shifts instead of a bitfield crate? Bitfield crates
        // add a dependency and abstract away the wire format. Here we want the
        // trainee to see the exact CCSDS bit layout in code.
        let word0: u16 = (packet_type << 12) | (sec_hdr << 11) | (apid & 0x07FF);

        // Build Word 1:
        //   [15:14] sequence flags = 0b11 (standalone / unsegmented)
        //   [13:0]  sequence count
        //
        // Standalone (0b11) means this packet is complete on its own and is not
        // a segment of a larger PDU. Flight software usually sends standalone packets;
        // segmentation is rare.
        let word1: u16 = (0b11 << 14) | (seq_count & 0x3FFF);

        // Build Word 2:
        //   [15:0]  packet data length (= total data field octets - 1)
        //
        // WHY minus one? CCSDS §4.1.3.2: "The Packet Data Length is a 16-bit
        // field containing a value that is one fewer than the length in octets
        // of the Packet Data Field." This is a classic off-by-one that trips up
        // new engineers. We store it exactly as the wire format specifies.
        let word2: u16 = data_len.saturating_sub(1);

        let w0 = word0.to_be_bytes();
        let w1 = word1.to_be_bytes();
        let w2 = word2.to_be_bytes();

        Ok(Self {
            raw: [w0[0], w0[1], w1[0], w1[1], w2[0], w2[1]],
        })
    }

    // ── Field Accessors ───────────────────────────────────────────────────

    /// Returns the Application Process Identifier (APID).
    ///
    /// The APID occupies bits \[10:0\] of the first 16-bit word.
    /// Valid range is `0x000..=0x7FE`; `0x7FF` (idle) is never returned
    /// because the constructor rejects it.
    ///
    /// In a flight system, the APID identifies the on-board *application process*
    /// — roughly equivalent to a process or task ID for routing purposes.
    ///
    /// # Examples
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tc(0x1AB, 0, 8)?;
    /// assert_eq!(hdr.apid(), 0x1AB);
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn apid(&self) -> u16 {
        self.word0() & 0x07FF
    }

    /// Returns the Packet Sequence Count (14-bit).
    ///
    /// The sequence count occupies bits \[13:0\] of the second 16-bit word.
    /// It increments by 1 for each new packet on a given APID and wraps at
    /// `0x3FFF` (16383) back to 0.
    ///
    /// Receivers use the sequence count to detect lost packets: a gap in the
    /// count (e.g., jumping from 5 to 7) indicates packet 6 was lost.
    ///
    /// Use [`Self::next_seq_count`] to advance the counter with correct wrapping.
    ///
    /// # Examples
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tm(0x10, 999, 32)?;
    /// assert_eq!(hdr.seq_count(), 999);
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn seq_count(&self) -> u16 {
        self.word1() & 0x3FFF
    }

    /// Returns the Packet Data Length field value.
    ///
    /// Per CCSDS 133.0-B-2 §4.1.3.2, the stored value is `(data_octets - 1)`.
    /// This accessor returns the raw stored value, so callers that want the
    /// actual data field size must add 1: `hdr.data_len() + 1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// // Create a header with a 10-octet data field.
    /// // The constructor sets the stored value to 10 - 1 = 9.
    /// let hdr = CcsdsPrimaryHeader::new_tc(0x01, 0, 10)?;
    /// assert_eq!(hdr.data_len(), 9);          // stored value
    /// assert_eq!(hdr.data_len() + 1, 10);     // actual data field size
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn data_len(&self) -> u16 {
        self.word2()
    }

    /// Returns `true` if this is a Telecommand (TC) packet.
    ///
    /// Checks bit \[12\] of word 0. TC packets originate at the ground and
    /// are uplinked to the spacecraft.
    ///
    /// # Examples
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let tc = CcsdsPrimaryHeader::new_tc(0x01, 0, 4)?;
    /// assert!(tc.is_tc());
    /// assert!(!tc.is_tm());
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn is_tc(&self) -> bool {
        (self.word0() >> 12) & 1 == 1
    }

    /// Returns `true` if this is a Telemetry (TM) packet.
    ///
    /// Checks bit \[12\] of word 0. TM packets originate on the spacecraft and
    /// are downlinked to the ground.
    ///
    /// # Examples
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let tm = CcsdsPrimaryHeader::new_tm(0x02, 5, 64)?;
    /// assert!(tm.is_tm());
    /// assert!(!tm.is_tc());
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn is_tm(&self) -> bool {
        !self.is_tc()
    }

    /// Returns `true` if the secondary header flag is set.
    ///
    /// Bit \[11\] of word 0. When set, a secondary header immediately follows
    /// the primary header in the packet data field. PUS packets always have
    /// a secondary header.
    pub fn has_secondary_header(&self) -> bool {
        (self.word0() >> 11) & 1 == 1
    }

    /// Returns the raw 6-byte representation of the header.
    ///
    /// The bytes are in big-endian wire order, suitable for direct
    /// transmission or DMA transfer.
    ///
    /// # Examples
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tc(0x01, 0, 4)?;
    /// let bytes = hdr.to_bytes();
    /// assert_eq!(bytes.len(), 6);
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn to_bytes(&self) -> [u8; 6] {
        self.raw
    }

    /// Constructs a [`CcsdsPrimaryHeader`] from a 6-byte array.
    ///
    /// Validates that the version field is zero and that the APID is not
    /// the idle APID (`0x7FF`). Does not validate `seq_count` because a
    /// parsed packet can legitimately have any 14-bit value.
    ///
    /// # Errors
    ///
    /// - [`CcsdsError::UnsupportedVersion`] if bits \[15:13\] of byte 0 are non-zero.
    /// - [`CcsdsError::InvalidApid`] if the APID field equals `0x7FF` (idle).
    ///
    /// # Examples
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tc(0x42, 7, 20)?;
    /// let bytes = hdr.to_bytes();
    /// let decoded = CcsdsPrimaryHeader::from_bytes(bytes)?;
    /// assert_eq!(decoded, hdr);
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn from_bytes(bytes: [u8; 6]) -> Result<Self, CcsdsError> {
        let word0 = u16::from_be_bytes([bytes[0], bytes[1]]);

        // Check version field — must be 0.
        let version = (word0 >> 13) & 0b111;
        if version != 0 {
            return Err(CcsdsError::UnsupportedVersion { version: version as u8 });
        }

        // Check APID — reject idle APID (0x7FF).
        let apid = word0 & 0x07FF;
        if apid > 0x7FE {
            return Err(CcsdsError::InvalidApid { value: apid });
        }

        Ok(Self { raw: bytes })
    }

    // ── Utility ───────────────────────────────────────────────────────────

    /// Advances a sequence counter by one, wrapping at the 14-bit boundary.
    ///
    /// The CCSDS sequence count is 14 bits wide (max `0x3FFF` = 16383). After
    /// reaching the maximum, it wraps to `0`. This function applies the wrap
    /// correctly without the caller needing to know the mask value.
    ///
    /// In a real flight system you would maintain a `HashMap<u16, u16>` keyed
    /// by APID and use this function to advance each APID's counter independently.
    ///
    /// # Examples
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// assert_eq!(CcsdsPrimaryHeader::next_seq_count(0),      1);
    /// assert_eq!(CcsdsPrimaryHeader::next_seq_count(16382), 16383);
    ///
    /// // Wrap-around at 14-bit boundary:
    /// assert_eq!(CcsdsPrimaryHeader::next_seq_count(0x3FFF), 0);
    /// ```
    pub fn next_seq_count(current: u16) -> u16 {
        // Apply 14-bit mask after addition to enforce wrap-around.
        // Using & instead of % because & is a single CPU instruction and
        // can never overflow.
        (current.wrapping_add(1)) & 0x3FFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_tc() {
        let hdr = CcsdsPrimaryHeader::new_tc(0x100, 42, 10).unwrap();
        let bytes = hdr.to_bytes();
        let decoded = CcsdsPrimaryHeader::from_bytes(bytes).unwrap();
        assert_eq!(hdr, decoded);
        assert!(decoded.is_tc());
        assert_eq!(decoded.apid(), 0x100);
        assert_eq!(decoded.seq_count(), 42);
    }

    #[test]
    fn round_trip_tm() {
        let hdr = CcsdsPrimaryHeader::new_tm(0x200, 1, 64).unwrap();
        let bytes = hdr.to_bytes();
        let decoded = CcsdsPrimaryHeader::from_bytes(bytes).unwrap();
        assert_eq!(hdr, decoded);
        assert!(decoded.is_tm());
    }

    #[test]
    fn invalid_apid_rejected() {
        assert!(matches!(
            CcsdsPrimaryHeader::new_tc(0x7FF, 0, 4),
            Err(CcsdsError::InvalidApid { value: 0x7FF })
        ));
        assert!(matches!(
            CcsdsPrimaryHeader::new_tc(0x800, 0, 4),
            Err(CcsdsError::InvalidApid { .. })
        ));
    }

    #[test]
    fn seq_count_wraps_correctly() {
        assert_eq!(CcsdsPrimaryHeader::next_seq_count(0x3FFF), 0);
        assert_eq!(CcsdsPrimaryHeader::next_seq_count(0x3FFE), 0x3FFF);
        assert_eq!(CcsdsPrimaryHeader::next_seq_count(0), 1);
    }

    #[test]
    fn data_len_stored_as_minus_one() {
        // 10-octet data field => stored value is 9.
        let hdr = CcsdsPrimaryHeader::new_tc(0x01, 0, 10).unwrap();
        assert_eq!(hdr.data_len(), 9);
    }
}
