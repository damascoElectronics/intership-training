# Proyecto Semana 2 — Pila de Software OBC

**Capstone de la Semana 2.** Una pila de software de computadora de a bordo (OBC) realista que modela la
arquitectura de un sistema de software de vuelo: cinco procesos en comunicación, manejo de paquetes CCSDS/PUS-C,
autenticación HMAC, protección contra repetición, servicio de housekeeping, FDIR y un arnés de prueba
simulador terrestre.

---

## Arquitectura

```
                    ┌─────────────┐
                    │  ground-sim │  (arnés de prueba — envía TCs, lee TMs)
                    └──────┬──────┘
                           │ UDS /tmp/obc_tc_uplink.sock
                           │ Paquetes TC CCSDS firmados con HMAC
                           ▼
                    ┌─────────────┐
                    │ tc-receiver │  verifica HMAC, comprueba ventana de repetición, reenvía al router
                    └──────┬──────┘
                           │ UDS /tmp/obc_router.sock
                           │ bytes TC verificados en bruto (con prefijo de longitud)
                           ▼
                    ┌─────────────┐
                    │  obc-router │  enruta por número de servicio PUS
                    └──────┬──────┘
                 ┌─────────┴──────────┐
                 │ svc=17 (ping)      │ svc=3 (HK)        svc=* (sensores)
                 │ responde en línea  ▼                    ▼
                 │          ┌──────────────┐    ┌───────────────────┐
                 │          │  hk-service  │    │  sensor-daemon    │
                 │          │  stats /proc │    │  máquina FDIR     │
                 │          └──────────────┘    └───────────────────┘
                 │
                 └── Las respuestas TM regresan por sockets UDS separados
```

---

## Ejecución de la Pila

**Opción A — script de conveniencia (recomendado):**
```bash
bash tools/run_obc_stack.sh
```

El script compila el workspace, inicia todos los daemons en segundo plano, ejecuta `ground-sim`,
y limpia al pulsar Ctrl+C.

**Opción B — manual (cuatro terminales):**
```bash
# Terminal 1
cargo run -p tc-receiver

# Terminal 2
cargo run -p obc-router

# Terminal 3
cargo run -p hk-service

# Terminal 4
cargo run -p sensor-daemon

# Terminal 5 (arnés de prueba)
cargo run -p ground-sim
```

---

## Salida Esperada de ground-sim

```
[TEST 1] TC(17,1) ping...
  → Enviados 20 bytes
  ← Recibido TM(17,2) pong    PASS

[TEST 2] TC(3,129) solicitud HK...
  → Enviados 20 bytes
  ← Recibido TM(3,25) informe HK (N bytes)    PASS

[TEST 3] Rechazo de HMAC incorrecto...
  → TC corrupto enviado
  ← Conexión cerrada / sin respuesta    PASS

[TEST 4] Rechazo de ataque de repetición...
  → TC(17,1) reproducido seq=1
  ← Rechazado (secuencia duplicada)    PASS

==========================================
Pruebas pasadas: 4 / 4
```

---

## Mapa de Crates

| Crate | Función |
|-------|---------|
| `obc_core` | Tipos compartidos: `SpacePacket`, `PusService`, `HealthState`, `IpcMessage`, `OBCError` |
| `tc_receiver` | Verificación HMAC-SHA256, protección contra repetición con ventana deslizante, reenvío de TC |
| `obc_router` | Enrutamiento por APID/servicio, pong en línea para TC(17,1) |
| `hk_service` | Lee `/proc/uptime` + `/proc/self/status`, produce TM(3,25) |
| `sensor_daemon` | Sensor sintético, FDIR (3 fallos consecutivos → Degradado) |
| `ground_sim` | Arnés de prueba: construye TCs firmados, valida respuestas TM, reporta PASS/FAIL |

---

## Propiedades de Seguridad Ejercidas

| Propiedad | Dónde se implementa |
|-----------|---------------------|
| Autenticación de paquetes HMAC-SHA256 | `tc_receiver` |
| Comparación MAC en tiempo constante | `tc_receiver` (`subtle::ConstantTimeEq`) |
| Protección contra repetición con ventana deslizante | `tc_receiver` (`ReplayWindow` con máscara de bits `u64`) |
| Sin bytes secretos en los registros | Todos los crates (solo se registra longitud del paquete + APID, nunca el payload) |

---

## Conceptos Integrados

Este proyecto ejercita todos los temas de la Semana 2:
- **Día 6** — rustdoc estructurado en la API pública de `obc_core`
- **Día 7** — análisis de cabeceras CCSDS, construcción de TC/TM PUS-C, verificación CRC
- **Día 8** — HMAC, ventana de repetición, diseño de minimización de privilegios
- **Día 9** — la propiedad de vector de prueba de `crc.rs`, invariante de viaje de ida y vuelta en `ground_sim`

Y lleva adelante las habilidades de la Semana 1:
- Tareas asíncronas con `tokio::select!` y `CancellationToken`
- IPC mediante sockets Unix con enmarcado de prefijo de longitud
- Máquina de estados FDIR en `sensor_daemon`
- Análisis de `/proc` en `hk_service`
