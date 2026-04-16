# Día 2 — I/O en Linux

**Tema:** Lectura de hardware a través de abstracciones de Linux — sysfs, tty, dispositivos de carácter, procfs.

En software embebido para naves espaciales el "sensor" rara vez es un registro que posees; es un
archivo exportado por el kernel o un nodo de dispositivo. Este día te enseña a manejar esas
abstracciones desde async Rust sin bloquear el ejecutor.

---

## Objetivos de Aprendizaje

- Leer y escribir GPIO en sysfs sin ningún crate específico de GPIO (I/O de archivos puro)
- Abrir y entramar un puerto serie con `tokio-serial`
- Emitir llamadas `ioctl` a través de `nix`
- Consultar un valor de sysfs con un temporizador y emitir eventos estructurados
- Parsear archivos `/proc` para telemetría del sistema

---

## Ejemplos

| Archivo | Lo que demuestra |
|------|----------------------|
| `01_sysfs_gpio.rs` | RAII `GpioPin`, exportar/desexportar, dirección, lectura/escritura de valor |
| `02_tty_serial.rs` | `tokio_serial::new()`, velocidad de baudios, paridad, lectura asíncrona línea a línea |
| `03_chardev_ioctl.rs` | `nix::ioctl_read!`, `#[repr(C)]` para la estructura del kernel, ejemplo con RTC |
| `04_sysfs_poll.rs` | Polling basado en intervalos, comparación de umbrales, eventos de alerta tipados |
| `05_procfs_reader.rs` | Parseo de `/proc/uptime`, `/proc/self/status`, `/proc/loadavg`, `/proc/meminfo` |

Ejecuta cualquier ejemplo (requiere un host Linux):
```
cargo run -p day2-linux-io --example 05_procfs_reader
```

---

## Ejercicios

### Ejercicio 1 — Daemon de Eco UART (`ex1_uart_echo.rs`)

Implementa `run_echo_daemon(port: &str, baud: u32)` que:

1. Abre el puerto serie con `tokio-serial`
2. Lee líneas con un `BufReader`
3. Repite cada línea de vuelta con el prefijo `ECHO:`

Solución: `ex1_uart_echo_sol.rs`

### Ejercicio 2 — Monitor Térmico (`ex2_sysfs_poll.rs`)

Implementa `run_monitor(monitor: ThermalMonitor, tx: Sender<ThermalAlert>)` que:

1. Consulta el archivo térmico de sysfs en el intervalo dado
2. Envía `ThermalAlert::Warning` cuando la temperatura supera el umbral de advertencia
3. Envía `ThermalAlert::Critical` cuando supera el umbral crítico

El arnés de pruebas usa archivos temporales en lugar de rutas sysfs reales — no se necesita hardware.

Solución: `ex2_sysfs_poll_sol.rs`

---

## Conceptos Clave

### ¿Por qué sysfs?

El kernel de Linux exporta el estado del hardware como un sistema de archivos virtual bajo `/sys`. Líneas GPIO,
sensores térmicos, fuentes de alimentación, LEDs — todos aparecen como archivos. Esto te permite interactuar con
el hardware usando las mismas APIs de I/O de archivos que ya conoces, sin módulos del kernel ni drivers en C.

### I/O bloqueante en ejecutores async

La regla de oro: **nunca llames un syscall bloqueante desde dentro de una tarea async sin envolverlo
en `tokio::task::spawn_blocking`**. Un hilo bloqueado retiene el hilo del ejecutor, dejando sin
recursos a todas las demás tareas en ese hilo.

Excepción: las lecturas de sysfs son casi instantáneas (el kernel responde de inmediato), por lo que
lecturas cortas con `tokio::fs::read_to_string` son aceptables en la práctica. Evita este atajo
para nodos de dispositivo reales como `/dev/ttyUSBx` donde el kernel puede bloquearse esperando datos.

### Seguridad de ioctl

`nix::ioctl_read!` genera una `unsafe fn`. La obligación de SAFETY es:
- El descriptor de archivo debe referirse al tipo de dispositivo que entiende este número de ioctl.
- El puntero de salida debe apuntar a una estructura válida, correctamente dimensionada y alineada.
- El layout de la estructura debe coincidir exactamente con lo que escribe el kernel (`#[repr(C)]`).
