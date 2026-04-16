//! Tipo de cabecera primaria de Space Packet CCSDS y accesores de campos.
//!
//! Este módulo proporciona [`CcsdsPrimaryHeader`], una representación de copia cero
//! de la cabecera primaria CCSDS de 6 octetos definida en CCSDS 133.0-B-2.

use crate::error::CcsdsError;

/// Una cabecera primaria de Space Packet CCSDS.
///
/// La cabecera primaria tiene 6 octetos (48 bits) con el siguiente diseño de bits:
///
/// ```text
/// Palabra  Bits    Campo
/// ────────────────────────────────────────────────────────────
/// 0-1    15-13   Número de Versión del Paquete (siempre 0b000)
/// 0-1    12      Tipo de Paquete          (0 = TM,  1 = TC)
/// 0-1    11      Indicador de Cabecera Secundaria (1 = presente)
/// 0-1    10-0    Identificador de Proceso de Aplicación (APID)
/// 2-3    15-14   Indicadores de Secuencia  (0b11 = independiente)
/// 2-3    13-0    Conteo de Secuencia del Paquete (14 bits, por APID)
/// 4-5    15-0    Longitud de Datos del Paquete    (data_octets - 1)
/// ────────────────────────────────────────────────────────────
/// ```
///
/// Los bytes crudos se almacenan en orden big-endian según lo exige CCSDS.
///
/// # Invariantes
///
/// Un valor de este tipo siempre satisface:
/// - `version == 0` (bits \[15:13\] de la palabra 0)
/// - `apid <= 0x7FE` (bits \[10:0\] de la palabra 0; 0x7FF es el APID inactivo)
/// - `seq_count <= 0x3FFF` (bits \[13:0\] de la palabra 1)
///
/// Estas invariantes se verifican en todos los constructores, por lo que cualquier `CcsdsPrimaryHeader`
/// existente está garantizado de ser válido.
///
/// # Referencias
/// - CCSDS 133.0-B-2, Sección 4.1 — *Space Packet Primary Header*
#[derive(Debug, Clone, PartialEq)]
pub struct CcsdsPrimaryHeader {
    // Almacenar la representación cruda de 6 bytes.
    //
    // POR QUÉ bytes crudos en lugar de campos individuales?
    //   1. Copia cero: podemos convertir buffers DMA directamente (con from_bytes).
    //   2. La serialización es trivial: to_bytes() es una sola copia.
    //   3. El struct tiene exactamente el tamaño del cable — sin sorpresas de relleno.
    raw: [u8; 6],
}

impl CcsdsPrimaryHeader {
    // ── Auxiliares internos ───────────────────────────────────────────────

    /// Devuelve la palabra de 16 bits en el desplazamiento de byte 0 (big-endian).
    #[inline]
    fn word0(&self) -> u16 {
        u16::from_be_bytes([self.raw[0], self.raw[1]])
    }

    /// Devuelve la palabra de 16 bits en el desplazamiento de byte 2 (big-endian).
    #[inline]
    fn word1(&self) -> u16 {
        u16::from_be_bytes([self.raw[2], self.raw[3]])
    }

    /// Devuelve la palabra de 16 bits en el desplazamiento de byte 4 (big-endian).
    #[inline]
    fn word2(&self) -> u16 {
        u16::from_be_bytes([self.raw[4], self.raw[5]])
    }

    // ── Constructores ─────────────────────────────────────────────────────

    /// Crea una nueva cabecera primaria de Telecomando (TC).
    ///
    /// Establece packet_type = 1 (TC), secondary_header_flag = 1 (presente),
    /// y sequence_flags = 0b11 (paquete independiente — sin segmentación).
    ///
    /// # Argumentos
    ///
    /// - `apid`: Identificador de Proceso de Aplicación, rango `0x000..=0x7FE`.
    ///   El valor `0x7FF` está reservado como APID inactivo y por lo tanto es rechazado.
    /// - `seq_count`: Conteo de secuencia del paquete en `0..=0x3FFF`.
    ///   Los llamadores deben usar [`Self::next_seq_count`] para avanzar el contador
    ///   correctamente en los desbordamientos.
    /// - `data_len`: Longitud del *campo de datos del paquete* en octetos.
    ///   La cabecera almacena `data_len - 1` según la especificación CCSDS (ver §4.1.3).
    ///   Un valor de `0` significa que el campo de datos del paquete tiene 1 octeto de longitud.
    ///
    /// # Errores
    ///
    /// - [`CcsdsError::InvalidApid`] si `apid > 0x7FE`.
    /// - [`CcsdsError::InvalidSeqCount`] si `seq_count > 0x3FFF`.
    ///
    /// # Ejemplos
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tc(0x100, 1, 10)?;
    /// assert_eq!(hdr.apid(), 0x100);
    /// assert_eq!(hdr.seq_count(), 1);
    /// assert_eq!(hdr.data_len(), 10);
    /// assert!(hdr.is_tc());
    /// assert!(!hdr.is_tm());
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn new_tc(apid: u16, seq_count: u16, data_len: u16) -> Result<Self, CcsdsError> {
        Self::new_inner(
            /*packet_type=*/ 1,
            /*sec_hdr=*/ 1,
            apid,
            seq_count,
            data_len,
        )
    }

    /// Crea una nueva cabecera primaria de Telemetría (TM).
    ///
    /// Establece packet_type = 0 (TM), secondary_header_flag = 1 (presente),
    /// y sequence_flags = 0b11 (paquete independiente).
    ///
    /// # Argumentos
    ///
    /// Ver [`Self::new_tc`] — los argumentos son idénticos.
    ///
    /// # Errores
    ///
    /// - [`CcsdsError::InvalidApid`] si `apid > 0x7FE`.
    /// - [`CcsdsError::InvalidSeqCount`] si `seq_count > 0x3FFF`.
    ///
    /// # Ejemplos
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tm(0x200, 42, 128)?;
    /// assert!(hdr.is_tm());
    /// assert!(!hdr.is_tc());
    /// assert_eq!(hdr.apid(), 0x200);
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn new_tm(apid: u16, seq_count: u16, data_len: u16) -> Result<Self, CcsdsError> {
        Self::new_inner(
            /*packet_type=*/ 0,
            /*sec_hdr=*/ 1,
            apid,
            seq_count,
            data_len,
        )
    }

    /// Lógica de construcción compartida.
    ///
    /// Privada: los llamadores deben usar [`Self::new_tc`] o [`Self::new_tm`].
    fn new_inner(
        packet_type: u16,
        sec_hdr: u16,
        apid: u16,
        seq_count: u16,
        data_len: u16,
    ) -> Result<Self, CcsdsError> {
        // Validar APID: debe caber en 11 bits y no ser el APID inactivo (0x7FF).
        //
        // POR QUÉ rechazar 0x7FF? Según CCSDS 133.0-B-2 §4.1.2.3.2, el APID de paquete
        // inactivo está reservado para paquetes de relleno. Crear una cabecera "real" con el
        // APID inactivo la haría indistinguible del relleno y podría causar que
        // los receptores la descarten silenciosamente.
        if apid > 0x7FE {
            return Err(CcsdsError::InvalidApid { value: apid });
        }

        // Validar el conteo de secuencia: debe caber en 14 bits.
        if seq_count > 0x3FFF {
            return Err(CcsdsError::InvalidSeqCount { value: seq_count });
        }

        // Construir la Palabra 0:
        //   [15:13] versión = 0b000
        //   [12]    packet_type
        //   [11]    indicador de cabecera secundaria
        //   [10:0]  APID
        //
        // POR QUÉ desplazamientos explícitos en lugar de un crate de campos de bits?
        // Los crates de campos de bits añaden una dependencia y abstraen el formato del cable.
        // Aquí queremos que el practicante vea el diseño exacto de bits CCSDS en el código.
        let word0: u16 = (packet_type << 12) | (sec_hdr << 11) | (apid & 0x07FF);

        // Construir la Palabra 1:
        //   [15:14] indicadores de secuencia = 0b11 (independiente / no segmentado)
        //   [13:0]  conteo de secuencia
        //
        // Independiente (0b11) significa que este paquete es completo por sí solo y no es
        // un segmento de una PDU más grande. El software de vuelo generalmente envía paquetes
        // independientes; la segmentación es rara.
        let word1: u16 = (0b11 << 14) | (seq_count & 0x3FFF);

        // Construir la Palabra 2:
        //   [15:0]  longitud de datos del paquete (= total de octetos del campo de datos - 1)
        //
        // POR QUÉ menos uno? CCSDS §4.1.3.2: "La Longitud de Datos del Paquete es un campo
        // de 16 bits que contiene un valor que es uno menos que la longitud en octetos
        // del Campo de Datos del Paquete." Este es un clásico error de uno en uno que confunde
        // a los ingenieros nuevos. Lo almacenamos exactamente como especifica el formato del cable.
        let word2: u16 = data_len.saturating_sub(1);

        let w0 = word0.to_be_bytes();
        let w1 = word1.to_be_bytes();
        let w2 = word2.to_be_bytes();

        Ok(Self {
            raw: [w0[0], w0[1], w1[0], w1[1], w2[0], w2[1]],
        })
    }

    // ── Accesores de Campos ───────────────────────────────────────────────

    /// Devuelve el Identificador de Proceso de Aplicación (APID).
    ///
    /// El APID ocupa los bits \[10:0\] de la primera palabra de 16 bits.
    /// El rango válido es `0x000..=0x7FE`; `0x7FF` (inactivo) nunca se devuelve
    /// porque el constructor lo rechaza.
    ///
    /// En un sistema de vuelo, el APID identifica el *proceso de aplicación* a bordo
    /// — aproximadamente equivalente a un ID de proceso o tarea para propósitos de enrutamiento.
    ///
    /// # Ejemplos
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tc(0x1AB, 0, 8)?;
    /// assert_eq!(hdr.apid(), 0x1AB);
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn apid(&self) -> u16 {
        self.word0() & 0x07FF
    }

    /// Devuelve el Conteo de Secuencia del Paquete (14 bits).
    ///
    /// El conteo de secuencia ocupa los bits \[13:0\] de la segunda palabra de 16 bits.
    /// Se incrementa en 1 por cada nuevo paquete en un APID dado y se desborda en
    /// `0x3FFF` (16383) volviendo a 0.
    ///
    /// Los receptores usan el conteo de secuencia para detectar paquetes perdidos: una brecha en el
    /// conteo (por ejemplo, saltar de 5 a 7) indica que el paquete 6 se perdió.
    ///
    /// Usa [`Self::next_seq_count`] para avanzar el contador con desbordamiento correcto.
    ///
    /// # Ejemplos
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tm(0x10, 999, 32)?;
    /// assert_eq!(hdr.seq_count(), 999);
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn seq_count(&self) -> u16 {
        self.word1() & 0x3FFF
    }

    /// Devuelve el valor del campo Longitud de Datos del Paquete.
    ///
    /// Según CCSDS 133.0-B-2 §4.1.3.2, el valor almacenado es `(data_octets - 1)`.
    /// Este accesor devuelve el valor almacenado crudo, por lo que los llamadores que quieran
    /// el tamaño real del campo de datos deben sumar 1: `hdr.data_len() + 1`.
    ///
    /// # Ejemplos
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// // Crear una cabecera con un campo de datos de 10 octetos.
    /// // El constructor establece el valor almacenado en 10 - 1 = 9.
    /// let hdr = CcsdsPrimaryHeader::new_tc(0x01, 0, 10)?;
    /// assert_eq!(hdr.data_len(), 9);          // valor almacenado
    /// assert_eq!(hdr.data_len() + 1, 10);     // tamaño real del campo de datos
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn data_len(&self) -> u16 {
        self.word2()
    }

    /// Devuelve `true` si este es un paquete de Telecomando (TC).
    ///
    /// Verifica el bit \[12\] de la palabra 0. Los paquetes TC se originan en tierra y
    /// se envían por enlace ascendente a la nave espacial.
    ///
    /// # Ejemplos
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let tc = CcsdsPrimaryHeader::new_tc(0x01, 0, 4)?;
    /// assert!(tc.is_tc());
    /// assert!(!tc.is_tm());
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn is_tc(&self) -> bool {
        (self.word0() >> 12) & 1 == 1
    }

    /// Devuelve `true` si este es un paquete de Telemetría (TM).
    ///
    /// Verifica el bit \[12\] de la palabra 0. Los paquetes TM se originan en la nave espacial y
    /// se envían por enlace descendente a tierra.
    ///
    /// # Ejemplos
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let tm = CcsdsPrimaryHeader::new_tm(0x02, 5, 64)?;
    /// assert!(tm.is_tm());
    /// assert!(!tm.is_tc());
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn is_tm(&self) -> bool {
        !self.is_tc()
    }

    /// Devuelve `true` si el indicador de cabecera secundaria está activado.
    ///
    /// Bit \[11\] de la palabra 0. Cuando está activado, una cabecera secundaria sigue inmediatamente
    /// a la cabecera primaria en el campo de datos del paquete. Los paquetes PUS siempre tienen
    /// una cabecera secundaria.
    pub fn has_secondary_header(&self) -> bool {
        (self.word0() >> 11) & 1 == 1
    }

    /// Devuelve la representación cruda de 6 bytes de la cabecera.
    ///
    /// Los bytes están en orden big-endian del cable, adecuados para transmisión
    /// directa o transferencia DMA.
    ///
    /// # Ejemplos
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tc(0x01, 0, 4)?;
    /// let bytes = hdr.to_bytes();
    /// assert_eq!(bytes.len(), 6);
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn to_bytes(&self) -> [u8; 6] {
        self.raw
    }

    /// Construye un [`CcsdsPrimaryHeader`] a partir de un array de 6 bytes.
    ///
    /// Valida que el campo de versión sea cero y que el APID no sea
    /// el APID inactivo (`0x7FF`). No valida `seq_count` porque un
    /// paquete analizado puede legítimamente tener cualquier valor de 14 bits.
    ///
    /// # Errores
    ///
    /// - [`CcsdsError::UnsupportedVersion`] si los bits \[15:13\] del byte 0 son distintos de cero.
    /// - [`CcsdsError::InvalidApid`] si el campo APID es igual a `0x7FF` (inactivo).
    ///
    /// # Ejemplos
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// let hdr = CcsdsPrimaryHeader::new_tc(0x42, 7, 20)?;
    /// let bytes = hdr.to_bytes();
    /// let decoded = CcsdsPrimaryHeader::from_bytes(bytes)?;
    /// assert_eq!(decoded, hdr);
    /// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
    /// ```
    pub fn from_bytes(bytes: [u8; 6]) -> Result<Self, CcsdsError> {
        let word0 = u16::from_be_bytes([bytes[0], bytes[1]]);

        // Verificar el campo de versión — debe ser 0.
        let version = (word0 >> 13) & 0b111;
        if version != 0 {
            return Err(CcsdsError::UnsupportedVersion { version: version as u8 });
        }

        // Verificar APID — rechazar el APID inactivo (0x7FF).
        let apid = word0 & 0x07FF;
        if apid > 0x7FE {
            return Err(CcsdsError::InvalidApid { value: apid });
        }

        Ok(Self { raw: bytes })
    }

    // ── Utilidades ────────────────────────────────────────────────────────

    /// Avanza un contador de secuencia en uno, desbordando en el límite de 14 bits.
    ///
    /// El conteo de secuencia CCSDS tiene 14 bits de ancho (máx `0x3FFF` = 16383). Después
    /// de alcanzar el máximo, se desborda a `0`. Esta función aplica el desbordamiento
    /// correctamente sin que el llamador necesite conocer el valor de la máscara.
    ///
    /// En un sistema de vuelo real se mantendría un `HashMap<u16, u16>` indexado
    /// por APID y se usaría esta función para avanzar el contador de cada APID independientemente.
    ///
    /// # Ejemplos
    ///
    /// ```
    /// use day6_rustdoc::frame::CcsdsPrimaryHeader;
    ///
    /// assert_eq!(CcsdsPrimaryHeader::next_seq_count(0),      1);
    /// assert_eq!(CcsdsPrimaryHeader::next_seq_count(16382), 16383);
    ///
    /// // Desbordamiento en el límite de 14 bits:
    /// assert_eq!(CcsdsPrimaryHeader::next_seq_count(0x3FFF), 0);
    /// ```
    pub fn next_seq_count(current: u16) -> u16 {
        // Aplicar la máscara de 14 bits después de la suma para imponer el desbordamiento.
        // Usando & en lugar de % porque & es una sola instrucción CPU y
        // nunca puede desbordarse.
        (current.wrapping_add(1)) & 0x3FFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_tc() {
        let hdr = CcsdsPrimaryHeader::new_tc(0x100, 42, 10).unwrap();
        let bytes = hdr.to_bytes();
        let decoded = CcsdsPrimaryHeader::from_bytes(bytes).unwrap();
        assert_eq!(hdr, decoded);
        assert!(decoded.is_tc());
        assert_eq!(decoded.apid(), 0x100);
        assert_eq!(decoded.seq_count(), 42);
    }

    #[test]
    fn round_trip_tm() {
        let hdr = CcsdsPrimaryHeader::new_tm(0x200, 1, 64).unwrap();
        let bytes = hdr.to_bytes();
        let decoded = CcsdsPrimaryHeader::from_bytes(bytes).unwrap();
        assert_eq!(hdr, decoded);
        assert!(decoded.is_tm());
    }

    #[test]
    fn invalid_apid_rejected() {
        assert!(matches!(
            CcsdsPrimaryHeader::new_tc(0x7FF, 0, 4),
            Err(CcsdsError::InvalidApid { value: 0x7FF })
        ));
        assert!(matches!(
            CcsdsPrimaryHeader::new_tc(0x800, 0, 4),
            Err(CcsdsError::InvalidApid { .. })
        ));
    }

    #[test]
    fn seq_count_wraps_correctly() {
        assert_eq!(CcsdsPrimaryHeader::next_seq_count(0x3FFF), 0);
        assert_eq!(CcsdsPrimaryHeader::next_seq_count(0x3FFE), 0x3FFF);
        assert_eq!(CcsdsPrimaryHeader::next_seq_count(0), 1);
    }

    #[test]
    fn data_len_stored_as_minus_one() {
        // Campo de datos de 10 octetos => valor almacenado es 9.
        let hdr = CcsdsPrimaryHeader::new_tc(0x01, 0, 10).unwrap();
        assert_eq!(hdr.data_len(), 9);
    }
}
