//! Ejemplo 04 — Qué detecta Miri (y qué no puede)
//!
//! Miri es un intérprete de Rust que detecta Comportamiento Indefinido en tiempo de ejecución.
//! Detecta cosas que el compilador no puede demostrar seguras en tiempo de compilación:
//!   - Lectura de memoria no inicializada
//!   - Uso tras liberación (use-after-free)
//!   - Aritmética de punteros fuera de límites
//!   - Procedencia de puntero inválida
//!
//! Instalar y ejecutar:
//!   rustup component add miri
//!   cargo +nightly miri run --example 04_miri_safety
//!
//! Miri encontrará el Comportamiento Indefinido en la sección UNSAFE y lo reportará claramente.
//! La sección SAFE demuestra los patrones correctos.

fn main() {
    println!("=== Patrones de Seguridad con Miri ===\n");

    safe_patterns();
    println!();
    println!("Para verificar Comportamiento Indefinido en código unsafe, ejecutar bajo Miri:");
    println!("  cargo +nightly miri run --example 04_miri_safety");
    println!("  cargo +nightly miri test");
}

fn safe_patterns() {
    // ── Patrón 1: verificación de límites ────────────────────────────────────────
    let data = vec![1u8, 2, 3, 4, 5, 6];

    // Seguro: Rust verifica los límites en tiempo de ejecución
    let slice = &data[1..4];
    println!("Slice seguro: {slice:?}");

    // También seguro: get() devuelve Option
    let item = data.get(10);
    println!("get() fuera de límites: {item:?} (None, no Comportamiento Indefinido)");

    // ── Patrón 2: memoria inicializada ─────────────────────────────────────────
    // Seguro: MaybeUninit para inicialización manual
    use std::mem::MaybeUninit;
    let mut buffer: [MaybeUninit<u8>; 4] = MaybeUninit::uninit_array();
    for (i, b) in buffer.iter_mut().enumerate() {
        b.write(i as u8 * 10);
    }
    // SAFETY: todos los elementos fueron inicializados arriba
    let initialized: [u8; 4] = unsafe { MaybeUninit::array_assume_init(buffer) };
    println!("Correctamente inicializado: {initialized:?}");

    // ── Patrón 3: procedencia de puntero ─────────────────────────────────────────
    // Seguro: derivar puntero de una referencia válida
    let mut value: u32 = 42;
    let ptr: *mut u32 = &mut value;
    // SAFETY: ptr fue derivado de &mut value que es válido y alineado
    unsafe { *ptr = 100; }
    println!("Escritura mediante puntero con procedencia válida: {value}");

    // ── Patrón 4: paso de buffer mediante FFI ─────────────────────────────────────
    // Patrón seguro: pasar puntero + longitud juntos, sin calcular desplazamientos manualmente
    let mut out_buf = vec![0u8; 16];
    fill_buffer_safe(out_buf.as_mut_ptr(), out_buf.len());
    println!("Buffer rellenado mediante patrón FFI seguro: {out_buf:?}");
}

/// Relleno seguro de buffer al estilo FFI: recibe ptr + len, permanece dentro de los límites.
///
/// # Safety
/// `ptr` debe ser válido para `len` escrituras, alineado a u8 (trivialmente cierto).
unsafe fn fill_buffer_safe(ptr: *mut u8, len: usize) {
    for i in 0..len {
        // SAFETY: i < len, por lo que ptr.add(i) está dentro del buffer asignado
        unsafe { ptr.add(i).write(i as u8); }
    }
}

// ── Lo que Miri detectaría (comentado para evitar ejecutar Comportamiento Indefinido) ─────────

/*
fn ub_examples() {
    // ❌ CI 1: lectura de memoria no inicializada
    let x: u32 = unsafe { std::mem::uninitialized() };
    println!("{x}"); // Miri: "using uninitialized data"

    // ❌ CI 2: acceso fuera de límites
    let v = vec![1u8, 2, 3];
    let ptr = v.as_ptr();
    let _bad = unsafe { *ptr.add(10) }; // Miri: "out-of-bounds pointer"

    // ❌ CI 3: uso tras liberación
    let b = Box::new(42u32);
    let ptr = Box::into_raw(b);
    drop(unsafe { Box::from_raw(ptr) }); // liberar
    let _bad = unsafe { *ptr };          // Miri: "use-after-free"
}
*/
