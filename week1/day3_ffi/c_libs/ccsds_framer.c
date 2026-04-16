/*
 * ccsds_framer.c — codificador/decodificador de la Cabecera Primaria del Paquete Espacial CCSDS.
 *
 * Implementa las funciones declaradas en ccsds_framer.h.
 *
 * Resumen de la distribución de bits (campos big-endian empaquetados en 6 bytes):
 *
 *   Byte 0: [V V V T S A A A]   V=versión(3), T=tipo(1), S=cab_sec(1), A=apid[10:8](3)
 *   Byte 1: [A A A A A A A A]   A=apid[7:0](8)
 *   Byte 2: [F F C C C C C C]   F=banderas_seq[1:0](2), C=conteo_seq[13:8](6)
 *   Byte 3: [C C C C C C C C]   C=conteo_seq[7:0](8)
 *   Byte 4: [L L L L L L L L]   L=longitud_datos_menos1[15:8](8)
 *   Byte 5: [L L L L L L L L]   L=longitud_datos_menos1[7:0](8)
 *
 * Todos los campos multibyte son big-endian (orden de bytes de red), según lo requerido por CCSDS.
 */

#include "ccsds_framer.h"
#include <stddef.h>  /* NULL */

int ccsds_pack(struct CcsdsPrimaryHeader *hdr,
               uint16_t apid,
               uint16_t seq_count,
               uint16_t data_len,
               int is_tc)
{
    if (hdr == NULL) {
        return -1;
    }
    /* APID es de 11 bits: 0x000 – 0x7FF */
    if (apid > 0x07FFu) {
        return -1;
    }
    /* El conteo de secuencia es de 14 bits: 0 – 16383 */
    if (seq_count > 0x3FFFu) {
        return -1;
    }
    /* data_len debe ser al menos 1; el valor almacenado = data_len - 1 cabe en uint16_t */
    if (data_len < 1u) {
        return -1;
    }

    uint16_t stored_len = (uint16_t)(data_len - 1u);

    /*
     * Palabra 0 (bytes 0-1):
     *   bits 15-13 : versión = 0b000
     *   bit  12    : tipo    = is_tc ? 1 : 0
     *   bit  11    : bandera de cabecera secundaria = 0 (sin cabecera secundaria en estos ejemplos)
     *   bits 10-0  : APID
     */
    uint16_t word0 = 0u;
    if (is_tc) {
        word0 |= (uint16_t)(1u << 12);   /* bit de tipo */
    }
    word0 |= (uint16_t)(apid & 0x07FFu); /* 11 bits inferiores */

    hdr->raw[0] = (uint8_t)((word0 >> 8) & 0xFFu);
    hdr->raw[1] = (uint8_t)( word0       & 0xFFu);

    /*
     * Palabra 1 (bytes 2-3):
     *   bits 15-14 : banderas de secuencia = 0b11 (paquete independiente / no segmentado)
     *   bits 13-0  : conteo de secuencia
     */
    uint16_t word1 = (uint16_t)(0x3u << 14);          /* banderas_seq = 0b11 */
    word1 |= (uint16_t)(seq_count & 0x3FFFu);

    hdr->raw[2] = (uint8_t)((word1 >> 8) & 0xFFu);
    hdr->raw[3] = (uint8_t)( word1       & 0xFFu);

    /*
     * Palabra 2 (bytes 4-5): campo de longitud de datos = data_len - 1
     */
    hdr->raw[4] = (uint8_t)((stored_len >> 8) & 0xFFu);
    hdr->raw[5] = (uint8_t)( stored_len       & 0xFFu);

    return 0;
}

int ccsds_unpack(const struct CcsdsPrimaryHeader *hdr,
                 uint16_t *apid,
                 uint16_t *seq_count,
                 uint16_t *data_len)
{
    if (hdr == NULL) {
        return -1;
    }

    /* Reconstruir la palabra 0 de los bytes 0-1 (big-endian) */
    uint16_t word0 = (uint16_t)(((uint16_t)hdr->raw[0] << 8) | hdr->raw[1]);
    /* Reconstruir la palabra 1 de los bytes 2-3 */
    uint16_t word1 = (uint16_t)(((uint16_t)hdr->raw[2] << 8) | hdr->raw[3]);
    /* Reconstruir la palabra 2 de los bytes 4-5 */
    uint16_t word2 = (uint16_t)(((uint16_t)hdr->raw[4] << 8) | hdr->raw[5]);

    if (apid != NULL) {
        /* APID está en los bits 10-0 de la palabra 0 */
        *apid = (uint16_t)(word0 & 0x07FFu);
    }

    if (seq_count != NULL) {
        /* El conteo de secuencia está en los bits 13-0 de la palabra 1 */
        *seq_count = (uint16_t)(word1 & 0x3FFFu);
    }

    if (data_len != NULL) {
        /* El valor almacenado es data_len - 1; restaurarlo */
        *data_len = (uint16_t)(word2 + 1u);
    }

    return 0;
}

int ccsds_is_tc(const struct CcsdsPrimaryHeader *hdr)
{
    if (hdr == NULL) {
        return 0;
    }
    /* El bit de tipo es el bit 4 del byte 0 (bit 12 de la palabra 0) */
    return (hdr->raw[0] & 0x10u) ? 1 : 0;
}
