# Matriz de Decisión IPC

| Mecanismo | Latencia | Rendimiento | Fiabilidad | Direccionalidad | Complejidad | Ideal Para |
|-----------|----------|-------------|------------|-----------------|-------------|------------|
| **Unix socket (SOCK_STREAM)** | ~1µs | Alto | Fiable (ordenado, sin pérdidas) | Bidireccional | Baja | IPC general entre demonios; petición/respuesta |
| **Unix socket (SOCK_DGRAM)** | ~0.5µs | Alto | Puede descartar si el buffer está lleno | Unidireccional | Baja | Mensajes de log; telemetría de mejor esfuerzo |
| **Named pipe (FIFO)** | ~1µs | Medio | Fiable | Unidireccional | Muy baja | Flujos de datos simples; canalizaciones de log |
| **POSIX Message Queue** | ~2µs | Medio | Fiable, **ordenado por prioridad** | Unidireccional | Media | Carriles de prioridad para TC; encolado de comandos |
| **Shared memory** | ~100ns | Muy alto | Sin garantía de entrega; requiere sincronización | Ambas | Alta | Datos de sensores de alta frecuencia; buffers grandes |
| **D-Bus** | ~100µs | Bajo | Fiable | Ambas | Alta | Descubrimiento de servicios; consultas de estado |

---

## Orientación Específica para Naves Espaciales

### Usar Unix domain sockets cuando:
- Dos demonios necesitan petición/respuesta bidireccional
- Se necesita delimitación de mensajes (usar `LengthDelimitedCodec`)
- Se desea control de acceso mediante permisos del sistema de archivos
- **Ejemplos**: tc_receiver → router, router → hk_service

### Usar POSIX MQ cuando:
- Se necesita ordenación por PRIORIDAD (los TCs tienen diferentes urgencias)
- Hay un único productor y un único consumidor
- Un buffer acotado es aceptable (mq descarta cuando está lleno)
- **Ejemplos**: carriles de prioridad para TC, encolado de eventos de fallo

### Usar shared memory cuando:
- Se necesita compartir buffers de datos grandes (p. ej., datos de imagen, flujos de sensores en bruto)
- La latencia ultrabaja es crítica (microsegundos frente a milisegundos)
- Se puede gestionar la sincronización con cuidado (mutex, semáforo, RCU)
- **Ejemplos**: procesador de imágenes alimentando al compresor; buffer circular de telemetría

### Usar D-Bus cuando:
- Se necesita descubrimiento de servicios ("¿está disponible el subsistema de comunicaciones?")
- Se desea introspección (¿qué métodos expone este servicio?)
- La latencia no importa (las consultas de estado no son críticas en tiempo)
- **Ejemplos**: componentes del OBC registrando su estado de salud

### Evitar para naves espaciales:
- **TCP/IP** sobre loopback: funciona pero es más lento que UDS, sin ventaja clara
- **SysV IPC** (msgget/shmget): API más antigua, usar equivalentes POSIX en su lugar

---

## Recordatorio sobre Delimitación de Mensajes

Los Unix sockets y los FIFOs son **flujos de bytes** — no tienen límites de mensaje.
Es OBLIGATORIO agregar delimitación. Enfoques habituales:

| Método | Biblioteca | Usar cuando |
|--------|-----------|-------------|
| Prefijo de longitud (longitud en big-endian de 4 bytes + carga útil) | `tokio_util::codec::LengthDelimitedCodec` | Protocolos binarios (la mayoría de IPC en OBC) |
| Delimitador de nueva línea | `tokio::io::BufReader::lines()` | Canales de depuración/log en ASCII |
| COBS + delimitador 0x00 | Manual (ver ejemplos del día 9) | Delimitación en serie embebida |
