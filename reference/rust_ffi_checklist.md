# Lista de Verificación para Revisión de Código Rust FFI

Usar antes de fusionar cualquier código que cruce la frontera Rust/C.

---

## Structs que cruzan la frontera

- [ ] Todos los structs tienen `#[repr(C)]`
- [ ] No hay tipos específicos de Rust en structs expuestos a C (sin `Vec`, `Box`, `String`, `&T`)
- [ ] El relleno (padding) es explícito (no depender de las reglas de padding de Rust)
- [ ] Los unions tienen `#[repr(C)]` y el acceso está protegido por unsafe

## Funciones

- [ ] Todas las funciones `extern "C"` están marcadas como `unsafe` si desreferencian punteros
- [ ] Todas las funciones `#[no_mangle] extern "C"` envuelven su cuerpo en `std::panic::catch_unwind`
- [ ] Ningún panic puede propagarse a través de la frontera FFI (comportamiento indefinido)
- [ ] Los tipos de retorno son compatibles con C (sin `Result`, sin `Option` — usar valores centinela)

## Propiedad de memoria

- [ ] La propiedad de cada puntero está documentada: "el llamador es propietario", "el llamado es propietario", "prestado"
- [ ] Ningún objeto de Rust es liberado mientras el código C mantiene un puntero a él
- [ ] Ninguna memoria de C es liberada por el asignador de Rust (usar la función de desasignación correcta)
- [ ] Los manejadores RAII envuelven los recursos de C que necesitan limpieza explícita

## Seguridad de punteros

- [ ] Los punteros nulos se comprueban antes de desreferenciarlos
- [ ] La alineación de los punteros está verificada (o garantizada por construcción)
- [ ] `CStr::from_ptr` solo se llama sobre cadenas válidas terminadas en nulo
- [ ] Las longitudes de los buffers se pasan junto con los punteros (no "confiar en el código C" para la longitud)

## Sistema de compilación

- [ ] `build.rs` usa el crate `cc` (o similar) para compilar fuentes C
- [ ] `println!("cargo:rerun-if-changed=...")` cubre todos los archivos fuente/cabecera de C
- [ ] `bindgen` en `build.rs` o el archivo `bindings.rs` confirmado están actualizados
- [ ] El orden de enlazado es correcto (el crate Rust enlaza contra la biblioteca C, no al revés)

## Documentación

- [ ] Cada bloque `unsafe` tiene un comentario `// SAFETY:`
- [ ] El comentario de seguridad enumera: invariantes requeridos, cómo se mantienen, qué los rompería
- [ ] Los wrappers seguros públicos documentan lo que hace la función C (no decir "llama a adc_open")

---

## Plantilla de comentario SAFETY

```rust
// SAFETY: [brief description of why this unsafe operation is sound]
// Invariants required:
//   1. [invariant one]
//   2. [invariant two]
// Upheld because: [how you know the invariants hold at this call site]
// Would be unsound if: [what would break it]
```

Ejemplo:
```rust
// SAFETY: `ptr` is obtained from `CString::into_raw()` on line 42 and has not
// been freed.  The string contains no null bytes (guaranteed by CString::new).
// `adc_open` reads the string and does not retain the pointer.
// Would be unsound if: the CString were dropped before this call, or if
// adc_open stores the pointer for later use.
let fd = unsafe { ffi::adc_open(ptr) };
```
