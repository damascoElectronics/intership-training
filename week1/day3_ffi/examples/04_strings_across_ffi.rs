/// Ejemplo 04 — Manejo de cadenas a través del límite FFI
///
/// Las cadenas son donde viven muchos errores de FFI. La incompatibilidad fundamental:
///
///   Cadenas de Rust:
///     - `String`: asignada en el montón, codificada en UTF-8, longitud almacenada explícitamente,
///       NO terminada en nulo.
///     - `&str`: vista prestada de bytes UTF-8, NO terminada en nulo.
///
///   Cadenas de C:
///     - `char *`: un puntero a bytes, terminado en nulo (el valor de byte 0x00 marca el final).
///     - Puede o no ser UTF-8 (en la práctica suele ser ASCII).
///     - Puede contener bytes arbitrarios (UTF-8 no validado).
///
/// Los tipos clave en std::ffi de Rust:
///   - `CString`:  Cadena de propiedad de Rust, asignada en el montón, terminada en nulo.
///                 Creada desde datos de Rust, pasada a C.
///   - `CStr`:     Vista prestada de una secuencia de bytes terminada en nulo.
///                 Usada para leer cadenas que pertenecen a C.
///   - `OsString` / `OsStr`: Tipo de cadena nativo de la plataforma (UTF-8 en Unix,
///                 UTF-16 en Windows). Útil para rutas de archivo.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// ──────────────────────────────────────────────────────────────────────────────
// Declaramos algunas funciones C hipotéticas que trabajan con cadenas.
// Son sustitutos de las APIs reales de controladores de dispositivos que encontrarías en la práctica.
// ──────────────────────────────────────────────────────────────────────────────
extern "C" {
    /// Abre un dispositivo por ruta, devuelve un descriptor de archivo o -1.
    /// Firma en C: int open_device(const char *path);
    fn open_device(path: *const c_char) -> i32;

    /// Escribe un mensaje de registro a través del subsistema de logging de C.
    /// Firma en C: void c_log_message(const char *msg);
    fn c_log_message(msg: *const c_char);

    /// Devuelve el nombre del dispositivo como una cadena C asignada estáticamente.
    /// El puntero devuelto es válido durante toda la vida del programa.
    /// Firma en C: const char *get_device_name(int fd);
    fn get_device_name(fd: i32) -> *const c_char;
}

// ──────────────────────────────────────────────────────────────────────────────
// Implementaciones de prueba (serían C real en un proyecto real).
// Usamos #[no_mangle] aquí solo para satisfacer al enlazador en este ejemplo
// binario autocontenido. En un proyecto real estarían en un archivo C.
// ──────────────────────────────────────────────────────────────────────────────
#[no_mangle]
pub extern "C" fn open_device(path: *const c_char) -> i32 {
    if path.is_null() {
        return -1;
    }
    // Stub: fingir que toda ruta se abre exitosamente con fd=42.
    // (En realidad: llamar aquí a la syscall open(2).)
    42
}

#[no_mangle]
pub extern "C" fn c_log_message(msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    // Seguridad: confiamos en que el código C pasó una cadena válida terminada en nulo.
    let s = unsafe { CStr::from_ptr(msg) };
    println!("[C-log] {}", s.to_string_lossy());
}

#[no_mangle]
pub extern "C" fn get_device_name(_fd: i32) -> *const c_char {
    // Devolver un literal de cadena C estático.
    // b"UART-A\0".as_ptr() es válido para 'static, por lo que esto es seguro.
    b"UART-A\0".as_ptr() as *const c_char
}

fn main() {
    println!("=== Ejemplo 04: Cadenas a través del límite FFI ===\n");

    // ──────────────────────────────────────────────────────────────────────────
    // PATRÓN 1: Pasar una cadena de Rust a C (Rust → C)
    //
    // CString::new() asigna una copia terminada en nulo de tu cadena en el
    // montón. Falla si la cadena contiene bytes nulos interiores.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Patrón 1: cadena de Rust → C ---");

    let path = "/dev/ttyS0";

    // Paso 1: Convertir a CString. Esto asigna y agrega un byte nulo.
    let c_path = CString::new(path).expect("la ruta no contiene bytes nulos");

    // Paso 2: Obtener un puntero crudo. El puntero toma prestado de c_path.
    // ESTE ES EL PUNTO CRÍTICO: ¡c_path debe mantenerse vivo mientras C use el ptr!
    let ptr = c_path.as_ptr();

    // Paso 3: Llamar a C. Seguro porque c_path sigue vivo en esta línea.
    // Seguridad: ptr no es nulo, apunta a una cadena válida terminada en nulo,
    // y la cadena vive durante toda la llamada de open_device.
    let fd = unsafe { open_device(ptr) };
    println!("  open_device(\"{}\") devolvió fd={}", path, fd);
    // c_path se descarta aquí — DESPUÉS de la llamada a C. Esto es correcto.

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // EL ERROR DE CADENA FFI MÁS COMÚN — vida útil de temporales
    //
    // INCORRECTO: CString::new(...) crea un temporal. .as_ptr() devuelve un puntero
    //        a ese temporal. El temporal se descarta en el punto y coma (;),
    //        dejando `ptr` apuntando a memoria liberada — un error use-after-free.
    //
    // ¡El compilador de Rust NO siempre detecta esto! Antes lo hacía (ediciones pre-2021
    // con temporales), pero el comportamiento es complicado y ha cambiado entre ediciones.
    // El enfoque seguro es: SIEMPRE vincular el CString a un binding `let` con nombre.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- INCORRECTO: puntero colgante desde un CString temporal ---");
    println!();
    println!("  // INCORRECTO — NO escribas esto:");
    println!("  // let ptr = CString::new(\"/dev/ttyS0\").unwrap().as_ptr();");
    println!("  //                                               ^ ¡CString descartado aquí!");
    println!("  // open_device(ptr);  // ptr es ahora un puntero colgante → UB");
    println!();
    println!("  // CORRECTO — vincular el CString a un nombre:");
    println!("  // let cstr = CString::new(\"/dev/ttyS0\").unwrap();");
    println!("  // open_device(cstr.as_ptr());  // cstr está vivo aquí");
    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // PATRÓN 2: Recibir una cadena C de C (C → Rust)
    //
    // CStr::from_ptr() toma prestado de la cadena C. NO copia ni asigna.
    // El &CStr resultante solo es válido mientras la cadena de C esté viva.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Patrón 2: cadena C → Rust ---");

    // Seguridad: get_device_name devuelve un literal de cadena 'static de C.
    // Sabemos que no es nulo y está terminado en nulo (al leer la implementación en C).
    let raw_name: *const c_char = unsafe { get_device_name(fd) };

    if raw_name.is_null() {
        println!("  get_device_name devolvió NULL — no hay nombre disponible");
    } else {
        // Seguridad: raw_name no es nulo, está terminado en nulo, válido para 'static.
        let name_cstr: &CStr = unsafe { CStr::from_ptr(raw_name) };

        // to_str() convierte a &str si los bytes son UTF-8 válido.
        // to_string_lossy() reemplaza bytes UTF-8 inválidos con U+FFFD — siempre tiene éxito.
        match name_cstr.to_str() {
            Ok(s) => println!("  Nombre del dispositivo (UTF-8 válido): \"{}\"", s),
            Err(_) => {
                println!(
                    "  Nombre del dispositivo (no UTF-8, con pérdida): \"{}\"",
                    name_cstr.to_string_lossy()
                )
            }
        }

        // Si necesitas un String propio (ej., para almacenar en un struct), clónalo:
        let owned: String = name_cstr.to_string_lossy().into_owned();
        println!("  String de Rust propio: \"{}\"", owned);
    }

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // PATRÓN 3: Bytes nulos interiores — la mina escondida
    //
    // Las cadenas de C usan nulo (0x00) como terminador. Si tus DATOS de cadena contienen
    // un byte nulo, C pensará que la cadena terminó ahí, truncándola silenciosamente.
    // CString::new() VERIFICA los nulos interiores y devuelve un error — esto te salva
    // de la corrupción silenciosa de datos.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Patrón 3: Bytes nulos interiores ---");

    let safe_string = "hello, world";
    let tricky_string = "hello\x00world"; // contiene un nulo en el medio

    match CString::new(safe_string) {
        Ok(cs) => println!("  CString::new({:?}) → OK (len={})", safe_string, cs.as_bytes().len()),
        Err(e) => println!("  CString::new({:?}) → Err({})", safe_string, e),
    }

    match CString::new(tricky_string) {
        Ok(_) => println!("  CString::new({:?}) → OK (¡inesperado!)", tricky_string),
        Err(e) => println!(
            "  CString::new({:?}) → Err({}) ← ¡nulo interior detectado!",
            tricky_string, e
        ),
    }

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // PATRÓN 4: Enviar un mensaje de registro (ejemplo práctico)
    //
    // Muchos frameworks C embebidos exponen una función de logging que recibe una cadena C.
    // Esta es la forma idiomática de llamarla.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Patrón 4: Llamar a la función de log de C ---");

    let apid = 0x100u16;
    let seq = 42u16;

    // format! crea un String de Rust. Luego lo convertimos para C.
    let msg = format!("Paquete CCSDS recibido: apid=0x{:03X} seq={}", apid, seq);
    let c_msg = CString::new(msg).expect("el mensaje de log no contiene bytes nulos");

    // Seguridad: c_msg está vivo durante toda la llamada.
    unsafe { c_log_message(c_msg.as_ptr()); }

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // TABLA RESUMEN
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Resumen ---");
    println!();
    println!("  Dirección           | Tipo Rust   | Punto clave");
    println!("  --------------------|-------------|------------------------------------");
    println!("  Rust → C (propio)   | CString     | Asigna + termina en nulo");
    println!("  Rust → C (prestado) | &CStr       | Vista sin copia; ¡verificar vida útil!");
    println!("  C → Rust (prestado) | &CStr       | from_ptr(); NO sobrevivir al ptr de C");
    println!("  C → Rust (propio)   | String      | CStr::to_string_lossy().into_owned()");
    println!("  Rutas de archivo    | OsStr/Path  | Codificación nativa de la plataforma");
    println!();
    println!("  Regla de oro: SIEMPRE vincular CString a una variable con nombre.");
    println!("  Nunca llamar .as_ptr() sobre un temporal.");
}
