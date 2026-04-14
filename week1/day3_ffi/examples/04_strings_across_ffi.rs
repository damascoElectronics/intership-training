/// Example 04 — String handling across the FFI boundary
///
/// Strings are where many FFI bugs live. The fundamental mismatch:
///
///   Rust strings:
///     - `String`: heap-allocated, UTF-8 encoded, length stored explicitly,
///       NOT null-terminated.
///     - `&str`: a borrowed view of UTF-8 bytes, NOT null-terminated.
///
///   C strings:
///     - `char *`: a pointer to bytes, null-terminated (byte value 0x00 marks the end).
///     - May or may not be UTF-8 (often ASCII in practice).
///     - May contain arbitrary bytes (not validated UTF-8).
///
/// The key types in Rust's std::ffi:
///   - `CString`:  Rust-owned, heap-allocated, null-terminated string.
///                 Created from Rust data, passed to C.
///   - `CStr`:     Borrowed view of a null-terminated byte sequence.
///                 Used to read strings that C owns.
///   - `OsString` / `OsStr`: Platform-native string type (UTF-8 on Unix,
///                 UTF-16 on Windows). Useful for file paths.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// ──────────────────────────────────────────────────────────────────────────────
// We declare a few hypothetical C functions that deal with strings.
// These are stand-ins for real device driver APIs you'd find in the wild.
// ──────────────────────────────────────────────────────────────────────────────
extern "C" {
    /// Opens a device by path, returns a file descriptor or -1.
    /// C signature: int open_device(const char *path);
    fn open_device(path: *const c_char) -> i32;

    /// Writes a log message via the C logging subsystem.
    /// C signature: void c_log_message(const char *msg);
    fn c_log_message(msg: *const c_char);

    /// Returns the device's name as a statically-allocated C string.
    /// The returned pointer is valid for the lifetime of the program.
    /// C signature: const char *get_device_name(int fd);
    fn get_device_name(fd: i32) -> *const c_char;
}

// ──────────────────────────────────────────────────────────────────────────────
// Stub implementations (would be real C in a real project).
// We use #[no_mangle] here just to satisfy the linker for this self-contained
// example binary. In a real project these would be in a C file.
// ──────────────────────────────────────────────────────────────────────────────
#[no_mangle]
pub extern "C" fn open_device(path: *const c_char) -> i32 {
    if path.is_null() {
        return -1;
    }
    // Stub: pretend every path opens successfully with fd=42.
    // (In reality: call open(2) syscall here.)
    42
}

#[no_mangle]
pub extern "C" fn c_log_message(msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    // Safety: we trust C code passed a valid, null-terminated string.
    let s = unsafe { CStr::from_ptr(msg) };
    println!("[C-log] {}", s.to_string_lossy());
}

#[no_mangle]
pub extern "C" fn get_device_name(_fd: i32) -> *const c_char {
    // Return a static C string literal.
    // b"UART-A\0".as_ptr() is valid for 'static, so this is safe.
    b"UART-A\0".as_ptr() as *const c_char
}

fn main() {
    println!("=== Example 04: Strings Across the FFI Boundary ===\n");

    // ──────────────────────────────────────────────────────────────────────────
    // PATTERN 1: Passing a Rust string to C (Rust → C)
    //
    // CString::new() allocates a null-terminated copy of your string on the
    // heap. It fails if the string contains any interior null bytes.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Pattern 1: Rust string → C ---");

    let path = "/dev/ttyS0";

    // Step 1: Convert to CString. This allocates and appends a null byte.
    let c_path = CString::new(path).expect("path contains no null bytes");

    // Step 2: Get a raw pointer. The pointer borrows from c_path.
    // THIS IS THE CRITICAL POINT: c_path must stay alive while C uses the ptr!
    let ptr = c_path.as_ptr();

    // Step 3: Call C. Safe because c_path is still alive on this line.
    // Safety: ptr is non-null, points to a valid null-terminated string,
    // and the string lives for the duration of open_device's call.
    let fd = unsafe { open_device(ptr) };
    println!("  open_device(\"{}\") returned fd={}", path, fd);
    // c_path is dropped here — AFTER the C call. This is correct.

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // THE MOST COMMON FFI STRING BUG — temporary lifetime
    //
    // WRONG: CString::new(...) creates a temporary. .as_ptr() returns a pointer
    //        into that temporary. The temporary is dropped at the semicolon (;),
    //        leaving `ptr` pointing to freed memory — a use-after-free bug.
    //
    // The Rust compiler does NOT always catch this! It used to (pre-2021 edition
    // temporaries), but the behaviour is tricky and has changed across editions.
    // The safe approach is: ALWAYS bind the CString to a named `let` binding.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- WRONG: dangling pointer from temporary CString ---");
    println!();
    println!("  // WRONG — do NOT write this:");
    println!("  // let ptr = CString::new(\"/dev/ttyS0\").unwrap().as_ptr();");
    println!("  //                                               ^ CString dropped here!");
    println!("  // open_device(ptr);  // ptr is now a dangling pointer → UB");
    println!();
    println!("  // CORRECT — bind the CString to a name:");
    println!("  // let cstr = CString::new(\"/dev/ttyS0\").unwrap();");
    println!("  // open_device(cstr.as_ptr());  // cstr is alive here");
    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // PATTERN 2: Receiving a C string from C (C → Rust)
    //
    // CStr::from_ptr() borrows from the C string. It does NOT copy or allocate.
    // The resulting &CStr is only valid while C's string is alive.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Pattern 2: C string → Rust ---");

    // Safety: get_device_name returns a 'static string literal from C.
    // We know it's non-null and null-terminated (by reading the C implementation).
    let raw_name: *const c_char = unsafe { get_device_name(fd) };

    if raw_name.is_null() {
        println!("  get_device_name returned NULL — no name available");
    } else {
        // Safety: raw_name is non-null, null-terminated, valid for 'static.
        let name_cstr: &CStr = unsafe { CStr::from_ptr(raw_name) };

        // to_str() converts to &str if the bytes are valid UTF-8.
        // to_string_lossy() replaces invalid UTF-8 bytes with U+FFFD — always succeeds.
        match name_cstr.to_str() {
            Ok(s) => println!("  Device name (valid UTF-8): \"{}\"", s),
            Err(_) => {
                println!(
                    "  Device name (non-UTF-8, lossy): \"{}\"",
                    name_cstr.to_string_lossy()
                )
            }
        }

        // If you need an owned String (e.g., to store in a struct), clone it:
        let owned: String = name_cstr.to_string_lossy().into_owned();
        println!("  Owned Rust String: \"{}\"", owned);
    }

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // PATTERN 3: Interior null bytes — the hidden landmine
    //
    // C strings use null (0x00) as the terminator. If your string DATA contains
    // a null byte, C will think the string ended there, silently truncating it.
    // CString::new() CHECKS for interior nulls and returns an error — this saves
    // you from silent data corruption.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Pattern 3: Interior null bytes ---");

    let safe_string = "hello, world";
    let tricky_string = "hello\x00world"; // contains a null in the middle

    match CString::new(safe_string) {
        Ok(cs) => println!("  CString::new({:?}) → OK (len={})", safe_string, cs.as_bytes().len()),
        Err(e) => println!("  CString::new({:?}) → Err({})", safe_string, e),
    }

    match CString::new(tricky_string) {
        Ok(_) => println!("  CString::new({:?}) → OK (unexpected!)", tricky_string),
        Err(e) => println!(
            "  CString::new({:?}) → Err({}) ← interior null detected!",
            tricky_string, e
        ),
    }

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // PATTERN 4: Sending a log message (practical example)
    //
    // Many embedded C frameworks expose a logging function that takes a C string.
    // Here's the idiomatic way to call it.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Pattern 4: Calling C log function ---");

    let apid = 0x100u16;
    let seq = 42u16;

    // format! creates a Rust String. Then we convert it for C.
    let msg = format!("CCSDS packet received: apid=0x{:03X} seq={}", apid, seq);
    let c_msg = CString::new(msg).expect("log message contains no null bytes");

    // Safety: c_msg is alive for the duration of the call.
    unsafe { c_log_message(c_msg.as_ptr()); }

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // SUMMARY TABLE
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Summary ---");
    println!();
    println!("  Direction          | Rust type   | Key point");
    println!("  -------------------|-------------|------------------------------------");
    println!("  Rust → C (owned)   | CString     | Allocates + null-terminates");
    println!("  Rust → C (borrow)  | &CStr       | Zero-copy view; check lifetime!");
    println!("  C → Rust (borrow)  | &CStr       | from_ptr(); do NOT outlive C ptr");
    println!("  C → Rust (owned)   | String      | CStr::to_string_lossy().into_owned()");
    println!("  File paths         | OsStr/Path  | Platform-native encoding");
    println!();
    println!("  Golden rule: ALWAYS bind CString to a named variable.");
    println!("  Never call .as_ptr() on a temporary.");
}
