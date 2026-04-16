//! Ejemplo 05 — Manejo de errores a través del límite FFI
//!
//! Las funciones de C devuelven códigos de error (0/-1/errno). Rust quiere Result<T, E>.
//! El puente entre ambos requiere reflexión cuidadosa:
//!   1. Mapear códigos de error de C a tipos de error de Rust
//!   2. Los pánicos NO DEBEN cruzar el límite FFI (comportamiento indefinido)
//!   3. Usar std::panic::catch_unwind en callbacks extern "C"
//!
//! Ejecutar con:  cargo run --example 05_errors_across_ffi

use std::panic;

// ── Código de error de C → Result de Rust ────────────────────────────────────────────────

/// Códigos de error devueltos por nuestro "controlador de sensor" en C (ver ccsds_framer.c como contexto)
#[derive(Debug, thiserror::Error)]
pub enum FfiError {
    #[error("operación exitosa")]
    // No es realmente un error, pero muestra el patrón de mapeo de códigos
    Success,
    #[error("argumento inválido (errno EINVAL de C)")]
    InvalidArgument,
    #[error("dispositivo no listo")]
    NotReady,
    #[error("código de error C desconocido: {0}")]
    Unknown(i32),
}

impl FfiError {
    pub fn from_c_code(code: i32) -> Result<(), Self> {
        match code {
            0 => Ok(()),
            -1 => Err(Self::InvalidArgument),
            -2 => Err(Self::NotReady),
            other => Err(Self::Unknown(other)),
        }
    }
}

/// Envoltorio que mapea un código de retorno de C a Result<(), FfiError>
fn call_c_function_safely(code: i32) -> Result<(), FfiError> {
    FfiError::from_c_code(code)
}

// ── catch_unwind en el límite FFI ──────────────────────────────────────────────

/// Un callback de Rust que será llamado desde código C.
/// CUALQUIER pánico aquí sería UB — la pila de C no conoce los pánicos de Rust.
/// DEBEMOS capturarlo.
///
/// # Seguridad
/// Llamado desde C; no debe desenrollar la pila.
#[no_mangle]
pub extern "C" fn rust_callback_safe(value: i32) -> i32 {
    // catch_unwind convierte un pánico en un Result, evitando que cruce
    // el límite FFI (lo cual sería comportamiento indefinido).
    match panic::catch_unwind(|| {
        // Simular código que podría entrar en pánico
        if value < 0 {
            panic!("valor negativo no permitido: {value}");
        }
        value * 2
    }) {
        Ok(result) => result,
        Err(_) => {
            // El pánico fue capturado — devolver un centinela de error a C
            -1
        }
    }
}

// ── Seguridad con cadenas C y punteros nulos ───────────────────────────────────────────

use std::ffi::CStr;
use std::os::raw::c_char;

/// Convierte de forma segura un puntero de cadena C a un &str de Rust.
/// Devuelve None si el puntero es nulo o los bytes no son UTF-8 válido.
///
/// # Seguridad
/// `ptr` debe ser nulo o apuntar a una cadena de bytes terminada en nulo.
unsafe fn c_str_to_rust<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: el llamador garantiza que ptr es válido y está terminado en nulo
    let cstr = unsafe { CStr::from_ptr(ptr) };
    cstr.to_str().ok()
}

fn main() {
    println!("=== Manejo de errores a través de FFI ===\n");

    // 1. Mapear códigos de error de C a Results de Rust
    println!("Código de error de C → Result de Rust:");
    for code in [0, -1, -2, -99] {
        match call_c_function_safely(code) {
            Ok(()) => println!("  code={code:3} → Ok(())"),
            Err(e) => println!("  code={code:3} → Err({e})"),
        }
    }

    // 2. Demostrar la captura de pánicos en el límite FFI
    println!("\ncatch_unwind en el límite FFI:");
    for value in [5, -3, 10] {
        let result = rust_callback_safe(value);
        println!("  rust_callback_safe({value:3}) → {result}");
    }
    println!("  (los valores negativos causan un pánico internamente, pero es capturado — sin UB)");

    // 3. Seguridad con punteros nulos
    println!("\nSeguridad con cadenas C:");
    let valid = c"Hello from C";
    let result = unsafe { c_str_to_rust(valid.as_ptr()) };
    println!("  cadena C válida:    {:?}", result);
    let result = unsafe { c_str_to_rust(std::ptr::null()) };
    println!("  puntero nulo:       {:?}", result);

    println!("\nReglas clave:");
    println!("  1. C devuelve códigos de error i32 → mapear a Result<T, E> de Rust");
    println!("  2. Funciones extern \"C\" → NUNCA dejar que los pánicos se propaguen (UB)");
    println!("  3. Usar std::panic::catch_unwind en callbacks llamados desde C");
    println!("  4. Siempre verificar si hay nulo antes de desreferenciar punteros de C");
}
