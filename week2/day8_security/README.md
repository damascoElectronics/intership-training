# Día 8: Seguridad para daemons Linux embebidos

El software de una nave espacial se ejecuta en tarjetas Linux embebidas (por ejemplo, una Raspberry Pi CM4 con
PetaLinux) conectadas a hardware de enlace ascendente RF. El entorno de amenazas es diferente al de
un servidor web corporativo, pero los principios son los mismos: **autenticar todo,
no confiar en nada, darle a cada componente solo los privilegios que necesita**.

---

## Modelo de amenazas

### Suplantación del enlace ascendente (inyección de TC)

Una estación terrestre transmite telecomandos (TCs) por un enlace de radio. Cualquier persona dentro del alcance RF
puede transmitir en la misma frecuencia. Sin autenticación, un atacante puede:

- Inyectar telecomandos falsos (abrir una válvula, deshabilitar un calentador, apagar el OBC)
- Reproducir un comando válido capturado anteriormente en un momento inoportuno
- Interferir el enlace y sustituirlo con sus propios paquetes

**Mitigación**: autenticación HMAC-SHA256 en cada paquete TC. La clave se carga
en el OBC antes del lanzamiento y nunca se transmite. Sin la clave, un atacante
no puede falsificar un MAC válido.

### Amenazas internas / cadena de suministro

El OBC de la nave espacial puede ejecutar bibliotecas de terceros (parsers CCSDS, pilas de protocolo).
Una biblioteca maliciosa o con errores no debería poder:

- Acceder a descriptores de fichero del driver de actuadores que nunca se le proporcionaron
- Realizar syscalls arbitrarias (p. ej., `execve` para abrir una shell)
- Escalar privilegios

**Mitigación**: separación de privilegios (procesos separados por función) + seccomp BPF
(lista de permisos con solo las syscalls que cada proceso necesita legítimamente).

### Corrupción de memoria

Incluso en Rust, el código `unsafe` puede introducir errores de seguridad de memoria. Un desbordamiento de búfer en
una llamada FFI de C o un cast de puntero incorrecto puede darle a un atacante control del flujo.

**Mitigación**: minimizar `unsafe`, escribir comentarios `SAFETY` para cada bloque unsafe,
ejecutar Miri y AddressSanitizer en CI, fuzzear los parsers.

---

## Capas de defensa en profundidad

```
Enlace ascendente (RF)
    │
    ▼
[Receptor RF] ─── bytes crudos ──► [daemon tc_receiver]
                                    │  autenticar (HMAC)
                                    │  verificar repetición (ventana de seq)
                                    │  seccomp: solo syscalls de E/S de red
                                    │  se ejecuta como uid=2001, sin capacidades
                                    ▼
                              Socket Unix (frontera de seguridad)
                                    │
                                    ▼
                              [daemon obc_router]
                                    │  parsear APID
                                    │  enrutar al subsistema
                                    │  seccomp: solo syscalls IPC
                                    ▼
                         [daemons de subsistema] (actuadores, sensores)
```

Si `tc_receiver` es comprometido, solo puede enviar bytes por el socket Unix.
No puede comandar actuadores directamente, leer datos de sensores ni acceder al sistema de ficheros.

---

## Capacidades Linux

### Por qué `setuid root` es peligroso

El modelo Unix tradicional es binario: root (uid=0) puede hacer todo, el resto
está restringido. Un daemon que necesite abrir un socket raw debe ejecutarse como root — y si tiene
un bug, el atacante obtiene una shell root.

### Seguridad basada en capacidades

Linux divide los privilegios de root en ~40 capacidades independientes:

| Capacidad             | Permite                                          |
|-----------------------|--------------------------------------------------|
| `CAP_NET_BIND_SERVICE`| Enlazar a puertos < 1024                        |
| `CAP_SYS_RAWIO`       | Acceder a puertos I/O raw (`/dev/mem`, iopl)    |
| `CAP_NET_RAW`         | Abrir sockets raw (captura/inyección de paquetes)|
| `CAP_SYS_NICE`        | Establecer prioridad de proceso / planificación en tiempo real |
| `CAP_NET_ADMIN`       | Configurar interfaces de red                    |

**Principio de mínimo privilegio**: arrancar como root, adquirir las pocas capacidades
necesarias para la inicialización, y luego **descartar todas las demás de forma permanente**. Aunque el proceso
sea explotado, el atacante solo obtiene las capacidades que se conservaron.

### `PR_SET_NO_NEW_PRIVS`

Tras llamar a `prctl(PR_SET_NO_NEW_PRIVS, 1)`, el proceso y todos sus hijos
nunca podrán obtener nuevos privilegios mediante binarios `setuid` ni capacidades de fichero. Esto es
una puerta de un solo sentido — no se puede deshacer.

### Secuencia de descarte de capacidades

```
1. Arrancar como root (necesario para abrir socket raw / acceder a /dev/spidev)
2. Abrir los recursos privilegiados (socket raw, fichero de dispositivo)
3. Descartar todas las capacidades que no se necesiten
4. setgid(daemon_gid)   ← debe ocurrir ANTES de setuid
5. setuid(daemon_uid)
6. prctl(PR_SET_NO_NEW_PRIVS, 1)
7. Cargar el filtro seccomp BPF
8. Entrar en el bucle principal de eventos
```

---

## Seccomp BPF

Seccomp (modo de cómputo seguro) restringe qué syscalls puede realizar un proceso.
Con reglas BPF (Berkeley Packet Filter) se puede escribir una **lista de permisos**:

```
# Para tc_receiver: solo necesitamos E/S de red
PERMITIR: read, write, recv, recvmsg, sendmsg, accept, close, epoll_wait, futex, exit
DENEGAR TODO (SIGKILL)
```

Si un atacante explota un bug de corrupción de memoria e intenta llamar a `execve` o
`open("/etc/passwd")`, el kernel mata el proceso inmediatamente.

El crate `seccomp` proporciona una API Rust segura. En los ejemplos de este día mostramos el
patrón con comentarios; una implementación completa de seccomp requiere un crate aparte
(bindings de `libseccomp`) que no está en el workspace.

---

## HMAC-SHA256 para autenticación de TC

### Qué proporciona

- **Integridad**: cualquier cambio de bit en el paquete cambia el MAC
- **Autenticación**: solo alguien con la clave puede producir un MAC válido
- **NO confidencialidad**: el contenido del paquete está en texto plano; HMAC no cifra

Para TCs de naves espaciales, la confidencialidad generalmente no es necesaria (los comandos no son
secretos; lo que importa es prevenir la ejecución no autorizada).

### Construcción

```
MAC = HMAC-SHA256(clave, bytes_paquete_excluyendo_campo_mac)
```

El MAC (32 bytes) se añade al final de cada paquete. El receptor:
1. Extrae los últimos 32 bytes (el MAC declarado)
2. Recalcula el HMAC sobre los bytes restantes
3. Compara en tiempo constante

### Gestión de claves

- La clave es un valor aleatorio de 256 bits generado en tierra
- Se carga en el almacenamiento no volátil (NVS) del OBC antes de la integración
- Nunca se transmite por ningún enlace
- Se rota entre misiones (o en órbita si existe un canal seguro de actualización de claves)

---

## Ataques de temporización

Un **ataque de temporización** explota el hecho de que `==` en arrays de bytes realiza cortocircuito:
devuelve `false` en el momento en que encuentra un byte diferente. Midiendo cuánto tiempo tarda
la verificación, un atacante puede deducir cuántos bytes de su suposición son correctos.

```rust
// MAL: comparación sensible al tiempo
if computed_mac == received_mac { ... }

// BIEN: comparación en tiempo constante (crate subtle)
use subtle::ConstantTimeEq;
if computed_mac.ct_eq(&received_mac).into() { ... }
```

`subtle::ConstantTimeEq` siempre examina todos los bytes independientemente de dónde está la primera
diferencia, así que la temporización no revela nada sobre la clave.

En la práctica, los ataques de temporización sobre MACs por red son difíciles debido al jitter,
pero **siempre se debe usar comparación en tiempo constante** — el coste es cero, el riesgo
de no hacerlo no lo es.

---

## Ataques de repetición

Un atacante graba un TC válido y autenticado (p. ej., "abrir válvula de combustible") y lo retransmite
más tarde. El HMAC sigue siendo válido — el atacante no modificó nada.

**Mitigación: número de secuencia con ventana deslizante**

Cada TC lleva un número de secuencia de 16 bits creciente monotónicamente. El receptor
mantiene una ventana de los últimos N números de secuencia vistos:

```
                   ventana (64 bits)
last_seq=100  ──►  bit 0 = seq 100 visto
                   bit 1 = seq 99 visto
                   ...
                   bit 63 = seq 37 visto

Aceptar:  seq 101 (uno adelante — avanzar ventana)
Aceptar:  seq 95  (en ventana, aún no visto — establecer su bit)
Rechazar: seq 100 (en ventana, bit 0 ya establecido — ¡repetición!)
Rechazar: seq 36  (fuera de ventana — demasiado antiguo, asumir repetición)
```

Combinado con HMAC, esto previene tanto la falsificación como la repetición.

---

## Seguridad de memoria en Rust y `unsafe`

### Qué garantiza el Rust seguro

- Sin desreferencias de puntero nulo
- Sin desbordamientos de búfer (los límites se comprueban en tiempo de ejecución o se prueban en compilación)
- Sin uso tras liberación (el borrow checker garantiza que las referencias no sobreviven a sus datos)
- Sin condiciones de carrera (el sistema de tipos impone Send/Sync)
- Sin lecturas de memoria no inicializada (el compilador exige inicialización)

### Qué requiere `unsafe` que TÚ garantices

Dentro de un bloque `unsafe`, Rust desactiva algunas de estas comprobaciones. El programador debe
garantizar manualmente:

1. La aritmética de punteros se mantiene dentro de los límites
2. La memoria apuntada está correctamente inicializada y alineada
3. No existen referencias mutables con alias
4. Los tipos de datos FFI coinciden exactamente con el ABI de C
5. Los invariantes documentados en comentarios `SAFETY` se cumplen

### El proceso de auditoría de unsafe

Para código de seguridad crítica (DO-178C DAL-B y superior), cada bloque `unsafe` necesita:

1. Un comentario `// SAFETY:` que explique por qué la operación unsafe es válida
2. Revisión por un segundo ingeniero
3. Ejecución limpia de Miri (ningún comportamiento indefinido detectado)
4. Prueba fuzz para parsers / deserializadores
5. Cobertura de pruebas ≥ criterios MC/DC (ver Día 9)

El objetivo no es eliminar `unsafe` (a veces imposible con acceso a hardware),
sino **contenerlo** en funciones pequeñas y bien revisadas con invariantes documentados.

---

## Arquitectura de separación de privilegios

El receptor TC es el componente más expuesto a ataques (da al enlace ascendente). Le otorgamos
los **mínimos privilegios posibles**:

```
[tc_receiver]                    [obc_router]
 - uid: 2001 (usuario tc_rx)      - uid: 2002 (usuario router)
 - Sin capacidades                - CAP_SYS_NICE (para planificación RT)
 - seccomp: solo E/S de red       - seccomp: syscalls IPC + timer
 - Puede leer: socket raw         - Puede leer/escribir: socket Unix
 - No puede: abrir ficheros,      - No puede: E/S de red, sockets raw
   hacer fork, execve, mmap exec              
```

El socket Unix entre ellos es la **frontera de seguridad**. El router:
- Valida la estructura del paquete (APID, campos de longitud)
- Limita la tasa por APID
- Enruta al handler del subsistema correcto

Incluso si un atacante encuentra un bug de ejecución remota de código en `tc_receiver`, está
en una sandbox: solo puede enviar bytes al router por el socket Unix, y la
propia validación del router limita el daño que esos bytes pueden causar.

---

## Referencias

- [Página de manual de capacidades Linux](https://man7.org/linux/man-pages/man7/capabilities.7.html)
- [Documentación del kernel de Seccomp BPF](https://www.kernel.org/doc/html/latest/userspace-api/seccomp_filter.html)
- [ECSS-E-ST-70-41C: Protocolos de telecomandos](https://ecss.nl/standard/ecss-e-st-70-41c-space-engineering-space-packet-protocol/)
- NIST SP 800-38B: Recomendaciones HMAC
- [Crate subtle](https://docs.rs/subtle): operaciones criptográficas en tiempo constante
