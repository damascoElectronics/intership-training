# Unsafe Audit Template

Copy this template for every `unsafe` block in your codebase.

---

## The 5 Categories of `unsafe` in Rust

| Category | What it allows | When to use |
|----------|----------------|-------------|
| Raw pointer dereference | `*ptr` read/write | Hardware register access, buffer management |
| FFI call | `extern "C" { fn foo(); }` | C driver integration |
| `static mut` access | Read/write global mutable state | Embedded no-std contexts only |
| Implementing unsafe trait | `unsafe impl Send for Foo` | When you guarantee thread safety manually |
| Inline assembly | `asm!("...")` | Very low-level hardware init (VTOR, etc.) |

---

## SAFETY Comment Template

```rust
// SAFETY: <one-line summary>
// Invariants required:
//   1. <invariant>
//   2. <invariant>
// Upheld because: <justification>
// Would be unsound if: <what could break it>
unsafe { ... }
```

---

## Examples for Each Category

### Raw pointer dereference
```rust
// SAFETY: `base_ptr` is a valid, properly aligned pointer to a memory-mapped
// peripheral register block, as established by the linker script.
// The pointer arithmetic `base_ptr.add(REG_OFFSET)` stays within the
// peripheral's address range (verified against datasheet Table 3.1).
// Only this task accesses these registers (enforced by ownership).
// Would be unsound if: called from two tasks simultaneously (data race on peripheral).
let val = unsafe { base_ptr.add(REG_OFFSET).read_volatile() };
```

### FFI call
```rust
// SAFETY: `cstr.as_ptr()` is valid and null-terminated (guaranteed by CString).
// `ccsds_pack` reads the pointer value but does not store it or free it.
// The `CcsdsPrimaryHeader` pointed to by `&mut raw` is valid, aligned,
// and initialized to zeroes above.
// Would be unsound if: raw is dropped before ccsds_pack returns.
let rc = unsafe { ccsds_pack(&mut raw, apid, seq_count, data_len, is_tc) };
```

### static mut
```rust
// SAFETY: This function is called exactly once during system initialization,
// before any task spawning.  No concurrent access is possible.
// After init, INIT_TABLE is treated as read-only.
// Would be unsound if: called from multiple threads simultaneously.
unsafe { INIT_TABLE[idx] = value; }
```

### Unsafe trait
```rust
// SAFETY: SensorFrame is #[repr(C)] and contains only Copy types with no
// padding (verified by compile-time assert below).
// It contains no pointers, references, or Rust-managed allocations.
// It is valid for any bit pattern.
// Would be unsound if: a Rust reference field were added to SensorFrame.
unsafe impl FfiSafe for SensorFrame {}
const _: () = assert!(std::mem::size_of::<SensorFrame>() == 16);
```

---

## Audit Process for a PR

1. `grep -rn "unsafe" src/` — list all unsafe blocks
2. For each: does it have a `// SAFETY:` comment?
3. Is the invariant actually upheld? (read the surrounding code)
4. Is there a safe alternative? (if yes, use it)
5. Add to `unsafe_count` metric in CI if tracking technical debt
