/// Example 01 — Hand-written FFI bindings
///
/// This is the raw layer: we declare C types and function signatures manually.
/// This is exactly what `bindgen` generates automatically, but writing it by hand
/// once teaches you what bindgen is doing and why each piece is needed.
///
/// Key concepts demonstrated:
///   - `#[repr(C)]` on a struct: makes Rust use C-compatible memory layout
///   - `extern "C"` block: declares functions that exist in linked C libraries
///   - `unsafe` block: where YOU assert the safety invariants the compiler can't check
///   - The calling convention: Rust generates a call that matches what C expects

// ──────────────────────────────────────────────────────────────────────────────
// Step 1: Re-declare the C struct in Rust
//
// C header says:
//   struct CcsdsPrimaryHeader { uint8_t raw[6]; };
//
// We must add #[repr(C)] so that Rust lays out the struct exactly as C would.
// Without it, Rust could theoretically reorder fields or add padding, making the
// struct incompatible — even though this one has only a single array field.
// It's a good habit to always use #[repr(C)] for any type crossing the boundary.
// ──────────────────────────────────────────────────────────────────────────────
#[repr(C)]
pub struct CcsdsPrimaryHeader {
    raw: [u8; 6],
}

// ──────────────────────────────────────────────────────────────────────────────
// Step 2: Declare the C functions in an extern "C" block
//
// "extern "C"" tells Rust: use the C ABI (System V AMD64 on x86-64 Linux,
// AAPCS on ARM). The linker will resolve these to symbols in libccsds_framer.a,
// which Cargo compiled via build.rs using the `cc` crate.
//
// The signatures MUST match the C header exactly:
//   - C `int`    ↔ Rust `i32`    (both are 32-bit signed on all platforms we care about)
//   - C `uint16_t` ↔ Rust `u16`
//   - C pointer `T*` ↔ Rust `*mut T`
//   - C const pointer `const T*` ↔ Rust `*const T`
//
// If you get a signature wrong, you will NOT get a compile error. You'll get
// wrong values, stack corruption, or a crash at runtime. Treat these
// declarations with the same care you'd give a safety-critical register map.
// ──────────────────────────────────────────────────────────────────────────────
extern "C" {
    /// Pack CCSDS primary header fields into 6 raw bytes.
    ///
    /// C signature:
    ///   int ccsds_pack(struct CcsdsPrimaryHeader *hdr,
    ///                  uint16_t apid, uint16_t seq_count,
    ///                  uint16_t data_len, int is_tc);
    ///
    /// Returns 0 on success, -1 if any argument is out of range or hdr is NULL.
    fn ccsds_pack(
        hdr: *mut CcsdsPrimaryHeader,
        apid: u16,
        seq_count: u16,
        data_len: u16,
        is_tc: i32,
    ) -> i32;

    /// Extract CCSDS primary header fields from 6 raw bytes.
    ///
    /// C signature:
    ///   int ccsds_unpack(const struct CcsdsPrimaryHeader *hdr,
    ///                    uint16_t *apid, uint16_t *seq_count, uint16_t *data_len);
    ///
    /// Output pointer arguments may be NULL if that field is not needed.
    /// Returns 0 on success, -1 if hdr is NULL.
    fn ccsds_unpack(
        hdr: *const CcsdsPrimaryHeader,
        apid: *mut u16,
        seq_count: *mut u16,
        data_len: *mut u16,
    ) -> i32;

    /// Returns non-zero if the type bit in the header is set (Telecommand).
    fn ccsds_is_tc(hdr: *const CcsdsPrimaryHeader) -> i32;
}

fn main() {
    println!("=== Example 01: Manual FFI Bindings ===\n");

    // ──────────────────────────────────────────────────────────────────────────
    // Build a TC packet with:
    //   APID       = 0x100  (256 decimal — a hypothetical attitude control APID)
    //   seq_count  = 1
    //   data_len   = 4      (4 bytes of payload; stored as 3 per CCSDS spec)
    //   is_tc      = 1      (Telecommand)
    // ──────────────────────────────────────────────────────────────────────────

    // Start with zeroed memory. We could use MaybeUninit here, but zeroing is
    // safe and makes the "before" state clear for educational purposes.
    let mut header = CcsdsPrimaryHeader { raw: [0u8; 6] };

    // ──────────────────────────────────────────────────────────────────────────
    // unsafe block: we are calling foreign (C) code.
    //
    // Invariants WE are asserting here (the compiler cannot check these):
    //   1. `header` is properly aligned for CcsdsPrimaryHeader (it is — it's a
    //      local stack variable with #[repr(C)]).
    //   2. `&mut header` is non-null (guaranteed — it's a reference, not a raw ptr).
    //   3. `header` is exclusively borrowed for the duration of this call
    //      (guaranteed by Rust's borrow checker on `&mut header`).
    //   4. The C function will not store the pointer past the call (it won't —
    //      ccsds_pack only writes to the struct synchronously).
    // ──────────────────────────────────────────────────────────────────────────
    let pack_result = unsafe {
        ccsds_pack(
            &mut header as *mut CcsdsPrimaryHeader,
            0x100, // apid
            1,     // seq_count
            4,     // data_len (stored as 3 in the header, per CCSDS: data_len - 1)
            1,     // is_tc = true
        )
    };

    // C functions return error codes, not Rust Results. Always check them.
    assert_eq!(pack_result, 0, "ccsds_pack failed with error {}", pack_result);

    println!("Packed TC header (APID=0x100, seq=1, data_len=4):");
    println!("  Raw bytes: {:02X?}", header.raw);
    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // Let's verify the bytes manually using our knowledge of the CCSDS bit layout:
    //
    //   Byte 0: [V V V T S A A A]  → version=000, type=1(TC), sec=0, apid[10:8]=001
    //           = 0b0001_0001 = 0x11
    //   Byte 1: [A A A A A A A A]  → apid[7:0] = 0x00
    //           = 0x00
    //   Byte 2: [F F C C C C C C]  → seq_flags=11, seq_count[13:8]=000000
    //           = 0b1100_0000 = 0xC0
    //   Byte 3: [C C C C C C C C]  → seq_count[7:0] = 0x01
    //           = 0x01
    //   Byte 4: [L L L L L L L L]  → (data_len-1)[15:8] = 0x00
    //           = 0x00
    //   Byte 5: [L L L L L L L L]  → (data_len-1)[7:0] = 0x03
    //           = 0x03
    // ──────────────────────────────────────────────────────────────────────────
    println!("Expected: [11, 00, C0, 01, 00, 03]");
    assert_eq!(header.raw[0], 0x11, "Byte 0 wrong: version/type/apid high");
    assert_eq!(header.raw[1], 0x00, "Byte 1 wrong: apid low");
    assert_eq!(header.raw[2], 0xC0, "Byte 2 wrong: seq_flags/seq_count high");
    assert_eq!(header.raw[3], 0x01, "Byte 3 wrong: seq_count low");
    assert_eq!(header.raw[4], 0x00, "Byte 4 wrong: data_len-1 high");
    assert_eq!(header.raw[5], 0x03, "Byte 5 wrong: data_len-1 low");
    println!("Byte-level verification: PASSED\n");

    // ──────────────────────────────────────────────────────────────────────────
    // Now unpack the header and verify we get back the same values.
    //
    // We use local variables and pass their addresses as output parameters,
    // matching C's "output parameter" idiom (C has no tuples or Result type).
    // ──────────────────────────────────────────────────────────────────────────
    let mut apid: u16 = 0;
    let mut seq_count: u16 = 0;
    let mut data_len: u16 = 0;

    // Safety: `header` is valid, aligned, and lives past this call.
    // apid, seq_count, data_len are local variables on the stack.
    let unpack_result = unsafe {
        ccsds_unpack(
            &header as *const CcsdsPrimaryHeader,
            &mut apid as *mut u16,
            &mut seq_count as *mut u16,
            &mut data_len as *mut u16,
        )
    };

    assert_eq!(unpack_result, 0, "ccsds_unpack failed");

    println!("Unpacked fields:");
    println!("  APID:      0x{:03X} (expected 0x100)", apid);
    println!("  seq_count: {}      (expected 1)", seq_count);
    println!("  data_len:  {}      (expected 4)", data_len);

    assert_eq!(apid, 0x100);
    assert_eq!(seq_count, 1);
    assert_eq!(data_len, 4);

    // Safety: header is valid and non-null.
    let is_tc = unsafe { ccsds_is_tc(&header as *const CcsdsPrimaryHeader) };
    println!("  is_tc:     {}      (expected 1)", is_tc);
    assert_ne!(is_tc, 0);

    println!("\nRound-trip verification: PASSED");
    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // Also test a TM (telemetry) packet to make sure the type bit is cleared.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- TM packet test ---");
    let mut tm_header = CcsdsPrimaryHeader { raw: [0u8; 6] };

    // Safety: same reasoning as above.
    let ret = unsafe { ccsds_pack(&mut tm_header, 0x050, 42, 16, 0 /* is_tc=false */) };
    assert_eq!(ret, 0);

    println!("TM header (APID=0x050, seq=42, data_len=16):");
    println!("  Raw bytes: {:02X?}", tm_header.raw);

    // Safety: tm_header is valid.
    let tm_is_tc = unsafe { ccsds_is_tc(&tm_header as *const CcsdsPrimaryHeader) };
    println!("  is_tc: {} (expected 0 for TM)", tm_is_tc);
    assert_eq!(tm_is_tc, 0);

    let mut out_apid: u16 = 0;
    let mut out_seq: u16 = 0;
    let mut out_len: u16 = 0;
    // Safety: tm_header is valid.
    unsafe {
        ccsds_unpack(&tm_header, &mut out_apid, &mut out_seq, &mut out_len);
    }
    assert_eq!(out_apid, 0x050);
    assert_eq!(out_seq, 42);
    assert_eq!(out_len, 16);
    println!("TM round-trip: PASSED\n");

    // ──────────────────────────────────────────────────────────────────────────
    // Test error handling: ccsds_pack should reject invalid arguments.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Error handling test ---");
    let mut bad_header = CcsdsPrimaryHeader { raw: [0u8; 6] };

    // APID 0x800 = 2048 = one past the valid 11-bit range (0-2047).
    // Safety: bad_header is a valid allocation; we're testing C's error path.
    let bad_ret = unsafe { ccsds_pack(&mut bad_header, 0x800, 0, 1, 0) };
    println!("  ccsds_pack(apid=0x800): returned {} (expected -1)", bad_ret);
    assert_eq!(bad_ret, -1);

    // data_len=0 is invalid per CCSDS (stored as data_len-1 would wrap).
    let bad_ret2 = unsafe { ccsds_pack(&mut bad_header, 0x001, 0, 0, 0) };
    println!("  ccsds_pack(data_len=0): returned {} (expected -1)", bad_ret2);
    assert_eq!(bad_ret2, -1);

    println!("\nError handling test: PASSED");
}
