# Plantilla de Auditoría Unsafe

Copiar esta plantilla para cada bloque `unsafe` en la base de código.

---

## Las 5 Categorías de `unsafe` en Rust

| Categoría | Qué permite | Cuándo usar |
|-----------|-------------|-------------|
| Desreferencia de puntero crudo | Lectura/escritura `*ptr` | Acceso a registros de hardware, gestión de buffers |
| Llamada FFI | `extern "C" { fn foo(); }` | Integración de controladores en C |
| Acceso a `static mut` | Lectura/escritura de estado global mutable | Solo en contextos embebidos no-std |
| Implementar trait unsafe | `unsafe impl Send for Foo` | Cuando se garantiza la seguridad de hilos manualmente |
| Ensamblador en línea | `asm!("...")` | Inicialización de hardware de muy bajo nivel (VTOR, etc.) |

---

## Plantilla de Comentario SAFETY

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

## Ejemplos por Categoría

### Desreferencia de puntero crudo
```rust
// SAFETY: `base_ptr` is a valid, properly aligned pointer to a memory-mapped
// peripheral register block, as established by the linker script.
// The pointer arithmetic `base_ptr.add(REG_OFFSET)` stays within the
// peripheral's address range (verified against datasheet Table 3.1).
// Only this task accesses these registers (enforced by ownership).
// Would be unsound if: called from two tasks simultaneously (data race on peripheral).
let val = unsafe { base_ptr.add(REG_OFFSET).read_volatile() };
```

### Llamada FFI
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

### Trait unsafe
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

## Proceso de Auditoría para una PR

1. `grep -rn "unsafe" src/` — listar todos los bloques unsafe
2. Para cada uno: ¿tiene un comentario `// SAFETY:`?
3. ¿El invariante se mantiene realmente? (leer el código circundante)
4. ¿Existe una alternativa segura? (si la hay, usarla)
5. Agregar a la métrica `unsafe_count` en CI si se hace seguimiento de deuda técnica
