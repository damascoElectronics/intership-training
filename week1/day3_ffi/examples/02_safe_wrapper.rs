/// Example 02 — Building a safe Rust API around C functions
///
/// The previous example showed the raw FFI layer. This example shows the critical
/// next step: wrapping the unsafe FFI calls in a safe public API.
///
/// The pattern:
///   1. Keep the `extern "C"` declarations private (module-private or crate-private)
///   2. Build a Rust struct/impl that validates all inputs BEFORE calling C
///   3. Map C error codes to a Rust `Result` type
///   4. After this wrapper, all callers write 100% safe Rust
///
/// Why this matters: the entire rest of your codebase doesn't need to know
/// about C, unsafe, or CCSDS bit manipulation. Only this module does.

// We declare the low-level bindings privately — callers use CcsdsHeader, not these.
#[repr(C)]
struct CcsdsPrimaryHeaderRaw {
    raw: [u8; 6],
}

extern "C" {
    fn ccsds_pack(
        hdr: *mut CcsdsPrimaryHeaderRaw,
        apid: u16,
        seq_count: u16,
        data_len: u16,
        is_tc: i32,
    ) -> i32;

    fn ccsds_unpack(
        hdr: *const CcsdsPrimaryHeaderRaw,
        apid: *mut u16,
        seq_count: *mut u16,
        data_len: *mut u16,
    ) -> i32;

    fn ccsds_is_tc(hdr: *const CcsdsPrimaryHeaderRaw) -> i32;
}

// ──────────────────────────────────────────────────────────────────────────────
// Error type: represent every way the API can fail as a Rust enum.
//
// Using an enum (rather than string messages or i32 codes) means callers can
// pattern-match on the exact error and handle it programmatically.
// ──────────────────────────────────────────────────────────────────────────────
#[derive(Debug, PartialEq)]
pub enum CcsdsError {
    /// APID must be 0–2047 (11 bits).
    ApidOutOfRange { provided: u16 },
    /// Sequence count must be 0–16383 (14 bits).
    SeqCountOutOfRange { provided: u16 },
    /// data_len must be ≥ 1 (CCSDS stores data_len-1; zero is invalid).
    DataLenZero,
    /// The underlying C function returned an unexpected error code.
    CFunctionFailed { code: i32 },
}

impl std::fmt::Display for CcsdsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CcsdsError::ApidOutOfRange { provided } => {
                write!(f, "APID 0x{:03X} is out of range (max 0x7FF = 2047)", provided)
            }
            CcsdsError::SeqCountOutOfRange { provided } => {
                write!(
                    f,
                    "seq_count {} is out of range (max 16383 = 0x3FFF)",
                    provided
                )
            }
            CcsdsError::DataLenZero => write!(f, "data_len must be ≥ 1"),
            CcsdsError::CFunctionFailed { code } => {
                write!(f, "C function returned error code {}", code)
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// The public safe API
// ──────────────────────────────────────────────────────────────────────────────

/// A validated CCSDS primary header.
///
/// Invariants (maintained by the constructors):
///   - apid ≤ 2047
///   - seq_count ≤ 16383
///   - data_len ≥ 1
///   - `inner.raw` contains the correct byte encoding
///
/// Once constructed, all these invariants hold and callers don't need unsafe.
#[derive(Debug, Clone, PartialEq)]
pub struct CcsdsHeader {
    // We store the raw bytes (as produced by C) rather than the decoded fields.
    // This means `as_bytes()` is a zero-copy view into the already-encoded header.
    inner: [u8; 6],
}

impl CcsdsHeader {
    // ──────────────────────────────────────────────────────────────────────────
    // Private helper: calls C and converts its error code to our error type.
    // This is the ONLY place unsafe appears in the entire public API.
    // ──────────────────────────────────────────────────────────────────────────
    fn pack_validated(
        apid: u16,
        seq_count: u16,
        data_len: u16,
        is_tc: bool,
    ) -> Result<[u8; 6], CcsdsError> {
        // Validate BEFORE calling C. We own these checks; C also checks them,
        // but we want precise Rust error variants, not just "C returned -1".
        if apid > 0x07FF {
            return Err(CcsdsError::ApidOutOfRange { provided: apid });
        }
        if seq_count > 0x3FFF {
            return Err(CcsdsError::SeqCountOutOfRange { provided: seq_count });
        }
        if data_len == 0 {
            return Err(CcsdsError::DataLenZero);
        }

        let mut raw_hdr = CcsdsPrimaryHeaderRaw { raw: [0u8; 6] };

        // Safety invariants we assert here:
        //   1. raw_hdr is a valid, aligned, locally-owned CcsdsPrimaryHeaderRaw.
        //   2. We just validated all input arguments, so C's preconditions are met.
        //   3. C will not store the pointer past this call.
        let ret = unsafe {
            ccsds_pack(
                &mut raw_hdr as *mut CcsdsPrimaryHeaderRaw,
                apid,
                seq_count,
                data_len,
                if is_tc { 1 } else { 0 },
            )
        };

        if ret != 0 {
            // This shouldn't happen since we pre-validated, but handle it anyway.
            return Err(CcsdsError::CFunctionFailed { code: ret });
        }

        Ok(raw_hdr.raw)
    }

    /// Create a new Telecommand (TC) header.
    ///
    /// Returns `Err` if any field is out of range.
    ///
    /// CCSDS field constraints:
    ///   - `apid`: 0–2047 (11 bits)
    ///   - `seq_count`: 0–16383 (14 bits)
    ///   - `data_len`: ≥ 1 (stored as `data_len - 1` per CCSDS spec)
    pub fn new_tc(apid: u16, seq_count: u16, data_len: u16) -> Result<Self, CcsdsError> {
        let bytes = Self::pack_validated(apid, seq_count, data_len, true)?;
        Ok(CcsdsHeader { inner: bytes })
    }

    /// Create a new Telemetry (TM) header.
    pub fn new_tm(apid: u16, seq_count: u16, data_len: u16) -> Result<Self, CcsdsError> {
        let bytes = Self::pack_validated(apid, seq_count, data_len, false)?;
        Ok(CcsdsHeader { inner: bytes })
    }

    /// Decode an existing raw 6-byte CCSDS header.
    ///
    /// This is useful when you received bytes over a serial link and want
    /// a structured view.
    pub fn from_bytes(raw: [u8; 6]) -> Self {
        // We trust the bytes as-is. If they came from the wire, they may or may not
        // be valid CCSDS — the caller's responsibility. We just provide the decode API.
        CcsdsHeader { inner: raw }
    }

    /// The raw 6-byte header for transmission.
    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.inner
    }

    /// The Application Process Identifier (11 bits, 0–2047).
    pub fn apid(&self) -> u16 {
        // Safety: self.inner is always a valid 6-byte buffer.
        let mut apid: u16 = 0;
        let raw = CcsdsPrimaryHeaderRaw { raw: self.inner };
        unsafe {
            ccsds_unpack(&raw, &mut apid, std::ptr::null_mut(), std::ptr::null_mut());
        }
        apid
    }

    /// The sequence count (14 bits, 0–16383).
    pub fn seq_count(&self) -> u16 {
        let mut seq: u16 = 0;
        let raw = CcsdsPrimaryHeaderRaw { raw: self.inner };
        // Safety: raw is a valid local copy of our 6-byte buffer.
        unsafe {
            ccsds_unpack(&raw, std::ptr::null_mut(), &mut seq, std::ptr::null_mut());
        }
        seq
    }

    /// The data field length in bytes (always ≥ 1).
    ///
    /// This is the *actual* byte count of the data field, not the stored value
    /// (which is `data_len - 1` per CCSDS). The wrapper handles this adjustment.
    pub fn data_len(&self) -> u16 {
        let mut len: u16 = 0;
        let raw = CcsdsPrimaryHeaderRaw { raw: self.inner };
        // Safety: raw is a valid local copy.
        unsafe {
            ccsds_unpack(&raw, std::ptr::null_mut(), std::ptr::null_mut(), &mut len);
        }
        len
    }

    /// Returns `true` if this is a Telecommand (TC) packet.
    pub fn is_tc(&self) -> bool {
        let raw = CcsdsPrimaryHeaderRaw { raw: self.inner };
        // Safety: raw is a valid local copy.
        let result = unsafe { ccsds_is_tc(&raw) };
        result != 0
    }

    /// Returns `true` if this is a Telemetry (TM) packet.
    pub fn is_tm(&self) -> bool {
        !self.is_tc()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Display implementation for human-readable output
// ──────────────────────────────────────────────────────────────────────────────
impl std::fmt::Display for CcsdsHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CcsdsHeader {{ type: {}, apid: 0x{:03X}, seq: {}, data_len: {}, bytes: {:02X?} }}",
            if self.is_tc() { "TC" } else { "TM" },
            self.apid(),
            self.seq_count(),
            self.data_len(),
            self.inner,
        )
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Unit tests
//
// These live in the same file for this example. In a real codebase they'd live
// in a `#[cfg(test)] mod tests { ... }` block or a separate test file.
//
// Run with: cargo test --example 02_safe_wrapper
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip: pack a TC header, then read the fields back.
    #[test]
    fn tc_round_trip() {
        let hdr = CcsdsHeader::new_tc(0x100, 7, 12).expect("new_tc should succeed");
        assert_eq!(hdr.apid(), 0x100);
        assert_eq!(hdr.seq_count(), 7);
        assert_eq!(hdr.data_len(), 12);
        assert!(hdr.is_tc());
        assert!(!hdr.is_tm());
    }

    /// Round-trip: pack a TM header, then read the fields back.
    #[test]
    fn tm_round_trip() {
        let hdr = CcsdsHeader::new_tm(0x050, 1023, 64).expect("new_tm should succeed");
        assert_eq!(hdr.apid(), 0x050);
        assert_eq!(hdr.seq_count(), 1023);
        assert_eq!(hdr.data_len(), 64);
        assert!(hdr.is_tm());
        assert!(!hdr.is_tc());
    }

    /// Edge case: maximum valid field values.
    #[test]
    fn max_valid_fields() {
        let hdr = CcsdsHeader::new_tm(0x7FF, 0x3FFF, 0xFFFF).expect("max valid fields");
        assert_eq!(hdr.apid(), 0x7FF);
        assert_eq!(hdr.seq_count(), 0x3FFF);
        assert_eq!(hdr.data_len(), 0xFFFF);
    }

    /// Edge case: minimum valid field values.
    #[test]
    fn min_valid_fields() {
        let hdr = CcsdsHeader::new_tc(0, 0, 1).expect("min valid fields");
        assert_eq!(hdr.apid(), 0);
        assert_eq!(hdr.seq_count(), 0);
        assert_eq!(hdr.data_len(), 1);
    }

    /// APID = 2048 is one past the valid 11-bit range (max = 2047).
    #[test]
    fn apid_out_of_range() {
        let err = CcsdsHeader::new_tc(2048, 0, 1).expect_err("should reject APID 2048");
        assert_eq!(err, CcsdsError::ApidOutOfRange { provided: 2048 });
    }

    /// seq_count = 16384 is one past the valid 14-bit range (max = 16383).
    #[test]
    fn seq_count_out_of_range() {
        let err =
            CcsdsHeader::new_tm(0, 16384, 1).expect_err("should reject seq_count 16384");
        assert_eq!(err, CcsdsError::SeqCountOutOfRange { provided: 16384 });
    }

    /// data_len = 0 is invalid because the CCSDS field stores data_len-1,
    /// and underflow would make the header encode an incorrect length.
    #[test]
    fn data_len_zero_rejected() {
        let err = CcsdsHeader::new_tc(1, 0, 0).expect_err("should reject data_len=0");
        assert_eq!(err, CcsdsError::DataLenZero);
    }

    /// Two headers with the same fields should have the same byte representation.
    #[test]
    fn deterministic_encoding() {
        let a = CcsdsHeader::new_tc(0x200, 5, 8).unwrap();
        let b = CcsdsHeader::new_tc(0x200, 5, 8).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    /// A TC and TM with the same APID must differ in their type bit (byte 0, bit 4).
    #[test]
    fn tc_and_tm_differ_in_type_bit() {
        let tc = CcsdsHeader::new_tc(0x100, 1, 4).unwrap();
        let tm = CcsdsHeader::new_tm(0x100, 1, 4).unwrap();
        // Byte 0 has the type bit at bit 4; TC must have it set.
        assert_ne!(tc.as_bytes()[0], tm.as_bytes()[0]);
        // Bytes 1–5 must be the same (only type bit differs).
        assert_eq!(tc.as_bytes()[1..], tm.as_bytes()[1..]);
    }
}

fn main() {
    println!("=== Example 02: Safe Wrapper around C FFI ===\n");

    // ──────────────────────────────────────────────────────────────────────────
    // Using the safe API: no unsafe anywhere in this code path.
    // All the unsafe is hidden inside CcsdsHeader's private helpers.
    // ──────────────────────────────────────────────────────────────────────────

    // Create a TC header — the API validates our inputs before touching C.
    let tc_header = CcsdsHeader::new_tc(0x100, 1, 4)
        .expect("valid TC header should succeed");

    println!("TC Header: {}", tc_header);
    println!("  as_bytes: {:02X?}", tc_header.as_bytes());
    println!();

    // Create a TM header — for telemetry going back to ground.
    let tm_header = CcsdsHeader::new_tm(0x050, 42, 64)
        .expect("valid TM header should succeed");

    println!("TM Header: {}", tm_header);
    println!("  as_bytes: {:02X?}", tm_header.as_bytes());
    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // Demonstrate error handling: invalid inputs return Err, not a crash.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Error handling ---");

    match CcsdsHeader::new_tc(0x800, 0, 1) {
        Ok(_) => panic!("should have been rejected"),
        Err(e) => println!("  APID too large: {}", e),
    }

    match CcsdsHeader::new_tc(0, 20000, 1) {
        Ok(_) => panic!("should have been rejected"),
        Err(e) => println!("  seq_count too large: {}", e),
    }

    match CcsdsHeader::new_tc(0, 0, 0) {
        Ok(_) => panic!("should have been rejected"),
        Err(e) => println!("  data_len zero: {}", e),
    }

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // Demonstrate from_bytes: decode a header received from the wire.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- from_bytes (decode received header) ---");
    let raw_from_wire: [u8; 6] = [0x11, 0x00, 0xC0, 0x01, 0x00, 0x03];
    let decoded = CcsdsHeader::from_bytes(raw_from_wire);
    println!("  Decoded: {}", decoded);
    assert_eq!(decoded.apid(), 0x100);
    assert_eq!(decoded.seq_count(), 1);
    assert_eq!(decoded.data_len(), 4);
    assert!(decoded.is_tc());

    println!("\nAll safe wrapper demonstrations: PASSED");
}
