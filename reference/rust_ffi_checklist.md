# Rust FFI Code Review Checklist

Use this before merging any code that crosses the Rust/C boundary.

---

## Structs crossing the boundary

- [ ] All structs have `#[repr(C)]`
- [ ] No Rust-specific types in C-facing structs (no `Vec`, `Box`, `String`, `&T`)
- [ ] Padding is explicit (no relying on Rust's padding rules)
- [ ] Unions have `#[repr(C)]` and access is gated by unsafe

## Functions

- [ ] All extern "C" functions are marked `unsafe` if they dereference pointers
- [ ] All `#[no_mangle] extern "C"` functions wrap their body in `std::panic::catch_unwind`
- [ ] No panics can propagate across the FFI boundary (UB)
- [ ] Return types are C-compatible (no `Result`, no `Option` — use sentinel values)

## Memory ownership

- [ ] Ownership of every pointer is documented: "caller owns", "callee owns", "borrowed"
- [ ] No Rust objects are dropped while C code holds a pointer to them
- [ ] No C memory is freed by Rust's allocator (use the correct deallocation function)
- [ ] RAII handles wrap C handles that need explicit cleanup

## Pointer safety

- [ ] Null pointers are checked before dereferencing
- [ ] Pointer alignment is verified (or guaranteed by construction)
- [ ] `CStr::from_ptr` is only called on valid null-terminated strings
- [ ] Buffer lengths are passed alongside pointers (no "trust the C code" for length)

## Build system

- [ ] `build.rs` uses `cc` crate (or similar) to compile C sources
- [ ] `println!("cargo:rerun-if-changed=...")` covers all C source/header files
- [ ] Either `bindgen` in `build.rs` or committed `bindings.rs` is up to date
- [ ] Link order is correct (Rust crate links against C library, not the other way)

## Documentation

- [ ] Every `unsafe` block has a `// SAFETY:` comment
- [ ] Safety comment lists: invariants required, how they are upheld, what would break them
- [ ] Public safe wrappers document what the C function does (don't say "calls adc_open")

---

## SAFETY comment template

```rust
// SAFETY: [brief description of why this unsafe operation is sound]
// Invariants required:
//   1. [invariant one]
//   2. [invariant two]
// Upheld because: [how you know the invariants hold at this call site]
// Would be unsound if: [what would break it]
```

Example:
```rust
// SAFETY: `ptr` is obtained from `CString::into_raw()` on line 42 and has not
// been freed.  The string contains no null bytes (guaranteed by CString::new).
// `adc_open` reads the string and does not retain the pointer.
// Would be unsound if: the CString were dropped before this call, or if
// adc_open stores the pointer for later use.
let fd = unsafe { ffi::adc_open(ptr) };
```
