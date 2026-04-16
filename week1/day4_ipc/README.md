# Día 4 — Comunicación entre Procesos (IPC)

**Tema:** Mover datos entre procesos de forma confiable — la fontanería de un stack de software de OBC real.

Un stack de software de vuelo no es un monolito. El receptor de TC, el servicio de housekeeping, el monitor FDIR
y el gestor de carga útil son procesos separados. Este día enseña los cuatro mecanismos IPC que
realmente usarás, y cuándo elegir cada uno.

---

## Objetivos de Aprendizaje

- Enmarcar mensajes delimitados por longitud sobre Unix Domain Sockets con Tokio + `tokio-util`
- Usar tuberías con nombre (FIFOs) para datos unidireccionales y secuenciales
- Aprovechar las prioridades de las colas de mensajes POSIX para garantizar que los comandos de alta prioridad salten la cola
- Llamar a servicios D-Bus con `zbus` para IPC estructurado y tipado

---

## Ejemplos

| Archivo | Lo que demuestra |
|---------|-----------------|
| `01_uds_server.rs` | `UnixListener`, `Framed<LengthDelimitedCodec>`, Request/Response con bincode |
| `02_uds_client.rs` | `UnixStream::connect`, codec equivalente, bucle async de envío/recepción |
| `03_named_pipe.rs` | `nix::unistd::mkfifo`, dos tareas compartiendo una ruta FIFO |
| `04_posix_mq.rs` | `nix::mqueue`, envío con prioridad, demostrar que PRIO_HIGH llega primero |
| `05_dbus_intro.rs` | `zbus::interface`, `zbus::proxy`, servicio de salud en el bus de sesión |

Ejecutar el par UDS (dos terminales):
```
# Terminal 1
cargo run -p day4-ipc --example 01_uds_server

# Terminal 2
cargo run -p day4-ipc --example 02_uds_client
```

---

## Ejercicios

### Ejercicio 1 — Bus TM (`ex1_tm_bus.rs`)

Implementa `TmBus`, un enrutador de publicación/suscripción para tramas de telemetría:

```rust
pub struct TmBus { /* ... */ }

impl TmBus {
    pub fn new() -> Self;
    pub fn subscribe(&mut self, apid: u16) -> Receiver<TmFrame>;
    pub fn publish(&self, frame: TmFrame) -> Result<(), BusError>;
}
```

- Cada APID obtiene su propio canal
- `publish` distribuye a todos los suscriptores de ese APID
- `subscribe` sobre un APID ya registrado devuelve un segundo receptor (broadcast)

Solución: `ex1_tm_bus_sol.rs`

---

## Matriz de Decisión IPC

| Mecanismo | Latencia | Rendimiento | Ordenamiento | Persistencia | Ideal para |
|-----------|----------|-------------|--------------|--------------|------------|
| UDS stream | ~1 µs | Alto | FIFO | Ninguna | Comando/respuesta bidireccional |
| UDS datagram | ~1 µs | Alto | Ninguno | Ninguna | Eventos fire-and-forget |
| Tubería con nombre | ~5 µs | Medio | FIFO | Ninguna | Flujos de bytes unidireccionales |
| POSIX MQ | ~5 µs | Medio | Prioridad | Kernel | Comandos ordenados por prioridad |
| D-Bus | ~50 µs | Bajo | Por método | Ninguna | Llamadas a servicios tipados, introspección |

---

## Conceptos Clave

### Enmarcado delimitado por longitud

Los streams TCP y UDS crudos son streams de bytes — no hay límites de mensajes. `LengthDelimitedCodec`
antepone una longitud de 4 bytes en big-endian a cada mensaje, de modo que el receptor sabe exactamente cuántos bytes
leer antes de llamar al deserializador.

```
[len: u32 BE][bytes del payload...]
```

Combinar con `bincode` para serialización binaria compacta, o `serde_json` cuando importa la legibilidad humana.

### Prioridades de POSIX MQ

`mq_send` acepta una prioridad (0–31 en Linux). `mq_receive` siempre devuelve el mensaje más antiguo
con la *mayor* prioridad, independientemente del orden de envío. Esto es exactamente lo que se necesita para una cola
de uplink TC: los paquetes EMERGENCY_STOP tienen prioridad sobre las subidas de parámetros rutinarias.

### ¿Por qué no memoria compartida?

La memoria compartida es la más rápida (cero copia) pero requiere sincronización explícita — mutexes, semáforos,
o estructuras libres de bloqueo. No tiene límites de mensajes inherentes, no tiene recepción bloqueante, y
tiene errores de ordenamiento de memoria fáciles de pasar por alto. Resérvala para rutas verdaderamente críticas
en latencia y alto ancho de banda (ej., datos de sensor en streaming a 10 MHz). Para distribución de comandos/respuesta
y telemetría, UDS o POSIX MQ son más seguros y simples.
