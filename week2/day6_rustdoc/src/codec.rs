//! Codificación/decodificación a nivel de bytes para [`CcsdsPrimaryHeader`].
//!
//! Estas funciones forman el límite entre el mundo tipado de Rust y los
//! buffers de bytes crudos usados por los controladores DMA y los drivers de red. Se
//! mantienen intencionalmente delgadas: sin asignación, sin E/S, sin dependencias.
//!
//! ## ¿Por qué un Módulo Separado?
//!
//! Separar la serialización del tipo de dominio sigue el *Principio de
//! Responsabilidad Única*: [`crate::frame::CcsdsPrimaryHeader`] sabe sobre
//! los campos de la cabecera; este módulo sabe sobre el formato del cable. Si CCSDS
//! alguna vez añade un formato de cabecera versión 2, solo `codec.rs` necesita cambiar.

use crate::error::CcsdsError;
use crate::frame::CcsdsPrimaryHeader;

/// Codifica un [`CcsdsPrimaryHeader`] en un array big-endian de 6 bytes.
///
/// La salida es adecuada para transmisión directa: puede escribirse en un
/// buffer DMA, serializarse en una trama UART, o anteponerse a un payload UDP.
///
/// Esta función es infalible porque un [`CcsdsPrimaryHeader`] en memoria es
/// siempre válido por construcción (los constructores imponen las invariantes).
///
/// # Rendimiento
///
/// Esto es una copia de 6 bytes. No ocurre ninguna asignación. La función se expande en línea
/// esencialmente a un `memcpy` en compilaciones de lanzamiento.
///
/// # Ejemplos
///
/// ```rust
/// use day6_rustdoc::frame::CcsdsPrimaryHeader;
/// use day6_rustdoc::codec::{encode, decode};
///
/// let hdr = CcsdsPrimaryHeader::new_tc(0x42, 7, 20)?;
/// let bytes = encode(&hdr);
/// assert_eq!(bytes.len(), 6);
///
/// // Los bytes codificados pueden decodificarse de vuelta a una cabecera equivalente.
/// let decoded = decode(&bytes)?;
/// assert_eq!(decoded.apid(), 0x42);
/// assert_eq!(decoded.seq_count(), 7);
/// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
/// ```
pub fn encode(hdr: &CcsdsPrimaryHeader) -> [u8; 6] {
    // Delegar al propio to_bytes() de la cabecera para que haya exactamente un lugar
    // donde el diseño de bytes crudos sea autoritativo.
    hdr.to_bytes()
}

/// Decodifica un array de 6 bytes en un [`CcsdsPrimaryHeader`].
///
/// Valida el campo de versión y el APID. Devuelve un error si los bytes no
/// representan una cabecera primaria CCSDS bien formada.
///
/// # Argumentos
///
/// - `bytes`: Exactamente 6 bytes en orden big-endian del cable CCSDS.
///
/// # Errores
///
/// - [`CcsdsError::UnsupportedVersion`] si los bits \[15:13\] del byte 0 son distintos de cero.
/// - [`CcsdsError::InvalidApid`] si los bits \[10:0\] de los bytes 0–1 son iguales a `0x7FF`.
///
/// # Ejemplos
///
/// ```rust
/// use day6_rustdoc::frame::CcsdsPrimaryHeader;
/// use day6_rustdoc::codec::{encode, decode};
///
/// // Ida y vuelta: construir → codificar → decodificar.
/// let original = CcsdsPrimaryHeader::new_tm(0x10, 3, 64)?;
/// let bytes = encode(&original);
/// let recovered = decode(&bytes)?;
/// assert_eq!(original, recovered);
/// # Ok::<(), day6_rustdoc::error::CcsdsError>(())
/// ```
///
/// Manejando un buffer corrupto:
///
/// ```rust
/// use day6_rustdoc::codec::decode;
/// use day6_rustdoc::error::CcsdsError;
///
/// // Primer byte 0x20 = bits de versión 001 — no 000 como se requiere.
/// let bad_bytes = [0x20, 0x00, 0xC0, 0x00, 0x00, 0x03];
/// match decode(&bad_bytes) {
///     Err(CcsdsError::UnsupportedVersion { version }) => {
///         println!("Cabecera corrupta rechazada: versión={}", version);
///     }
///     other => panic!("resultado inesperado: {:?}", other),
/// }
/// ```
pub fn decode(bytes: &[u8; 6]) -> Result<CcsdsPrimaryHeader, CcsdsError> {
    CcsdsPrimaryHeader::from_bytes(*bytes)
}

/// Decodifica una cabecera primaria de un slice de bytes, verificando la longitud primero.
///
/// Este es un envoltorio de conveniencia alrededor de [`decode`] para situaciones donde el
/// tamaño de entrada no se conoce estáticamente (por ejemplo, al leer de un buffer de socket).
///
/// # Errores
///
/// - [`CcsdsError::BufferTooShort`] si `bytes.len() < 6`.
/// - Todos los errores de [`decode`].
///
/// # Ejemplos
///
/// ```rust
/// use day6_rustdoc::codec::decode_slice;
/// use day6_rustdoc::error::CcsdsError;
///
/// let short: &[u8] = &[0x18, 0x01];
/// match decode_slice(short) {
///     Err(CcsdsError::BufferTooShort { got, expected }) => {
///         assert_eq!(got, 2);
///         assert_eq!(expected, 6);
///     }
///     other => panic!("inesperado: {:?}", other),
/// }
/// ```
pub fn decode_slice(bytes: &[u8]) -> Result<CcsdsPrimaryHeader, CcsdsError> {
    if bytes.len() < 6 {
        return Err(CcsdsError::BufferTooShort {
            got: bytes.len(),
            expected: 6,
        });
    }
    // SAFETY: acabamos de verificar que bytes.len() >= 6.
    let arr: [u8; 6] = bytes[..6].try_into().expect("el slice tiene exactamente 6 bytes");
    decode(&arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CcsdsError;

    #[test]
    fn encode_decode_round_trip() {
        let hdr = CcsdsPrimaryHeader::new_tc(0x100, 10, 50).unwrap();
        let bytes = encode(&hdr);
        let decoded = decode(&bytes).unwrap();
        assert_eq!(hdr, decoded);
    }

    #[test]
    fn decode_slice_too_short() {
        let short = [0u8; 4];
        match decode_slice(&short) {
            Err(CcsdsError::BufferTooShort { got: 4, expected: 6 }) => {}
            other => panic!("inesperado: {:?}", other),
        }
    }

    #[test]
    fn decode_slice_exact_length() {
        let hdr = CcsdsPrimaryHeader::new_tm(0x20, 99, 8).unwrap();
        let bytes: Vec<u8> = hdr.to_bytes().to_vec();
        let decoded = decode_slice(&bytes).unwrap();
        assert_eq!(hdr, decoded);
    }
}
