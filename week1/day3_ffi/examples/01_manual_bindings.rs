/// Ejemplo 01 — Bindings FFI escritos a mano
///
/// Esta es la capa cruda: declaramos tipos C y firmas de funciones manualmente.
/// Esto es exactamente lo que `bindgen` genera automáticamente, pero escribirlo a mano
/// una vez te enseña lo que bindgen hace y por qué cada pieza es necesaria.
///
/// Conceptos clave demostrados:
///   - `#[repr(C)]` en una estructura: hace que Rust use la distribución de memoria compatible con C
///   - Bloque `extern "C"`: declara funciones que existen en bibliotecas C enlazadas
///   - Bloque `unsafe`: donde TÚ afirmas las invariantes de seguridad que el compilador no puede verificar
///   - La convención de llamada: Rust genera una llamada que coincide con lo que C espera

// ──────────────────────────────────────────────────────────────────────────────
// Paso 1: Re-declarar la estructura C en Rust
//
// La cabecera C dice:
//   struct CcsdsPrimaryHeader { uint8_t raw[6]; };
//
// Debemos añadir #[repr(C)] para que Rust distribuya la estructura exactamente como lo haría C.
// Sin él, Rust podría teóricamente reordenar campos o añadir relleno, haciendo la
// estructura incompatible — aunque esta solo tiene un único campo de array.
// Es una buena costumbre usar siempre #[repr(C)] para cualquier tipo que cruce el límite.
// ──────────────────────────────────────────────────────────────────────────────
#[repr(C)]
pub struct CcsdsPrimaryHeader {
    raw: [u8; 6],
}

// ──────────────────────────────────────────────────────────────────────────────
// Paso 2: Declarar las funciones C en un bloque extern "C"
//
// "extern "C"" le dice a Rust: usar el ABI C (System V AMD64 en x86-64 Linux,
// AAPCS en ARM). El enlazador resolverá estos a símbolos en libccsds_framer.a,
// que Cargo compiló via build.rs usando el crate `cc`.
//
// Las firmas DEBEN coincidir exactamente con la cabecera C:
//   - C `int`    ↔ Rust `i32`    (ambos son enteros de 32 bits con signo en todas las plataformas que nos importan)
//   - C `uint16_t` ↔ Rust `u16`
//   - C puntero `T*` ↔ Rust `*mut T`
//   - C puntero const `const T*` ↔ Rust `*const T`
//
// Si obtienes una firma incorrecta, NO tendrás un error de compilación. Obtendrás
// valores incorrectos, corrupción de pila, o un fallo en tiempo de ejecución. Trata estas
// declaraciones con el mismo cuidado que darías a un mapa de registros de seguridad crítica.
// ──────────────────────────────────────────────────────────────────────────────
extern "C" {
    /// Empaqueta los campos de la cabecera primaria CCSDS en 6 bytes crudos.
    ///
    /// Firma C:
    ///   int ccsds_pack(struct CcsdsPrimaryHeader *hdr,
    ///                  uint16_t apid, uint16_t seq_count,
    ///                  uint16_t data_len, int is_tc);
    ///
    /// Devuelve 0 en caso de éxito, -1 si algún argumento está fuera de rango o hdr es NULL.
    fn ccsds_pack(
        hdr: *mut CcsdsPrimaryHeader,
        apid: u16,
        seq_count: u16,
        data_len: u16,
        is_tc: i32,
    ) -> i32;

    /// Extrae los campos de la cabecera primaria CCSDS de 6 bytes crudos.
    ///
    /// Firma C:
    ///   int ccsds_unpack(const struct CcsdsPrimaryHeader *hdr,
    ///                    uint16_t *apid, uint16_t *seq_count, uint16_t *data_len);
    ///
    /// Los argumentos de puntero de salida pueden ser NULL si ese campo no se necesita.
    /// Devuelve 0 en caso de éxito, -1 si hdr es NULL.
    fn ccsds_unpack(
        hdr: *const CcsdsPrimaryHeader,
        apid: *mut u16,
        seq_count: *mut u16,
        data_len: *mut u16,
    ) -> i32;

    /// Devuelve distinto de cero si el bit de tipo en la cabecera está activado (Telecomando).
    fn ccsds_is_tc(hdr: *const CcsdsPrimaryHeader) -> i32;
}

fn main() {
    println!("=== Ejemplo 01: Bindings FFI Manuales ===\n");

    // ──────────────────────────────────────────────────────────────────────────
    // Construir un paquete TC con:
    //   APID       = 0x100  (256 decimal — un APID hipotético de control de actitud)
    //   seq_count  = 1
    //   data_len   = 4      (4 bytes de payload; almacenado como 3 según la especificación CCSDS)
    //   is_tc      = 1      (Telecomando)
    // ──────────────────────────────────────────────────────────────────────────

    // Comenzar con memoria puesta a cero. Podríamos usar MaybeUninit aquí, pero poner a cero es
    // seguro y deja claro el estado "antes" con fines educativos.
    let mut header = CcsdsPrimaryHeader { raw: [0u8; 6] };

    // ──────────────────────────────────────────────────────────────────────────
    // Bloque unsafe: estamos llamando código extranjero (C).
    //
    // Invariantes que NOSOTROS afirmamos aquí (el compilador no puede verificarlas):
    //   1. `header` está correctamente alineado para CcsdsPrimaryHeader (lo está — es una
    //      variable local en la pila con #[repr(C)]).
    //   2. `&mut header` no es nulo (garantizado — es una referencia, no un puntero crudo).
    //   3. `header` está prestado exclusivamente durante la duración de esta llamada
    //      (garantizado por el verificador de préstamos de Rust en `&mut header`).
    //   4. La función C no almacenará el puntero más allá de la llamada (no lo hará —
    //      ccsds_pack solo escribe en la estructura de forma síncrona).
    // ──────────────────────────────────────────────────────────────────────────
    let pack_result = unsafe {
        ccsds_pack(
            &mut header as *mut CcsdsPrimaryHeader,
            0x100, // apid
            1,     // seq_count
            4,     // data_len (almacenado como 3 en la cabecera, según CCSDS: data_len - 1)
            1,     // is_tc = verdadero
        )
    };

    // Las funciones C devuelven códigos de error, no Results de Rust. Siempre verifícalos.
    assert_eq!(pack_result, 0, "ccsds_pack falló con error {}", pack_result);

    println!("Cabecera TC empaquetada (APID=0x100, seq=1, data_len=4):");
    println!("  Bytes crudos: {:02X?}", header.raw);
    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // Verifiquemos los bytes manualmente usando nuestro conocimiento de la distribución de bits CCSDS:
    //
    //   Byte 0: [V V V T S A A A]  → versión=000, tipo=1(TC), sec=0, apid[10:8]=001
    //           = 0b0001_0001 = 0x11
    //   Byte 1: [A A A A A A A A]  → apid[7:0] = 0x00
    //           = 0x00
    //   Byte 2: [F F C C C C C C]  → banderas_seq=11, conteo_seq[13:8]=000000
    //           = 0b1100_0000 = 0xC0
    //   Byte 3: [C C C C C C C C]  → conteo_seq[7:0] = 0x01
    //           = 0x01
    //   Byte 4: [L L L L L L L L]  → (data_len-1)[15:8] = 0x00
    //           = 0x00
    //   Byte 5: [L L L L L L L L]  → (data_len-1)[7:0] = 0x03
    //           = 0x03
    // ──────────────────────────────────────────────────────────────────────────
    println!("Esperado: [11, 00, C0, 01, 00, 03]");
    assert_eq!(header.raw[0], 0x11, "Byte 0 incorrecto: versión/tipo/apid alto");
    assert_eq!(header.raw[1], 0x00, "Byte 1 incorrecto: apid bajo");
    assert_eq!(header.raw[2], 0xC0, "Byte 2 incorrecto: banderas_seq/conteo_seq alto");
    assert_eq!(header.raw[3], 0x01, "Byte 3 incorrecto: conteo_seq bajo");
    assert_eq!(header.raw[4], 0x00, "Byte 4 incorrecto: data_len-1 alto");
    assert_eq!(header.raw[5], 0x03, "Byte 5 incorrecto: data_len-1 bajo");
    println!("Verificación a nivel de byte: PASADA\n");

    // ──────────────────────────────────────────────────────────────────────────
    // Ahora desempaquetar la cabecera y verificar que obtenemos los mismos valores.
    //
    // Usamos variables locales y pasamos sus direcciones como parámetros de salida,
    // siguiendo el idioma de C de "parámetro de salida" (C no tiene tuplas ni tipo Result).
    // ──────────────────────────────────────────────────────────────────────────
    let mut apid: u16 = 0;
    let mut seq_count: u16 = 0;
    let mut data_len: u16 = 0;

    // Safety: `header` es válido, alineado, y vive más allá de esta llamada.
    // apid, seq_count, data_len son variables locales en la pila.
    let unpack_result = unsafe {
        ccsds_unpack(
            &header as *const CcsdsPrimaryHeader,
            &mut apid as *mut u16,
            &mut seq_count as *mut u16,
            &mut data_len as *mut u16,
        )
    };

    assert_eq!(unpack_result, 0, "ccsds_unpack falló");

    println!("Campos desempaquetados:");
    println!("  APID:      0x{:03X} (esperado 0x100)", apid);
    println!("  seq_count: {}      (esperado 1)", seq_count);
    println!("  data_len:  {}      (esperado 4)", data_len);

    assert_eq!(apid, 0x100);
    assert_eq!(seq_count, 1);
    assert_eq!(data_len, 4);

    // Safety: header es válido y no es nulo.
    let is_tc = unsafe { ccsds_is_tc(&header as *const CcsdsPrimaryHeader) };
    println!("  is_tc:     {}      (esperado 1)", is_tc);
    assert_ne!(is_tc, 0);

    println!("\nVerificación de ida y vuelta: PASADA");
    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // También probar un paquete TM (telemetría) para asegurarse de que el bit de tipo está despejado.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Prueba de paquete TM ---");
    let mut tm_header = CcsdsPrimaryHeader { raw: [0u8; 6] };

    // Safety: el mismo razonamiento que arriba.
    let ret = unsafe { ccsds_pack(&mut tm_header, 0x050, 42, 16, 0 /* is_tc=falso */) };
    assert_eq!(ret, 0);

    println!("Cabecera TM (APID=0x050, seq=42, data_len=16):");
    println!("  Bytes crudos: {:02X?}", tm_header.raw);

    // Safety: tm_header es válido.
    let tm_is_tc = unsafe { ccsds_is_tc(&tm_header as *const CcsdsPrimaryHeader) };
    println!("  is_tc: {} (esperado 0 para TM)", tm_is_tc);
    assert_eq!(tm_is_tc, 0);

    let mut out_apid: u16 = 0;
    let mut out_seq: u16 = 0;
    let mut out_len: u16 = 0;
    // Safety: tm_header es válido.
    unsafe {
        ccsds_unpack(&tm_header, &mut out_apid, &mut out_seq, &mut out_len);
    }
    assert_eq!(out_apid, 0x050);
    assert_eq!(out_seq, 42);
    assert_eq!(out_len, 16);
    println!("Ida y vuelta TM: PASADA\n");

    // ──────────────────────────────────────────────────────────────────────────
    // Probar manejo de errores: ccsds_pack debe rechazar argumentos inválidos.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Prueba de manejo de errores ---");
    let mut bad_header = CcsdsPrimaryHeader { raw: [0u8; 6] };

    // APID 0x800 = 2048 = uno más que el rango válido de 11 bits (0-2047).
    // Safety: bad_header es una asignación válida; estamos probando la ruta de error de C.
    let bad_ret = unsafe { ccsds_pack(&mut bad_header, 0x800, 0, 1, 0) };
    println!("  ccsds_pack(apid=0x800): devolvió {} (esperado -1)", bad_ret);
    assert_eq!(bad_ret, -1);

    // data_len=0 es inválido según CCSDS (almacenado como data_len-1 haría un desbordamiento).
    let bad_ret2 = unsafe { ccsds_pack(&mut bad_header, 0x001, 0, 0, 0) };
    println!("  ccsds_pack(data_len=0): devolvió {} (esperado -1)", bad_ret2);
    assert_eq!(bad_ret2, -1);

    println!("\nPrueba de manejo de errores: PASADA");
}
