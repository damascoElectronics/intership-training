//! Ejemplo 04 — La auditoría de unsafe: 5 categorías con contratos SAFETY
//!
//! `unsafe` no significa "código peligroso" — significa "código donde el
//! programador toma responsabilidad por invariantes que el compilador no puede verificar."
//!
//! Para la revisión de código aeroespacial, cada bloque `unsafe` DEBE tener un comentario SAFETY
//! que explique qué invariante se cumple y por qué.
//!
//! Ejecutar con:  cargo run --example 04_unsafe_audit

#![allow(unused_unsafe)]

use std::sync::atomic::{AtomicU32, Ordering};

fn main() {
    println!("=== Las 5 categorías de unsafe ===\n");
    demo_raw_pointer();
    demo_ffi_call();
    demo_static_mut();
    demo_unsafe_trait();
    println!("Todos los ejemplos se ejecutaron sin problemas.");
    println!("\nLista de verificación de auditoría para cada bloque unsafe:");
    println!("  1. ¿Qué invariante debe cumplirse? (validez de memoria, aliasing, inicialización)");
    println!("  2. ¿Qué garantiza ese invariante? (sistema de tipos, contrato de API, revisión)");
    println!("  3. ¿Qué rompería el invariante? (documentarlo)");
    println!("  4. ¿Existe una alternativa segura? (preferirla si la hay)");
}

// ── Categoría 1: Desreferencia de puntero raw ──────────────────────────────────

fn demo_raw_pointer() {
    let mut value: u32 = 42;
    let ptr: *mut u32 = &mut value;

    // SAFETY: `ptr` fue obtenido de &mut value en la línea anterior.
    // Apunta a un u32 válido y correctamente alineado que está vivo durante
    // toda esta función. No existe ninguna otra referencia a `value`
    // mientras `ptr` está en uso (el borrow checker normalmente lo impediría,
    // pero aquí se usan punteros raw deliberadamente para mostrar el patrón).
    unsafe { *ptr = 100; }
    println!("Categoría 1 (ptr raw): value = {value}");
}

// ── Categoría 2: Llamada FFI ──────────────────────────────────────────────────

extern "C" {
    // Declarando una función C que no existe realmente — enlazamos contra libc
    // que tiene strlen. En un proyecto real esto sería el driver de hardware.
    fn strlen(s: *const std::os::raw::c_char) -> usize;
}

fn demo_ffi_call() {
    let s = c"Hello, spacecraft";
    let len = unsafe {
        // SAFETY: `s.as_ptr()` apunta a una cadena C válida con terminación nula literal.
        // El literal `c""` garantiza terminación nula y vida útil estática.
        // strlen solo lee memoria; no escribe ni libera.
        strlen(s.as_ptr())
    };
    println!("Categoría 2 (FFI):     strlen = {len}");
}

// ── Categoría 3: static mut ───────────────────────────────────────────────────
// static mut es la categoría más peligrosa — las condiciones de carrera son UB.
// Preferir AtomicXxx o Mutex para estado compartido.

static COUNTER_UNSAFE: AtomicU32 = AtomicU32::new(0);

// Si absolutamente se debe usar static mut (p. ej., no-std sin allocator):
static mut INIT_BUFFER: [u8; 64] = [0u8; 64];

fn demo_static_mut() {
    // Alternativa segura: usar atómicos (no se necesita unsafe)
    COUNTER_UNSAFE.fetch_add(1, Ordering::SeqCst);
    println!("Categoría 3 (static):  contador = {}", COUNTER_UNSAFE.load(Ordering::SeqCst));

    // Cuando static mut es inevitable (inicialización monohilo):
    // SAFETY: esta función solo se llama una vez, durante la inicialización,
    // antes de que arranque cualquier otro hilo. Ningún otro código accede a INIT_BUFFER
    // concurrentemente. Tras la inicialización, INIT_BUFFER es de solo lectura.
    unsafe {
        INIT_BUFFER[0] = 0xA5; // valor clásico embebido de "pila pintada"
    }
    let first = unsafe { INIT_BUFFER[0] };
    println!("Categoría 3 (static):  INIT_BUFFER[0] = 0x{first:02X}");
}

// ── Categoría 4: Implementar un trait unsafe ──────────────────────────────────

/// Un trait que promete que el tipo puede enviarse de forma segura a través de una frontera FFI.
///
/// # Safety
/// El implementador garantiza que el tipo tiene layout `#[repr(C)]` y no contiene
/// tipos específicos de Rust (referencias, Box, Vec, etc.) que serían inválidos
/// en C.
unsafe trait FfiSafe {}

#[repr(C)]
struct SensorReading {
    timestamp_ms: u64,
    temperature_mc: i32,
    pressure_pa: u32,
}

// SAFETY: SensorReading es #[repr(C)] y contiene solo enteros primitivos.
// Puede pasarse de forma segura a través de la frontera FFI.
unsafe impl FfiSafe for SensorReading {}

fn demo_unsafe_trait() {
    let r = SensorReading { timestamp_ms: 1000, temperature_mc: 25_000, pressure_pa: 101325 };
    println!("Categoría 4 (trait):   SensorReading {{ ts={}, temp={}mc, p={}Pa }}",
             r.timestamp_ms, r.temperature_mc, r.pressure_pa);
}
