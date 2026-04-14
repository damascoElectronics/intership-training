# Day 3 — C/Rust FFI: Calling Legacy C Drivers from Rust

## Why this matters for spacecraft software

Spacecraft on-board computers (OBCs) accumulate decades of C drivers. The UART driver for
your CCSDS interface was written in 2003. The SRAM controller library came from the ASIC
vendor as a compiled `.a` file with a header. You are not going to rewrite them — the
qualification cost alone would be prohibitive.

Rust's Foreign Function Interface (FFI) lets you call that C code directly, with zero
overhead, while building a safe Rust API on top. This is the pattern that lets you write new
spacecraft software in Rust without abandoning the existing C ecosystem.

---

## ABI vs API — why the distinction matters for FFI

**API** (Application Programming Interface) is what you see in source code: function
signatures, type names, constant values. It's a contract expressed in text.

**ABI** (Application Binary Interface) is the machine-level contract: how arguments are
passed (registers vs stack), how structs are laid out in memory (field order, padding, size),
what calling convention is used, how return values come back. It's a contract expressed in
bits.

When you call a C function from Rust, the compiler doesn't have the C source. It only has
the ABI contract. If Rust lays out a struct differently from C — say, adding padding bytes
where C expects none — you'll corrupt data at the boundary. No compiler error. Just wrong
bytes.

**That's why every type you share across the FFI boundary must use `#[repr(C)]`.**

Without `#[repr(C)]`, Rust is free to reorder fields for cache efficiency and add or remove
padding as it sees fit. With `#[repr(C)]`, Rust uses the same layout rules as C: fields in
declaration order, natural alignment, standard padding. The binary layout matches exactly.

---

## The C calling convention

On x86-64 Linux (the System V AMD64 ABI, also used on most embedded Linux targets):

- Integer/pointer arguments go in RDI, RSI, RDX, RCX, R8, R9; overflow goes on the stack
- Floating-point arguments go in XMM0–XMM7
- Return value goes in RAX (or XMM0 for float)
- The callee must preserve RBX, RBP, R12–R15 (callee-saved registers)

On ARM Cortex-A (AAPCS64):
- Integer/pointer arguments go in X0–X7
- Callee preserves X19–X28

For embedded bare-metal targets (Cortex-M, RISC-V without Linux), the calling convention
is simpler but still standardised. Rust respects `extern "C"` to mean "use the platform C
ABI", so this is automatic — you just need to declare it.

---

## `#[repr(C)]`: layout discipline at the boundary

```rust
// WITHOUT #[repr(C)]: Rust may reorder fields and add padding unpredictably.
struct Bad {
    flag: u8,     // Rust might put this at offset 4 to pack it with the u32
    value: u32,
}

// WITH #[repr(C)]: C-compatible layout.
// flag at offset 0, 3 bytes padding, value at offset 4 — same as a C struct.
#[repr(C)]
struct Good {
    flag: u8,
    value: u32,
}
```

Rules for `#[repr(C)]`:
- Fields appear in declaration order
- Each field is aligned to its own alignment requirement
- The struct size is padded to a multiple of its largest field's alignment
- This is exactly what `sizeof` and `offsetof` return in C

Also available:
- `#[repr(packed)]`: removes padding (dangerous; may cause unaligned access faults on some CPUs)
- `#[repr(u8)]` etc. on enums: controls the discriminant size

---

## Ownership at the FFI boundary

Rust's ownership system doesn't cross the FFI boundary. When you pass a pointer to C:

```
Rust side                   | C side
----------------------------|----------------------------------
Owns the allocation         | Receives a raw pointer
Must keep it alive!         | Has no idea about Rust's lifetime
```

**Key rules:**

1. **You keep ownership.** If you pass `data.as_ptr()` to a C function, Rust still owns
   `data`. You must ensure `data` lives at least as long as C is using the pointer.

2. **C-allocated memory.** If C allocates memory and returns a pointer, you cannot free it
   with Rust's allocator. You must call the C `free()` function (or the library's own free
   function) to release it.

3. **Temporary lifetime hazard.** This is the most common FFI bug in Rust:
   ```rust
   // WRONG: CString is dropped at the semicolon, pointer dangles
   let ptr = CString::new("hello").unwrap().as_ptr();
   c_function(ptr); // use-after-free!

   // CORRECT: bind the CString to a named variable
   let cstr = CString::new("hello").unwrap();
   c_function(cstr.as_ptr()); // cstr lives here
   ```

4. **Nullable pointers.** C functions often return NULL on error. You must check for NULL
   before dereferencing. Rust's `Option<NonNull<T>>` or `Option<extern "C" fn()>` model
   this explicitly.

---

## Two directions of FFI

### Direction 1: Calling C from Rust (`extern "C"` blocks)

```rust
// Declare the C function's signature — Rust trusts you to get this right.
extern "C" {
    fn ccsds_pack(
        hdr: *mut CcsdsPrimaryHeader,
        apid: u16,
        seq_count: u16,
        data_len: u16,
        is_tc: i32,
    ) -> i32;
}

// Call it in an unsafe block — you're asserting: "I've checked the invariants."
unsafe {
    let ret = ccsds_pack(&mut header, 0x100, 1, 4, 1);
    assert_eq!(ret, 0);
}
```

The `extern "C"` block is a declaration, not a definition. You're telling Rust "trust me,
this function exists in a linked library with this signature."

### Direction 2: Calling Rust from C (`#[no_mangle] extern "C"`)

```rust
/// # Safety
/// `output` must point to a buffer of at least 6 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn rust_validate_header(raw: *const u8, len: usize) -> i32 {
    if raw.is_null() { return -1; }
    if len < 6 { return -2; }
    // ... validation logic
    0
}
```

`#[no_mangle]` tells the Rust compiler not to apply name-mangling (which would turn
`rust_validate_header` into something like `_ZN7mypackage20rust_validate_header17h3a...`).
Without it, C can't find the function by its declared name.

`extern "C"` on a Rust function tells Rust to use the C calling convention for that
function (by default Rust uses its own unstable ABI).

---

## Build system integration: the `cc` crate and `build.rs`

Cargo's build system runs `build.rs` before compilation. The `cc` crate within `build.rs`
compiles C source files using the platform's C compiler with the right flags:

```rust
// build.rs
fn main() {
    cc::Build::new()
        .file("c_libs/ccsds_framer.c")
        .flag("-Wall")           // optional: extra warnings
        .compile("ccsds_framer"); // produces libccsds_framer.a
    // cc automatically emits "cargo:rustc-link-lib=static=ccsds_framer"
    // Cargo links it into your binary.
}
```

For pre-compiled libraries (vendor `.a` files):
```rust
println!("cargo:rustc-link-search=native=/path/to/vendor/lib");
println!("cargo:rustc-link-lib=static=vendor_driver");
```

For system dynamic libraries:
```rust
println!("cargo:rustc-link-lib=dylib=pthread");
```

---

## Safety contracts in `unsafe` blocks

`unsafe` in Rust doesn't mean "this code is dangerous." It means "I, the programmer, am
taking responsibility for upholding invariants the compiler can't verify automatically."

Every `unsafe` block should have a comment that explains exactly what contract you are
asserting. This is not just style — it's how code reviewers (and future you) know what to
check when something goes wrong.

```rust
// Safety: `hdr` was initialised by ccsds_pack above and has not been moved.
// The pointer is valid, non-null, and correctly aligned for CcsdsPrimaryHeader.
let raw = unsafe { &(*hdr_ptr).raw };
```

What invariants do YOU own in FFI unsafe blocks?

1. **Pointer validity**: the pointer is non-null, aligned, and points to a valid object
   of the declared type
2. **Lifetime**: the pointee outlives the duration of the C call
3. **Exclusive access**: if C mutates through the pointer, no other Rust code holds a
   reference to the same data
4. **Initialisation**: the data pointed to is fully initialised before passing to C
5. **Thread safety**: if C is not thread-safe, you must synchronise on the Rust side

---

## The mental model: the boundary is a contract, not a wall

Think of the `extern "C"` block as a promise: "I know this function exists with this
signature and this C ABI." The Rust compiler will believe you completely. If you lie —
wrong argument types, wrong number of args, wrong calling convention — you'll get silent
memory corruption or a crash at runtime. No type error at compile time.

This is why the safe wrapper pattern (Day 3, Example 02) is so important: isolate the
`unsafe` calls to a small, carefully reviewed layer, then expose a safe public API that
enforces all the preconditions at the type level.

```
[ C library: ccsds_framer.a ]
         ↑
[ unsafe FFI declarations ]     ← tiny; must be correct
         ↑
[ safe Rust wrapper ]           ← validates inputs, maps errors
         ↑
[ rest of your application ]    ← 100% safe Rust
```
