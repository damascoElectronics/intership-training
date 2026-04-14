/*
 * ccsds_framer.c — CCSDS Space Packet Primary Header encoder/decoder.
 *
 * Implements the functions declared in ccsds_framer.h.
 *
 * Bit-layout recap (big-endian fields packed into 6 bytes):
 *
 *   Byte 0: [V V V T S A A A]   V=version(3), T=type(1), S=sec_hdr(1), A=apid[10:8](3)
 *   Byte 1: [A A A A A A A A]   A=apid[7:0](8)
 *   Byte 2: [F F C C C C C C]   F=seq_flags[1:0](2), C=seq_count[13:8](6)
 *   Byte 3: [C C C C C C C C]   C=seq_count[7:0](8)
 *   Byte 4: [L L L L L L L L]   L=data_length_minus1[15:8](8)
 *   Byte 5: [L L L L L L L L]   L=data_length_minus1[7:0](8)
 *
 * All multi-byte fields are big-endian (network byte order), as required by CCSDS.
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
    /* APID is 11 bits: 0x000 – 0x7FF */
    if (apid > 0x07FFu) {
        return -1;
    }
    /* Sequence count is 14 bits: 0 – 16383 */
    if (seq_count > 0x3FFFu) {
        return -1;
    }
    /* data_len must be at least 1; stored value = data_len - 1 fits in uint16_t */
    if (data_len < 1u) {
        return -1;
    }

    uint16_t stored_len = (uint16_t)(data_len - 1u);

    /*
     * Word 0 (bytes 0-1):
     *   bits 15-13 : version = 0b000
     *   bit  12    : type    = is_tc ? 1 : 0
     *   bit  11    : secondary header flag = 0 (no secondary header in these examples)
     *   bits 10-0  : APID
     */
    uint16_t word0 = 0u;
    if (is_tc) {
        word0 |= (uint16_t)(1u << 12);   /* type bit */
    }
    word0 |= (uint16_t)(apid & 0x07FFu); /* lower 11 bits */

    hdr->raw[0] = (uint8_t)((word0 >> 8) & 0xFFu);
    hdr->raw[1] = (uint8_t)( word0       & 0xFFu);

    /*
     * Word 1 (bytes 2-3):
     *   bits 15-14 : sequence flags = 0b11 (standalone / unsegmented packet)
     *   bits 13-0  : sequence count
     */
    uint16_t word1 = (uint16_t)(0x3u << 14);          /* seq_flags = 0b11 */
    word1 |= (uint16_t)(seq_count & 0x3FFFu);

    hdr->raw[2] = (uint8_t)((word1 >> 8) & 0xFFu);
    hdr->raw[3] = (uint8_t)( word1       & 0xFFu);

    /*
     * Word 2 (bytes 4-5): data length field = data_len - 1
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

    /* Reconstruct word 0 from bytes 0-1 (big-endian) */
    uint16_t word0 = (uint16_t)(((uint16_t)hdr->raw[0] << 8) | hdr->raw[1]);
    /* Reconstruct word 1 from bytes 2-3 */
    uint16_t word1 = (uint16_t)(((uint16_t)hdr->raw[2] << 8) | hdr->raw[3]);
    /* Reconstruct word 2 from bytes 4-5 */
    uint16_t word2 = (uint16_t)(((uint16_t)hdr->raw[4] << 8) | hdr->raw[5]);

    if (apid != NULL) {
        /* APID is in bits 10-0 of word 0 */
        *apid = (uint16_t)(word0 & 0x07FFu);
    }

    if (seq_count != NULL) {
        /* Sequence count is in bits 13-0 of word 1 */
        *seq_count = (uint16_t)(word1 & 0x3FFFu);
    }

    if (data_len != NULL) {
        /* Stored value is data_len - 1; restore it */
        *data_len = (uint16_t)(word2 + 1u);
    }

    return 0;
}

int ccsds_is_tc(const struct CcsdsPrimaryHeader *hdr)
{
    if (hdr == NULL) {
        return 0;
    }
    /* Type bit is bit 4 of byte 0 (bit 12 of word 0) */
    return (hdr->raw[0] & 0x10u) ? 1 : 0;
}
