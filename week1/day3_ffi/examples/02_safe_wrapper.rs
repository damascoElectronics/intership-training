/// Ejemplo 02 — Construir una API Rust segura alrededor de funciones C
///
/// El ejemplo anterior mostró la capa FFI cruda. Este ejemplo muestra el
/// siguiente paso crítico: envolver las llamadas FFI inseguras en una API pública segura.
///
/// El patrón:
///   1. Mantener las declaraciones `extern "C"` privadas (privadas al módulo o al crate)
///   2. Construir una struct/impl Rust que valide todas las entradas ANTES de llamar a C
///   3. Mapear códigos de error C a un tipo `Result` de Rust
///   4. Después de esta envoltura, todos los llamadores escriben 100% Rust seguro
///
/// Por qué importa: el resto de tu base de código no necesita saber
/// sobre C, unsafe o la manipulación de bits CCSDS. Solo este módulo lo sabe.

// Declaramos los bindings de bajo nivel de forma privada — los llamadores usan CcsdsHeader, no estos.
#[repr(C)]
struct CcsdsPrimaryHeaderRaw {
    raw: [u8; 6],
}

extern "C" {
    fn ccsds_pack(
        hdr: *mut CcsdsPrimaryHeaderRaw,
        apid: u16,
        seq_count: u16,
        data_len: u16,
        is_tc: i32,
    ) -> i32;

    fn ccsds_unpack(
        hdr: *const CcsdsPrimaryHeaderRaw,
        apid: *mut u16,
        seq_count: *mut u16,
        data_len: *mut u16,
    ) -> i32;

    fn ccsds_is_tc(hdr: *const CcsdsPrimaryHeaderRaw) -> i32;
}

// ──────────────────────────────────────────────────────────────────────────────
// Tipo de error: representar cada forma en que la API puede fallar como una enumeración Rust.
//
// Usar una enumeración (en lugar de mensajes de cadena o códigos i32) significa que los llamadores pueden
// hacer pattern-matching en el error exacto y manejarlo programáticamente.
// ──────────────────────────────────────────────────────────────────────────────
#[derive(Debug, PartialEq)]
pub enum CcsdsError {
    /// APID debe ser 0–2047 (11 bits).
    ApidOutOfRange { provided: u16 },
    /// El conteo de secuencia debe ser 0–16383 (14 bits).
    SeqCountOutOfRange { provided: u16 },
    /// data_len debe ser ≥ 1 (CCSDS almacena data_len-1; cero es inválido).
    DataLenZero,
    /// La función C subyacente devolvió un código de error inesperado.
    CFunctionFailed { code: i32 },
}

impl std::fmt::Display for CcsdsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CcsdsError::ApidOutOfRange { provided } => {
                write!(f, "APID 0x{:03X} está fuera de rango (máx 0x7FF = 2047)", provided)
            }
            CcsdsError::SeqCountOutOfRange { provided } => {
                write!(
                    f,
                    "seq_count {} está fuera de rango (máx 16383 = 0x3FFF)",
                    provided
                )
            }
            CcsdsError::DataLenZero => write!(f, "data_len debe ser ≥ 1"),
            CcsdsError::CFunctionFailed { code } => {
                write!(f, "La función C devolvió el código de error {}", code)
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// La API pública segura
// ──────────────────────────────────────────────────────────────────────────────

/// Una cabecera primaria CCSDS validada.
///
/// Invariantes (mantenidas por los constructores):
///   - apid ≤ 2047
///   - seq_count ≤ 16383
///   - data_len ≥ 1
///   - `inner.raw` contiene la codificación correcta de bytes
///
/// Una vez construida, todas estas invariantes se mantienen y los llamadores no necesitan unsafe.
#[derive(Debug, Clone, PartialEq)]
pub struct CcsdsHeader {
    // Almacenamos los bytes crudos (tal como los produce C) en lugar de los campos decodificados.
    // Esto significa que `as_bytes()` es una vista de copia cero en la cabecera ya codificada.
    inner: [u8; 6],
}

impl CcsdsHeader {
    // ──────────────────────────────────────────────────────────────────────────
    // Auxiliar privado: llama a C y convierte su código de error a nuestro tipo de error.
    // Este es el ÚNICO lugar donde aparece unsafe en toda la API pública.
    // ──────────────────────────────────────────────────────────────────────────
    fn pack_validated(
        apid: u16,
        seq_count: u16,
        data_len: u16,
        is_tc: bool,
    ) -> Result<[u8; 6], CcsdsError> {
        // Validar ANTES de llamar a C. Nosotros somos responsables de estas verificaciones; C también las hace,
        // pero queremos variantes de error Rust precisas, no simplemente "C devolvió -1".
        if apid > 0x07FF {
            return Err(CcsdsError::ApidOutOfRange { provided: apid });
        }
        if seq_count > 0x3FFF {
            return Err(CcsdsError::SeqCountOutOfRange { provided: seq_count });
        }
        if data_len == 0 {
            return Err(CcsdsError::DataLenZero);
        }

        let mut raw_hdr = CcsdsPrimaryHeaderRaw { raw: [0u8; 6] };

        // Invariantes de seguridad que afirmamos aquí:
        //   1. raw_hdr es un CcsdsPrimaryHeaderRaw válido, alineado y de propiedad local.
        //   2. Acabamos de validar todos los argumentos de entrada, por lo que las precondiciones de C se cumplen.
        //   3. C no almacenará el puntero más allá de esta llamada.
        let ret = unsafe {
            ccsds_pack(
                &mut raw_hdr as *mut CcsdsPrimaryHeaderRaw,
                apid,
                seq_count,
                data_len,
                if is_tc { 1 } else { 0 },
            )
        };

        if ret != 0 {
            // Esto no debería ocurrir ya que pre-validamos, pero lo manejamos de todas formas.
            return Err(CcsdsError::CFunctionFailed { code: ret });
        }

        Ok(raw_hdr.raw)
    }

    /// Crea una nueva cabecera de Telecomando (TC).
    ///
    /// Devuelve `Err` si algún campo está fuera de rango.
    ///
    /// Restricciones de campos CCSDS:
    ///   - `apid`: 0–2047 (11 bits)
    ///   - `seq_count`: 0–16383 (14 bits)
    ///   - `data_len`: ≥ 1 (almacenado como `data_len - 1` según la especificación CCSDS)
    pub fn new_tc(apid: u16, seq_count: u16, data_len: u16) -> Result<Self, CcsdsError> {
        let bytes = Self::pack_validated(apid, seq_count, data_len, true)?;
        Ok(CcsdsHeader { inner: bytes })
    }

    /// Crea una nueva cabecera de Telemetría (TM).
    pub fn new_tm(apid: u16, seq_count: u16, data_len: u16) -> Result<Self, CcsdsError> {
        let bytes = Self::pack_validated(apid, seq_count, data_len, false)?;
        Ok(CcsdsHeader { inner: bytes })
    }

    /// Decodifica una cabecera CCSDS cruda existente de 6 bytes.
    ///
    /// Útil cuando recibiste bytes a través de un enlace serie y quieres
    /// una vista estructurada.
    pub fn from_bytes(raw: [u8; 6]) -> Self {
        // Confiamos en los bytes tal como están. Si vinieron del cable, pueden o no
        // ser CCSDS válido — es responsabilidad del llamador. Solo proporcionamos la API de decodificación.
        CcsdsHeader { inner: raw }
    }

    /// Los 6 bytes crudos de la cabecera para transmisión.
    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.inner
    }

    /// El Identificador de Proceso de Aplicación (11 bits, 0–2047).
    pub fn apid(&self) -> u16 {
        // Safety: self.inner es siempre un buffer de 6 bytes válido.
        let mut apid: u16 = 0;
        let raw = CcsdsPrimaryHeaderRaw { raw: self.inner };
        unsafe {
            ccsds_unpack(&raw, &mut apid, std::ptr::null_mut(), std::ptr::null_mut());
        }
        apid
    }

    /// El conteo de secuencia (14 bits, 0–16383).
    pub fn seq_count(&self) -> u16 {
        let mut seq: u16 = 0;
        let raw = CcsdsPrimaryHeaderRaw { raw: self.inner };
        // Safety: raw es una copia local válida de nuestro buffer de 6 bytes.
        unsafe {
            ccsds_unpack(&raw, std::ptr::null_mut(), &mut seq, std::ptr::null_mut());
        }
        seq
    }

    /// La longitud del campo de datos en bytes (siempre ≥ 1).
    ///
    /// Este es el recuento de bytes *real* del campo de datos, no el valor almacenado
    /// (que es `data_len - 1` según CCSDS). La envoltura maneja este ajuste.
    pub fn data_len(&self) -> u16 {
        let mut len: u16 = 0;
        let raw = CcsdsPrimaryHeaderRaw { raw: self.inner };
        // Safety: raw es una copia local válida.
        unsafe {
            ccsds_unpack(&raw, std::ptr::null_mut(), std::ptr::null_mut(), &mut len);
        }
        len
    }

    /// Devuelve `true` si este es un paquete de Telecomando (TC).
    pub fn is_tc(&self) -> bool {
        let raw = CcsdsPrimaryHeaderRaw { raw: self.inner };
        // Safety: raw es una copia local válida.
        let result = unsafe { ccsds_is_tc(&raw) };
        result != 0
    }

    /// Devuelve `true` si este es un paquete de Telemetría (TM).
    pub fn is_tm(&self) -> bool {
        !self.is_tc()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Implementación de Display para salida legible por humanos
// ──────────────────────────────────────────────────────────────────────────────
impl std::fmt::Display for CcsdsHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CcsdsHeader {{ tipo: {}, apid: 0x{:03X}, seq: {}, data_len: {}, bytes: {:02X?} }}",
            if self.is_tc() { "TC" } else { "TM" },
            self.apid(),
            self.seq_count(),
            self.data_len(),
            self.inner,
        )
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Pruebas unitarias
//
// Viven en el mismo archivo para este ejemplo. En una base de código real vivirían
// en un bloque `#[cfg(test)] mod tests { ... }` o en un archivo de prueba separado.
//
// Ejecutar con: cargo test --example 02_safe_wrapper
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// Ida y vuelta: empaquetar una cabecera TC, luego leer los campos de vuelta.
    #[test]
    fn tc_round_trip() {
        let hdr = CcsdsHeader::new_tc(0x100, 7, 12).expect("new_tc debe tener éxito");
        assert_eq!(hdr.apid(), 0x100);
        assert_eq!(hdr.seq_count(), 7);
        assert_eq!(hdr.data_len(), 12);
        assert!(hdr.is_tc());
        assert!(!hdr.is_tm());
    }

    /// Ida y vuelta: empaquetar una cabecera TM, luego leer los campos de vuelta.
    #[test]
    fn tm_round_trip() {
        let hdr = CcsdsHeader::new_tm(0x050, 1023, 64).expect("new_tm debe tener éxito");
        assert_eq!(hdr.apid(), 0x050);
        assert_eq!(hdr.seq_count(), 1023);
        assert_eq!(hdr.data_len(), 64);
        assert!(hdr.is_tm());
        assert!(!hdr.is_tc());
    }

    /// Caso límite: valores de campo máximos válidos.
    #[test]
    fn max_valid_fields() {
        let hdr = CcsdsHeader::new_tm(0x7FF, 0x3FFF, 0xFFFF).expect("campos máximos válidos");
        assert_eq!(hdr.apid(), 0x7FF);
        assert_eq!(hdr.seq_count(), 0x3FFF);
        assert_eq!(hdr.data_len(), 0xFFFF);
    }

    /// Caso límite: valores de campo mínimos válidos.
    #[test]
    fn min_valid_fields() {
        let hdr = CcsdsHeader::new_tc(0, 0, 1).expect("campos mínimos válidos");
        assert_eq!(hdr.apid(), 0);
        assert_eq!(hdr.seq_count(), 0);
        assert_eq!(hdr.data_len(), 1);
    }

    /// APID = 2048 es uno más que el rango válido de 11 bits (máx = 2047).
    #[test]
    fn apid_out_of_range() {
        let err = CcsdsHeader::new_tc(2048, 0, 1).expect_err("debe rechazar APID 2048");
        assert_eq!(err, CcsdsError::ApidOutOfRange { provided: 2048 });
    }

    /// seq_count = 16384 es uno más que el rango válido de 14 bits (máx = 16383).
    #[test]
    fn seq_count_out_of_range() {
        let err =
            CcsdsHeader::new_tm(0, 16384, 1).expect_err("debe rechazar seq_count 16384");
        assert_eq!(err, CcsdsError::SeqCountOutOfRange { provided: 16384 });
    }

    /// data_len = 0 es inválido porque el campo CCSDS almacena data_len-1,
    /// y el desbordamiento haría que la cabecera codifique una longitud incorrecta.
    #[test]
    fn data_len_zero_rejected() {
        let err = CcsdsHeader::new_tc(1, 0, 0).expect_err("debe rechazar data_len=0");
        assert_eq!(err, CcsdsError::DataLenZero);
    }

    /// Dos cabeceras con los mismos campos deben tener la misma representación de bytes.
    #[test]
    fn deterministic_encoding() {
        let a = CcsdsHeader::new_tc(0x200, 5, 8).unwrap();
        let b = CcsdsHeader::new_tc(0x200, 5, 8).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    /// Un TC y un TM con el mismo APID deben diferir en su bit de tipo (byte 0, bit 4).
    #[test]
    fn tc_and_tm_differ_in_type_bit() {
        let tc = CcsdsHeader::new_tc(0x100, 1, 4).unwrap();
        let tm = CcsdsHeader::new_tm(0x100, 1, 4).unwrap();
        // El byte 0 tiene el bit de tipo en el bit 4; TC debe tenerlo activado.
        assert_ne!(tc.as_bytes()[0], tm.as_bytes()[0]);
        // Los bytes 1–5 deben ser iguales (solo difiere el bit de tipo).
        assert_eq!(tc.as_bytes()[1..], tm.as_bytes()[1..]);
    }
}

fn main() {
    println!("=== Ejemplo 02: Envoltura Segura alrededor de C FFI ===\n");

    // ──────────────────────────────────────────────────────────────────────────
    // Usar la API segura: no hay unsafe en ningún lugar de esta ruta de código.
    // Todo el unsafe está oculto dentro de los auxiliares privados de CcsdsHeader.
    // ──────────────────────────────────────────────────────────────────────────

    // Crear una cabecera TC — la API valida nuestras entradas antes de tocar C.
    let tc_header = CcsdsHeader::new_tc(0x100, 1, 4)
        .expect("la cabecera TC válida debe tener éxito");

    println!("Cabecera TC: {}", tc_header);
    println!("  as_bytes: {:02X?}", tc_header.as_bytes());
    println!();

    // Crear una cabecera TM — para telemetría que regresa a tierra.
    let tm_header = CcsdsHeader::new_tm(0x050, 42, 64)
        .expect("la cabecera TM válida debe tener éxito");

    println!("Cabecera TM: {}", tm_header);
    println!("  as_bytes: {:02X?}", tm_header.as_bytes());
    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // Demostrar manejo de errores: las entradas inválidas devuelven Err, no un fallo.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- Manejo de errores ---");

    match CcsdsHeader::new_tc(0x800, 0, 1) {
        Ok(_) => panic!("debería haber sido rechazado"),
        Err(e) => println!("  APID demasiado grande: {}", e),
    }

    match CcsdsHeader::new_tc(0, 20000, 1) {
        Ok(_) => panic!("debería haber sido rechazado"),
        Err(e) => println!("  seq_count demasiado grande: {}", e),
    }

    match CcsdsHeader::new_tc(0, 0, 0) {
        Ok(_) => panic!("debería haber sido rechazado"),
        Err(e) => println!("  data_len cero: {}", e),
    }

    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // Demostrar from_bytes: decodificar una cabecera recibida del cable.
    // ──────────────────────────────────────────────────────────────────────────
    println!("--- from_bytes (decodificar cabecera recibida) ---");
    let raw_from_wire: [u8; 6] = [0x11, 0x00, 0xC0, 0x01, 0x00, 0x03];
    let decoded = CcsdsHeader::from_bytes(raw_from_wire);
    println!("  Decodificado: {}", decoded);
    assert_eq!(decoded.apid(), 0x100);
    assert_eq!(decoded.seq_count(), 1);
    assert_eq!(decoded.data_len(), 4);
    assert!(decoded.is_tc());

    println!("\nTodas las demostraciones de envoltura segura: PASADAS");
}
