#ifndef CCSDS_FRAMER_H
#define CCSDS_FRAMER_H

/*
 * CCSDS Space Packet Protocol — Primary Header
 * CCSDS 133.0-B-2 (Blue Book)
 *
 * The primary header is always 6 octets, laid out as follows:
 *
 *  Octet 0-1  : Version(3b) | Type(1b) | Sec.Hdr.Flag(1b) | APID(11b)
 *  Octet 2-3  : Sequence Flags(2b) | Sequence Count(14b)
 *  Octet 4-5  : Packet Data Length (total octets in data field - 1)
 *
 * Field widths and positions:
 *   Version     : bits 15-13 of word 0  (always 0b000)
 *   Type        : bit  12    of word 0  (0=TM, 1=TC)
 *   Sec.Hdr.Flg : bit  11    of word 0  (set to 0 here for simplicity)
 *   APID        : bits 10-0  of word 0  (0x000 – 0x7FF)
 *   Seq.Flags   : bits 15-14 of word 1  (0b11 = standalone packet)
 *   Seq.Count   : bits 13-0  of word 1  (0 – 16383)
 *   Data Length : bits 15-0  of word 2  (actual_data_bytes - 1)
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Raw 6-byte CCSDS primary header. Treat as opaque; use pack/unpack. */
struct CcsdsPrimaryHeader {
    uint8_t raw[6];
};

/*
 * ccsds_pack — fill in a CCSDS primary header.
 *
 * Parameters:
 *   hdr       : pointer to header struct to populate (must not be NULL)
 *   apid      : Application Process Identifier, 11 bits (0 – 2047)
 *   seq_count : packet sequence count, 14 bits (0 – 16383)
 *   data_len  : number of octets in the data field (the field is stored as data_len-1)
 *               must be >= 1
 *   is_tc     : non-zero for Telecommand (type=1), zero for Telemetry (type=0)
 *
 * Returns 0 on success, -1 on error (NULL pointer or out-of-range field).
 */
int ccsds_pack(struct CcsdsPrimaryHeader *hdr,
               uint16_t apid,
               uint16_t seq_count,
               uint16_t data_len,
               int is_tc);

/*
 * ccsds_unpack — extract fields from a CCSDS primary header.
 *
 * Parameters:
 *   hdr       : pointer to header to read (must not be NULL)
 *   apid      : output — receives the 11-bit APID
 *   seq_count : output — receives the 14-bit sequence count
 *   data_len  : output — receives the actual data field length (stored_value + 1)
 *
 * Any output pointer may be NULL if that field is not needed.
 * Returns 0 on success, -1 if hdr is NULL.
 */
int ccsds_unpack(const struct CcsdsPrimaryHeader *hdr,
                 uint16_t *apid,
                 uint16_t *seq_count,
                 uint16_t *data_len);

/*
 * ccsds_is_tc — return non-zero if the header's type bit indicates Telecommand.
 */
int ccsds_is_tc(const struct CcsdsPrimaryHeader *hdr);

#ifdef __cplusplus
}
#endif

#endif /* CCSDS_FRAMER_H */
