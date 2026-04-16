# CCSDS Space Packet Protocol — Introducción
**CCSDS 133.0-B-2**

---

## Disposición de Bits de la Cabecera Primaria (6 octetos, siempre presente)

```
Byte 0          Byte 1          Byte 2          Byte 3          Byte 4          Byte 5
 7 6 5 4 3 2 1 0  7 6 5 4 3 2 1 0  7 6 5 4 3 2 1 0  7 6 5 4 3 2 1 0  7 6 5 4 3 2 1 0  7 6 5 4 3 2 1 0
├───────┬─┬─┬───────────────────┤├──┬─────────────────────────────────┤├──────────────────────────────────┤
│ 0 0 0 │T│S│  APID [10:8]     ││FF│    SEQUENCE COUNT [13:0]        ││   DATA LENGTH FIELD [15:0]        │
│ ver=0 │C│H│  (3 bits)        ││  │    (14 bits)                    ││   (value = data_octets - 1)       │
└───────┴─┴─┴───────────────────┘└──┴─────────────────────────────────┘└──────────────────────────────────┘
                                 ←────── APID [7:0] ──────────────────→

Full APID = Byte0[2:0] concatenated with Byte1[7:0] = 11 bits total
```

### Resumen de Campos

| Campo | Bits | Valores | Notas |
|-------|------|---------|-------|
| Version | 3 | Siempre `0b000` | CCSDS versión 1 |
| Packet Type | 1 | `0`=TM, `1`=TC | TM=enlace descendente, TC=enlace ascendente |
| Sec Hdr Flag | 1 | `1` si es PUS | PUS siempre activa este bit |
| APID | 11 | `0x000`–`0x7FE` | `0x7FF` reservado para paquetes de relleno |
| Seq Flags | 2 | `11`=independiente, `01`=primero, `10`=último, `00`=continuación | La mayoría de los paquetes son independientes |
| Seq Count | 14 | `0x0000`–`0x3FFF` | Por APID, reinicia al llegar a 0x3FFF |
| Data Length | 16 | `data_octets - 1` | Mínimo 0 (1 byte de datos) |

---

## Asignación de APID (ejemplo para un OBC pequeño)

```
APID 0x000 — indefinido / no usado
APID 0x001 — Servicio de operaciones a bordo (PUS 17)
APID 0x002 — Subsistema de sensores
APID 0x003 — Servicio de monitoreo (PUS 3)
APID 0x004 — Subsistema de comunicaciones
APID 0x100 — Sistema de control de actitud
APID 0x200 — Gestión de energía
APID 0x300 — Control térmico
...
APID 0x7FF — Paquete IDLE (de relleno) — descartar siempre
```

---

## Indicadores de Secuencia

| Bits | Significado |
|------|-------------|
| `11` | Independiente — mensaje de un solo paquete (el más común) |
| `01` | Primer segmento de un mensaje multi-paquete |
| `10` | Último segmento |
| `00` | Segmento de continuación |

---

## Estructura del Paquete PUS-C (sobre la cabecera primaria)

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│ CCSDS Primary Header (6 B)                                                        │
├──────────────────────────────────────────────────────────────────────────────────┤
│ PUS Secondary Header (variable)                                                   │
│   TC (5 B): [ver+spare (1B)] [service (1B)] [subservice (1B)] [source_id (2B)]   │
│   TM (10B): [ver+spare (1B)] [service (1B)] [subservice (1B)] [dest_id (2B)]     │
│             [OBT coarse (4B)] [OBT fine (2B)]                                    │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Application Data (variable)                                                       │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Packet Error Control — CRC-CCITT (2 B, polynomial 0x1021, init 0xFFFF)           │
└──────────────────────────────────────────────────────────────────────────────────┘
```

---

## Tiempo a Bordo (OBT) — Formato CUC

| Campo | Bytes | Contenido |
|-------|-------|-----------|
| Coarse | 4 | Segundos desde el instante de referencia de la misión (normalmente J2000 o época UNIX) |
| Fine | 2 | Fracciones de segundo (65536 tics/segundo) |

---

## Reglas Clave
1. El **APID** es la "dirección" — enrutar paquetes por APID, no por servicio/subservicio
2. El **contador de secuencia** se incrementa por APID; los saltos indican paquetes perdidos
3. El **CRC** cubre desde la cabecera primaria hasta los datos de aplicación (sin incluir el propio CRC)
4. Los **paquetes de relleno** (APID 0x7FF) son de relleno y deben descartarse silenciosamente
5. El modo **independiente** es correcto para el 99% de los paquetes; la segmentación es poco frecuente

---

## Referencias
- CCSDS 133.0-B-2 (descarga gratuita desde public.ccsds.org)
- ECSS-E-ST-70-41C (servicios PUS-C, requiere membresía o compra en ECSS)
