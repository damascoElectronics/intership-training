# Día 7 — Telecomando y Telemetría (TC/TM)

**Tema:** Protocolo de Space Packet CCSDS y PUS-C — el lenguaje que hablan las naves espaciales.

Cada byte que viaja entre una estación terrestre y una computadora a bordo sigue un estándar definido.
Este día construye una implementación funcional de los formatos de paquetes que encontrarás
en la entrevista y en el trabajo.

---

## Objetivos de Aprendizaje

- Implementar la cabecera primaria CCSDS de 6 bytes con manipulación correcta de bits
- Calcular y verificar CRC-CCITT (polinomio 0x1021, inicio 0xFFFF)
- Construir estructuras de paquetes TC (telecomando) y TM (telemetría) PUS-C
- Enrutar paquetes por APID a manejadores por servicio
- Detectar brechas en el contador de secuencia y el desbordamiento de 14 bits en 0x3FFF

---

## Estructura del Paquete (diagrama ASCII)

```
Cabecera Primaria CCSDS (6 bytes, obligatoria para todos los Space Packets):

 Bit: 0         1         2         3         4         5
      0123456789012345678901234567890123456789012345678901234567
      │ Versión │T│SH│   APID (11 bits)   │SF│   Conteo Sec.  │   Long. Datos Pkt  │
      │  (3b)   │C│  │                    │(2│   (14 bits)    │   (16 bits)        │
      └─────────┴─┴──┴────────────────────┴──┴────────────────┴────────────────────┘

T  = Tipo de Paquete: 0=TM, 1=TC
SH = Indicador de Cabecera Secundaria: 1=presente
SF = Indicadores de Secuencia: 11=independiente, 01=primero, 00=continuación, 10=último
Long. Datos Pkt = (longitud_total_paquete - 7), es decir, bytes después de la cabecera primaria menos 1
```

---

## La Biblioteca `spacepacket`

Todos los tipos de paquetes están implementados en el sub-crate `spacepacket`:

```
spacepacket/src/
├── lib.rs              re-exportaciones + docs del módulo
├── primary_header.rs   CcsdsPrimaryHeader, PacketType, SeqFlags
├── crc.rs              crc_ccitt(), append_crc(), verify_and_strip_crc()
├── pus_tc.rs           PusTelecommand (cabecera secundaria de 5 bytes)
├── pus_tm.rs           PusTelemetry  (cabecera secundaria de 10 bytes + OBT)
├── apid_router.rs      ApidRouter, route_packet()
└── error.rs            enum PacketError
```

---

## Ejemplos

| Archivo | Lo que demuestra |
|---------|-----------------|
| `01_build_tc.rs` | Construir TC(17,1) ping, imprimir desglose campo por campo |
| `02_parse_tm.rs` | Construir y analizar TM(3,25) housekeeping con datos de sensor falsos |
| `03_route_packets.rs` | Enrutar 12 paquetes por 4 APIDs a hilos por servicio |
| `04_sequence_counter.rs` | Detección de brechas, contadores por APID, desbordamiento de 14 bits en 0x3FFF |

Ejecutar un ejemplo:
```
cargo run -p day7-tctm --example 01_build_tc
```

---

## Ejercicios

### Ejercicio 1 — Servicio HK (`ex1_hk_service.rs`)

Implementa un `HkService` que:

1. Acepta llamadas `register_parameter(id: u16, name: &str)` en el inicio
2. Acepta llamadas `update_parameter(id: u16, value: f32)` de los sensores
3. Produce un `PusTelemetry` (servicio 3, subservicio 25) mediante `build_report(apid, seq)`

El informe codifica todos los parámetros registrados como pares `[id: u16 LE][value: f32 LE]` en el
campo de datos de aplicación.

Solución: `ex1_hk_service_sol.rs`

---

## Conceptos Clave

### Asignación de APID

Los APIDs 0x000–0x7FF son definidos por el usuario por misión. Una asignación típica:

| APID | Servicio |
|------|---------|
| 0x001 | Verificación TC (servicio 1) |
| 0x002 | Housekeeping (servicio 3) |
| 0x003 | Reporte de Eventos (servicio 5) |
| 0x100–0x1FF | Subsistemas de carga útil |
| 0x7FF | Difusión (sin destino específico) |

### CRC-CCITT

El código de detección de errores estándar de PUS-C. Cada paquete TC y TM termina con un CRC de 2 bytes.
El receptor recalcula el CRC sobre los bytes 0..(N-2) y compara con los bytes (N-2)..(N).

Vector de prueba: `crc_ccitt(b"123456789")` == `0x29B1`

### Campo Longitud de Datos del Paquete

Con nombre confuso — es el número de octetos en el Campo de Datos del Paquete *menos uno*,
no la longitud total del paquete. Entonces un campo de datos de 10 bytes → PDL = 9. Paquete total = 6 + PDL + 1.

### OBT (Tiempo a Bordo)

Las cabeceras secundarias de TM PUS-C incluyen una marca de tiempo. El formato estándar es CCSDS CUC:
- 4 bytes de tiempo grueso (segundos enteros desde la época)
- 2 bytes de tiempo fino (sub-segundo, unidades de 2⁻¹⁶ segundos ≈ resolución de 15 µs)
