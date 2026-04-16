#ifndef CCSDS_FRAMER_H
#define CCSDS_FRAMER_H

/*
 * Protocolo de Paquetes Espaciales CCSDS — Cabecera Primaria
 * CCSDS 133.0-B-2 (Libro Azul)
 *
 * La cabecera primaria siempre tiene 6 octetos, distribuidos de la siguiente manera:
 *
 *  Octeto 0-1  : Versión(3b) | Tipo(1b) | Bandera Cab.Sec.(1b) | APID(11b)
 *  Octeto 2-3  : Banderas de Secuencia(2b) | Conteo de Secuencia(14b)
 *  Octeto 4-5  : Longitud de Datos del Paquete (total de octetos en el campo de datos - 1)
 *
 * Anchos y posiciones de campo:
 *   Versión     : bits 15-13 de la palabra 0  (siempre 0b000)
 *   Tipo        : bit  12    de la palabra 0  (0=TM, 1=TC)
 *   Bandera Cab.Sec. : bit  11    de la palabra 0  (puesto a 0 aquí por simplicidad)
 *   APID        : bits 10-0  de la palabra 0  (0x000 – 0x7FF)
 *   Banderas Seq.   : bits 15-14 de la palabra 1  (0b11 = paquete independiente)
 *   Conteo Seq. : bits 13-0  de la palabra 1  (0 – 16383)
 *   Longitud de Datos : bits 15-0  de la palabra 2  (bytes_datos_reales - 1)
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Cabecera primaria CCSDS cruda de 6 bytes. Tratar como opaca; usar pack/unpack. */
struct CcsdsPrimaryHeader {
    uint8_t raw[6];
};

/*
 * ccsds_pack — rellena una cabecera primaria CCSDS.
 *
 * Parámetros:
 *   hdr       : puntero a la estructura de cabecera a rellenar (no debe ser NULL)
 *   apid      : Identificador de Proceso de Aplicación, 11 bits (0 – 2047)
 *   seq_count : conteo de secuencia del paquete, 14 bits (0 – 16383)
 *   data_len  : número de octetos en el campo de datos (el campo se almacena como data_len-1)
 *               debe ser >= 1
 *   is_tc     : distinto de cero para Telecomando (tipo=1), cero para Telemetría (tipo=0)
 *
 * Devuelve 0 en caso de éxito, -1 en caso de error (puntero NULL o campo fuera de rango).
 */
int ccsds_pack(struct CcsdsPrimaryHeader *hdr,
               uint16_t apid,
               uint16_t seq_count,
               uint16_t data_len,
               int is_tc);

/*
 * ccsds_unpack — extrae campos de una cabecera primaria CCSDS.
 *
 * Parámetros:
 *   hdr       : puntero a la cabecera a leer (no debe ser NULL)
 *   apid      : salida — recibe el APID de 11 bits
 *   seq_count : salida — recibe el conteo de secuencia de 14 bits
 *   data_len  : salida — recibe la longitud real del campo de datos (valor_almacenado + 1)
 *
 * Cualquier puntero de salida puede ser NULL si ese campo no se necesita.
 * Devuelve 0 en caso de éxito, -1 si hdr es NULL.
 */
int ccsds_unpack(const struct CcsdsPrimaryHeader *hdr,
                 uint16_t *apid,
                 uint16_t *seq_count,
                 uint16_t *data_len);

/*
 * ccsds_is_tc — devuelve distinto de cero si el bit de tipo de la cabecera indica Telecomando.
 */
int ccsds_is_tc(const struct CcsdsPrimaryHeader *hdr);

#ifdef __cplusplus
}
#endif

#endif /* CCSDS_FRAMER_H */
