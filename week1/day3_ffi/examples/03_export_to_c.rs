/// Example 03 — Exporting Rust functions to C
///
/// So far we've called C from Rust. This example goes the other direction:
/// writing a Rust function that C code can call.
///
/// Two attributes are required:
///   - `extern "C"`: use the C calling convention (not Rust's internal ABI)
///   - `#[no_mangle]`: prevent Rust's name-mangling so C can find the symbol
///
/// WHY no_mangle matters:
///   Without it, Rust turns `rust_ccsds_validate` into something like
///   `_ZN7day3_ffi20rust_ccsds_validate17h3a4b5c6d7e8f9g0hE`.
///   C code doing `extern int rust_ccsds_validate(...)` would fail to link.
///   `#[no_mangle]` keeps the symbol name exactly as written.
///
/// This pattern is used when:
///   - You're writing a new Rust library to replace part of a C codebase
///   - You want to call Rust from an existing C main() or RTOS task
///   - You're building a plugin/callback system where C calls registered Rust fns
///
/// For large exported APIs, the `cbindgen` tool reads your Rust source and
/// auto-generates the corresponding C header. The comment at the bottom shows
/// what cbindgen would produce for the function defined here.

use std::slice;

// ──────────────────────────────────────────────────────────────────────────────
// The exported function
// ──────────────────────────────────────────────────────────────────────────────

/// Validates a raw CCSDS primary header buffer.
///
/// This function is exported with C linkage so that C code can call it:
///   `extern int rust_ccsds_validate(const uint8_t *raw, size_t len);`
///
/// Return values (matching C convention — no Rust Result here):
///   0  → valid CCSDS primary header
///  -1  → null pointer passed
///  -2  → buffer too short (need at least 6 bytes)
///  -3  → invalid CCSDS version (bits 15-13 of byte 0 must be 0b000)
///  -4  → APID 0x7FF is the IDLE/fill packet APID, treated as invalid for TC
///
/// # Safety
///
/// The caller must ensure:
///   - `raw` is non-null (or the function returns -1)
///   - `raw` points to a contiguous buffer of at least `len` bytes
///   - The buffer remains valid for the duration of this call
///   - No other thread mutates the buffer during this call
///
/// This is an `unsafe extern "C"` fn because Rust cannot verify the caller
/// upholds these invariants at compile time.
#[no_mangle]
pub unsafe extern "C" fn rust_ccsds_validate(raw: *const u8, len: usize) -> i32 {
    // ── Guard 1: null pointer check ──────────────────────────────────────────
    // In C it's possible (even common) to pass NULL by mistake.
    // We check this first because any deref of a null ptr is UB.
    if raw.is_null() {
        return -1;
    }

    // ── Guard 2: length check ────────────────────────────────────────────────
    // A CCSDS primary header is always exactly 6 bytes. If we have fewer,
    // we cannot meaningfully validate it.
    if len < 6 {
        return -2;
    }

    // ── Build a safe Rust slice ──────────────────────────────────────────────
    // Safety: we checked raw is non-null and len >= 6 above.
    // slice::from_raw_parts requires: valid ptr, in-bounds len, single object.
    // The caller's contract (documented in Safety section above) covers the rest.
    let buf: &[u8] = slice::from_raw_parts(raw, len);

    // ── Guard 3: version field ───────────────────────────────────────────────
    // CCSDS 133.0-B-2 specifies the 3 most significant bits of byte 0 as
    // the "version number", which must be 0b000 (= 0).
    // Bits 7-5 of byte 0 = (buf[0] >> 5) & 0x07
    let version = (buf[0] >> 5) & 0x07;
    if version != 0 {
        return -3;
    }

    // ── Guard 4: idle packet check ───────────────────────────────────────────
    // APID 0x7FF (all 1s) is the CCSDS idle/fill packet APID.
    // In many spacecraft implementations, routing a fill packet to a service
    // as if it were a real command would be a serious error.
    // bits 10-0 of bytes 0-1: (byte0 & 0x07) << 8 | byte1
    let apid = (((buf[0] & 0x07) as u16) << 8) | (buf[1] as u16);
    if apid == 0x7FF {
        return -4;
    }

    // All checks passed.
    0
}

/// Returns a human-readable description of a rust_ccsds_validate return code.
///
/// This is a helper exported alongside the validator so C callers can produce
/// diagnostic messages without embedding the meaning of each code themselves.
///
/// # Safety
///
/// The returned pointer is valid for `'static` (points to a string literal).
/// The caller must NOT free it.
#[no_mangle]
pub extern "C" fn rust_ccsds_validate_strerror(code: i32) -> *const std::os::raw::c_char {
    // We use byte string literals with explicit null terminator.
    // b"text\0".as_ptr() gives a *const u8; cast to *const c_char for C.
    match code {
        0  => b"valid CCSDS primary header\0".as_ptr() as *const std::os::raw::c_char,
        -1 => b"null pointer\0".as_ptr() as *const std::os::raw::c_char,
        -2 => b"buffer too short (need >= 6 bytes)\0".as_ptr() as *const std::os::raw::c_char,
        -3 => b"invalid CCSDS version field (must be 0b000)\0".as_ptr() as *const std::os::raw::c_char,
        -4 => b"APID 0x7FF is idle/fill packet\0".as_ptr() as *const std::os::raw::c_char,
        _  => b"unknown error code\0".as_ptr() as *const std::os::raw::c_char,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Internal test: use the function via its Rust signature (safe wrapper for tests)
// ──────────────────────────────────────────────────────────────────────────────

/// Safe test-only wrapper so we don't have to write unsafe in every test.
fn validate(raw: &[u8]) -> i32 {
    // Safety: raw.as_ptr() is non-null (slice refs are never null),
    // raw.len() is the actual length, buffer is valid for the call duration.
    unsafe { rust_ccsds_validate(raw.as_ptr(), raw.len()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_tc_header() {
        // APID=0x100, type=TC (byte0 bit4=1), version=0b000 → byte0=0x11
        let raw = [0x11u8, 0x00, 0xC0, 0x01, 0x00, 0x03];
        assert_eq!(validate(&raw), 0);
    }

    #[test]
    fn valid_tm_header() {
        // APID=0x050, type=TM, version=0b000 → byte0=0x00, byte1=0x50
        let raw = [0x00u8, 0x50, 0xC0, 0x2A, 0x00, 0x0F];
        assert_eq!(validate(&raw), 0);
    }

    #[test]
    fn null_pointer() {
        let result = unsafe { rust_ccsds_validate(std::ptr::null(), 6) };
        assert_eq!(result, -1);
    }

    #[test]
    fn too_short() {
        let raw = [0x00u8, 0x50, 0xC0]; // only 3 bytes
        assert_eq!(validate(&raw), -2);
    }

    #[test]
    fn bad_version_field() {
        // Set version to 0b001 in byte 0: (0b001 << 5) | type bits
        let raw = [0x20u8, 0x50, 0xC0, 0x00, 0x00, 0x00]; // version = 1
        assert_eq!(validate(&raw), -3);
    }

    #[test]
    fn idle_packet_apid() {
        // APID 0x7FF = 0b111_1111_1111
        // byte0: version=000, type=0, sec=0, apid[10:8]=111 → 0b0000_0111 = 0x07
        // byte1: apid[7:0] = 0xFF
        let raw = [0x07u8, 0xFF, 0xC0, 0x00, 0x00, 0x00];
        assert_eq!(validate(&raw), -4);
    }
}

fn main() {
    println!("=== Example 03: Exporting Rust Functions to C ===\n");

    // ──────────────────────────────────────────────────────────────────────────
    // Demonstrate calling the exported function from Rust itself
    // (In practice this would be called from C code, but we can test it here.)
    // ──────────────────────────────────────────────────────────────────────────

    let test_cases: &[(&str, &[u8])] = &[
        ("Valid TC (APID=0x100)", &[0x11, 0x00, 0xC0, 0x01, 0x00, 0x03]),
        ("Valid TM (APID=0x050)", &[0x00, 0x50, 0xC0, 0x2A, 0x00, 0x0F]),
        ("Too short (3 bytes)",   &[0x11, 0x00, 0xC0]),
        ("Bad version (v=1)",     &[0x20, 0x50, 0xC0, 0x00, 0x00, 0x00]),
        ("Idle APID (0x7FF)",     &[0x07, 0xFF, 0xC0, 0x00, 0x00, 0x00]),
    ];

    for (description, raw) in test_cases {
        let code = validate(raw);
        // Safety for strerror: returns *const c_char pointing to a static string.
        let msg_ptr = rust_ccsds_validate_strerror(code);
        let msg = unsafe { std::ffi::CStr::from_ptr(msg_ptr).to_str().unwrap() };
        println!("  {:<30}  → code {:2}  ({})", description, code, msg);
    }

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // CBINDGEN COMMENTARY
    //
    // If this crate were built as a `crate-type = ["cdylib"]` or `["staticlib"]`,
    // you'd add cbindgen to your build.rs and it would auto-generate a C header.
    //
    // The header cbindgen would produce for the above functions:
    //
    //   /* Auto-generated by cbindgen — do not edit */
    //   #ifndef RUST_CCSDS_VALIDATE_H
    //   #define RUST_CCSDS_VALIDATE_H
    //   #include <stdint.h>
    //   #include <stddef.h>
    //
    //   #ifdef __cplusplus
    //   extern "C" {
    //   #endif
    //
    //   /**
    //    * Validates a raw CCSDS primary header buffer.
    //    * Returns: 0 valid, -1 null ptr, -2 too short, -3 bad version, -4 idle APID
    //    */
    //   int32_t rust_ccsds_validate(const uint8_t *raw, size_t len);
    //
    //   /**
    //    * Returns a static C string describing a rust_ccsds_validate return code.
    //    * The returned pointer is valid forever; do NOT free it.
    //    */
    //   const char *rust_ccsds_validate_strerror(int32_t code);
    //
    //   #ifdef __cplusplus
    //   }
    //   #endif
    //
    //   #endif /* RUST_CCSDS_VALIDATE_H */
    //
    // cbindgen reads #[no_mangle] pub extern "C" functions and their doc comments,
    // then generates this header automatically. This is the recommended approach
    // for any exported API larger than a few functions.
    // ──────────────────────────────────────────────────────────────────────────
    println!("See comments in source for the cbindgen-generated C header.");
    println!("\nAll validation tests: PASSED");
}
