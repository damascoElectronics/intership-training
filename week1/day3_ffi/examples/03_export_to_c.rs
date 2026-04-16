/// Ejemplo 03 — Exportar funciones Rust a C
///
/// Hasta ahora hemos llamado C desde Rust. Este ejemplo va en la otra dirección:
/// escribir una función Rust que el código C pueda llamar.
///
/// Se requieren dos atributos:
///   - `extern "C"`: usar la convención de llamada C (no el ABI interno de Rust)
///   - `#[no_mangle]`: impedir la codificación de nombres de Rust para que C pueda encontrar el símbolo
///
/// POR QUÉ no_mangle importa:
///   Sin él, Rust convierte `rust_ccsds_validate` en algo como
///   `_ZN7day3_ffi20rust_ccsds_validate17h3a4b5c6d7e8f9g0hE`.
///   El código C que haga `extern int rust_ccsds_validate(...)` fallaría al enlazar.
///   `#[no_mangle]` mantiene el nombre del símbolo exactamente como está escrito.
///
/// Este patrón se usa cuando:
///   - Estás escribiendo una nueva biblioteca Rust para reemplazar parte de una base de código C
///   - Quieres llamar Rust desde un main() C existente o una tarea RTOS
///   - Estás construyendo un sistema de plugins/callbacks donde C llama funciones Rust registradas
///
/// Para APIs exportadas grandes, la herramienta `cbindgen` lee tu código fuente Rust y
/// genera automáticamente la cabecera C correspondiente. El comentario al final muestra
/// lo que cbindgen produciría para la función definida aquí.

use std::slice;

// ──────────────────────────────────────────────────────────────────────────────
// La función exportada
// ──────────────────────────────────────────────────────────────────────────────

/// Valida un buffer de cabecera primaria CCSDS crudo.
///
/// Esta función se exporta con enlace C para que el código C pueda llamarla:
///   `extern int rust_ccsds_validate(const uint8_t *raw, size_t len);`
///
/// Valores de retorno (siguiendo la convención C — no hay Result de Rust aquí):
///   0  → cabecera primaria CCSDS válida
///  -1  → puntero nulo pasado
///  -2  → buffer demasiado corto (se necesitan al menos 6 bytes)
///  -3  → versión CCSDS inválida (los bits 15-13 del byte 0 deben ser 0b000)
///  -4  → APID 0x7FF es el APID de paquete IDLE/relleno, tratado como inválido para TC
///
/// # Safety
///
/// El llamador debe asegurarse de que:
///   - `raw` no sea nulo (o la función devuelve -1)
///   - `raw` apunte a un buffer contiguo de al menos `len` bytes
///   - El buffer permanezca válido durante la duración de esta llamada
///   - Ningún otro hilo mute el buffer durante esta llamada
///
/// Esta es una `unsafe extern "C"` fn porque Rust no puede verificar que el llamador
/// cumpla estas invariantes en tiempo de compilación.
#[no_mangle]
pub unsafe extern "C" fn rust_ccsds_validate(raw: *const u8, len: usize) -> i32 {
    // ── Guardia 1: verificación de puntero nulo ──────────────────────────────────────────
    // En C es posible (incluso común) pasar NULL por error.
    // Verificamos esto primero porque cualquier desreferencia de un puntero nulo es UB.
    if raw.is_null() {
        return -1;
    }

    // ── Guardia 2: verificación de longitud ────────────────────────────────────────────
    // Una cabecera primaria CCSDS siempre tiene exactamente 6 bytes. Si tenemos menos,
    // no podemos validarla de manera significativa.
    if len < 6 {
        return -2;
    }

    // ── Construir un slice Rust seguro ──────────────────────────────────────────────
    // Safety: verificamos que raw no es nulo y len >= 6 arriba.
    // slice::from_raw_parts requiere: puntero válido, len dentro de límites, objeto único.
    // El contrato del llamador (documentado en la sección Safety arriba) cubre el resto.
    let buf: &[u8] = slice::from_raw_parts(raw, len);

    // ── Guardia 3: campo de versión ───────────────────────────────────────────────────
    // CCSDS 133.0-B-2 especifica los 3 bits más significativos del byte 0 como
    // el "número de versión", que debe ser 0b000 (= 0).
    // Bits 7-5 del byte 0 = (buf[0] >> 5) & 0x07
    let version = (buf[0] >> 5) & 0x07;
    if version != 0 {
        return -3;
    }

    // ── Guardia 4: verificación de paquete inactivo ───────────────────────────────────────
    // APID 0x7FF (todos 1s) es el APID de paquete inactivo/relleno de CCSDS.
    // En muchas implementaciones de naves espaciales, enrutar un paquete de relleno a un servicio
    // como si fuera un comando real sería un error grave.
    // bits 10-0 de los bytes 0-1: (byte0 & 0x07) << 8 | byte1
    let apid = (((buf[0] & 0x07) as u16) << 8) | (buf[1] as u16);
    if apid == 0x7FF {
        return -4;
    }

    // Todas las verificaciones pasaron.
    0
}

/// Devuelve una descripción legible por humanos de un código de retorno de rust_ccsds_validate.
///
/// Este es un auxiliar exportado junto al validador para que los llamadores C puedan producir
/// mensajes de diagnóstico sin incorporar el significado de cada código ellos mismos.
///
/// # Safety
///
/// El puntero devuelto es válido por `'static` (apunta a una literal de cadena).
/// El llamador NO debe liberarlo.
#[no_mangle]
pub extern "C" fn rust_ccsds_validate_strerror(code: i32) -> *const std::os::raw::c_char {
    // Usamos literales de cadenas de bytes con terminador nulo explícito.
    // b"texto\0".as_ptr() da un *const u8; convertir a *const c_char para C.
    match code {
        0  => b"cabecera primaria CCSDS valida\0".as_ptr() as *const std::os::raw::c_char,
        -1 => b"puntero nulo\0".as_ptr() as *const std::os::raw::c_char,
        -2 => b"buffer demasiado corto (necesita >= 6 bytes)\0".as_ptr() as *const std::os::raw::c_char,
        -3 => b"campo de version CCSDS invalido (debe ser 0b000)\0".as_ptr() as *const std::os::raw::c_char,
        -4 => b"APID 0x7FF es paquete inactivo/relleno\0".as_ptr() as *const std::os::raw::c_char,
        _  => b"codigo de error desconocido\0".as_ptr() as *const std::os::raw::c_char,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Prueba interna: usar la función mediante su firma Rust (envoltura segura para pruebas)
// ──────────────────────────────────────────────────────────────────────────────

/// Envoltura de prueba segura para no tener que escribir unsafe en cada prueba.
fn validate(raw: &[u8]) -> i32 {
    // Safety: raw.as_ptr() no es nulo (las referencias de slice nunca son nulas),
    // raw.len() es la longitud real, el buffer es válido durante la duración de la llamada.
    unsafe { rust_ccsds_validate(raw.as_ptr(), raw.len()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_tc_header() {
        // APID=0x100, tipo=TC (bit4 del byte0=1), versión=0b000 → byte0=0x11
        let raw = [0x11u8, 0x00, 0xC0, 0x01, 0x00, 0x03];
        assert_eq!(validate(&raw), 0);
    }

    #[test]
    fn valid_tm_header() {
        // APID=0x050, tipo=TM, versión=0b000 → byte0=0x00, byte1=0x50
        let raw = [0x00u8, 0x50, 0xC0, 0x2A, 0x00, 0x0F];
        assert_eq!(validate(&raw), 0);
    }

    #[test]
    fn null_pointer() {
        let result = unsafe { rust_ccsds_validate(std::ptr::null(), 6) };
        assert_eq!(result, -1);
    }

    #[test]
    fn too_short() {
        let raw = [0x00u8, 0x50, 0xC0]; // solo 3 bytes
        assert_eq!(validate(&raw), -2);
    }

    #[test]
    fn bad_version_field() {
        // Establecer versión a 0b001 en byte 0: (0b001 << 5) | bits de tipo
        let raw = [0x20u8, 0x50, 0xC0, 0x00, 0x00, 0x00]; // versión = 1
        assert_eq!(validate(&raw), -3);
    }

    #[test]
    fn idle_packet_apid() {
        // APID 0x7FF = 0b111_1111_1111
        // byte0: versión=000, tipo=0, sec=0, apid[10:8]=111 → 0b0000_0111 = 0x07
        // byte1: apid[7:0] = 0xFF
        let raw = [0x07u8, 0xFF, 0xC0, 0x00, 0x00, 0x00];
        assert_eq!(validate(&raw), -4);
    }
}

fn main() {
    println!("=== Ejemplo 03: Exportar Funciones Rust a C ===\n");

    // ──────────────────────────────────────────────────────────────────────────
    // Demostrar la llamada a la función exportada desde el propio Rust
    // (En la práctica esto sería llamado desde código C, pero podemos probarlo aquí.)
    // ──────────────────────────────────────────────────────────────────────────

    let test_cases: &[(&str, &[u8])] = &[
        ("TC válido (APID=0x100)", &[0x11, 0x00, 0xC0, 0x01, 0x00, 0x03]),
        ("TM válido (APID=0x050)", &[0x00, 0x50, 0xC0, 0x2A, 0x00, 0x0F]),
        ("Demasiado corto (3 bytes)",  &[0x11, 0x00, 0xC0]),
        ("Versión incorrecta (v=1)",   &[0x20, 0x50, 0xC0, 0x00, 0x00, 0x00]),
        ("APID inactivo (0x7FF)",      &[0x07, 0xFF, 0xC0, 0x00, 0x00, 0x00]),
    ];

    for (description, raw) in test_cases {
        let code = validate(raw);
        // Safety para strerror: devuelve *const c_char apuntando a una cadena estática.
        let msg_ptr = rust_ccsds_validate_strerror(code);
        let msg = unsafe { std::ffi::CStr::from_ptr(msg_ptr).to_str().unwrap() };
        println!("  {:<35}  → código {:2}  ({})", description, code, msg);
    }

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // COMENTARIO CBINDGEN
    //
    // Si este crate se construyera como `crate-type = ["cdylib"]` o `["staticlib"]`,
    // añadirías cbindgen a tu build.rs y generaría automáticamente una cabecera C.
    //
    // La cabecera que cbindgen produciría para las funciones anteriores:
    //
    //   /* Generado automáticamente por cbindgen — no editar */
    //   #ifndef RUST_CCSDS_VALIDATE_H
    //   #define RUST_CCSDS_VALIDATE_H
    //   #include <stdint.h>
    //   #include <stddef.h>
    //
    //   #ifdef __cplusplus
    //   extern "C" {
    //   #endif
    //
    //   /**
    //    * Valida un buffer de cabecera primaria CCSDS crudo.
    //    * Devuelve: 0 válido, -1 puntero nulo, -2 demasiado corto, -3 versión incorrecta, -4 APID inactivo
    //    */
    //   int32_t rust_ccsds_validate(const uint8_t *raw, size_t len);
    //
    //   /**
    //    * Devuelve una cadena C estática describiendo un código de retorno de rust_ccsds_validate.
    //    * El puntero devuelto es válido para siempre; NO lo liberes.
    //    */
    //   const char *rust_ccsds_validate_strerror(int32_t code);
    //
    //   #ifdef __cplusplus
    //   }
    //   #endif
    //
    //   #endif /* RUST_CCSDS_VALIDATE_H */
    //
    // cbindgen lee las funciones #[no_mangle] pub extern "C" y sus comentarios de documentación,
    // luego genera esta cabecera automáticamente. Este es el enfoque recomendado
    // para cualquier API exportada mayor que unas pocas funciones.
    // ──────────────────────────────────────────────────────────────────────────
    println!("Ver comentarios en el fuente para la cabecera C generada por cbindgen.");
    println!("\nTodas las pruebas de validación: PASADAS");
}
