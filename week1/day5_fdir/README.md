# Día 5: FDIR — Detección, Aislamiento y Recuperación de Fallos

## Descripción general

FDIR es una disciplina nacida en la ingeniería de software para naves espaciales. Cuando un
satélite se encuentra en órbita a cientos de kilómetros sobre la Tierra, ningún ingeniero puede
alcanzarlo para reiniciarlo. El software debe detectar los problemas, contener su radio de impacto
y devolver el sistema a un estado de salud correcto, todo ello sin intervención humana. Hoy
construimos los patrones Rust que hacen esto posible.

Este es el día de teoría final del módulo. Todos los patrones presentados aquí aparecerán en el
proyecto de la semana 1.

---

## Prerrequisitos

- Día 1: Async con Tokio / tareas
- Día 2: I/O Unix y sockets
- Día 3: FFI (no se usa directamente, pero proporciona contexto sobre los límites de código inseguro)
- Día 4: IPC, D-Bus, sockets Unix

---

## Los Tres Pasos: Detectar → Aislar → Recuperar

### 1. Detección de fallos

No se puede reparar lo que no se puede ver. Los mecanismos de detección incluyen:

- **Watchdogs**: Una tarea debe demostrar que está viva "pateando" periódicamente un temporizador.
  Si el temporizador expira, el watchdog se activa. Esto detecta bloqueos mutuos (deadlocks),
  bucles infinitos y agotamiento de recursos — errores que no hacen caer el programa pero lo
  dejan inútil.
- **Monitorización de salud**: Cada componente informa de su estado. Una tabla de salud central
  acumula esta información y proporciona una vista a nivel de sistema.
- **Comprobaciones de cordura**: Validación de rangos, verificación de checksums, pruebas de
  plausibilidad. Una lectura de sensor de temperatura de -500°C en una nave espacial en órbita
  es casi con certeza un fallo del sensor, no un avance en física.
- **Latidos perdidos**: Similar a los watchdogs pero a nivel de comunicación. Si un subsistema
  deja de enviar telemetría, asumir que ha fallado.

### 2. Aislamiento de fallos

Una vez detectado un fallo, hay que evitar que se propague:

- **Disyuntores (circuit breakers)**: Evitar martillar un componente que está fallando. Si un
  demonio de sensor devuelve errores en cada llamada, dejar de llamarlo durante un tiempo y
  dejarle recuperarse. Tomado de los sistemas distribuidos (Netflix Hystrix, etc.) pero igualmente
  relevante en demonios embebidos.
- **Sandboxing**: Ejecutar operaciones arriesgadas en procesos o tareas separados para que su
  fallo no corrompa el estado global.
- **Bloqueo de modo**: Cuando algo falla, restringir lo que el sistema puede hacer. No se puede
  disparar un propulsor accidentalmente si se está en un modo que no lo permite. El patrón
  typestate hace cumplir esto en tiempo de compilación.

### 3. Recuperación

La recuperación sigue una **cadena de escalada**: probar primero la opción más barata; escalar
solo si falla.

```
Fallo detectado
    ↓
Intentar reinicio / reintento (barato)
    ↓ (sigue fallando)
Cambiar al componente redundante
    ↓ (sin redundancia o la redundancia también falló)
Entrar en modo seguro (configuración de mínimo riesgo)
    ↓ (no se puede recuperar de forma autónoma)
Esperar comando desde tierra / modo de emergencia
```

**Ejemplo espacial — fallo del rastreador de estrellas:**
1. El rastreador de estrellas devuelve un cuaternión inválido.
2. Marcar el componente como Degradado. Intentar reinicio suave (enviar comando de reinicio por I2C).
3. Si sigue fallando tras 3 reinicios, cambiar al rastreador de estrellas redundante.
4. Si ambos rastreadores de estrellas fallaron, cambiar el control de actitud a magnetómetros
   únicamente (menor precisión pero funcional). Entrar en modo Degradado.
5. Si los magnetómetros también son poco fiables (p. ej., cerca de los polos magnéticos), entrar
   en Modo Seguro: orientar los paneles solares hacia el Sol, detener todas las operaciones no
   esenciales, enviar telemetría de baliza a un ritmo lento, esperar contacto con tierra.
6. Si incluso eso falla: modo de Emergencia. Activar calefactores de supervivencia, transmitir
   baliza de emergencia, esperar intervención desde tierra.

---

## Patrones de Watchdog

### Watchdog por hardware

Todos los microcontroladores modernos (STM32, NXP LPC, etc.) tienen un temporizador watchdog por
hardware (IWDG/WWDG). Si no se le "patea" dentro de una ventana configurable, el MCU se reinicia.
Esta es la última línea de defensa — una garantía a nivel de metal puro.

En computadoras de vuelo basadas en Linux (Raspberry Pi CM4, Jetson, SBCs personalizadas), el
kernel expone `/dev/watchdog`. Escribir cualquier byte en él patea el watchdog por hardware. Si
el proceso muere sin cerrar el fd (SO_KEEPALIVE está desactivado), el hardware se reinicia.

```bash
# Abrir /dev/watchdog y mantenerlo vivo — cerrarlo sin escribir reinicia el sistema
echo 1 > /dev/watchdog   # patear
echo V > /dev/watchdog   # cierre mágico: desarmar antes de apagado limpio
```

### Demonio watchdog por software

Un watchdog por software monitoriza múltiples tareas dentro de un único proceso. Cada tarea
recibe un `WatchdogToken`; llamar a `.kick()` reinicia su temporizador. Una tarea monitora en
segundo plano verifica todos los tokens. Esto detecta bloqueos por tarea sin necesidad de
reiniciar el proceso completo.

Este es el patrón en `examples/01_watchdog.rs`.

---

## Disyuntor (Circuit Breaker)

Originado en sistemas distribuidos (Fowler 2014). Funciona como un disyuntor físico: se activa
cuando ocurren demasiados fallos, evitando daños adicionales hasta que el fallo se resuelve.

```
         Contador de fallos < umbral
         ┌─────────────────────────┐
         │                         │
    ┌────▼─────┐   umbral       ┌──┴──────┐
    │  CERRADO │──────────────►│  ABIERTO│
    │ (normal) │               │(fallando)│
    └──────────┘               └────┬────┘
         ▲                          │
         │    sondeo exitoso        │ tiempo de espera transcurrido
         │   ┌───────────┐          │
         └───│SEMI-ABIERTO│◄─────────┘
             │ (probando) │
             └───────────┘
                   │ sondeo falla
                   │────────────►vuelve a ABIERTO
```

Estados:
- **Cerrado**: Operación normal. Los fallos incrementan el contador. Contador ≥ umbral → Abierto.
- **Abierto**: Todas las llamadas fallan inmediatamente (fallo rápido). Tras el tiempo de recuperación → Semi-Abierto.
- **Semi-Abierto**: Se permite pasar una llamada de sondeo. Éxito → Cerrado. Fallo → Abierto.

Ver `examples/02_circuit_breaker.rs`.

---

## Árboles de Supervisores (Influencia de Erlang)

Erlang introdujo la filosofía del "dejar que se caiga": en lugar de codificar defensivamente para
cada error posible, dejar que los procesos fallen y que un supervisor los reinicie. El supervisor
conoce la política de reinicio (uno-por-uno, todos-por-uno, resto-por-uno).

En Rust async, adaptamos esto: las tareas son baratas, los pánicos o retornos `Err` indican fallo
fatal, los supervisores reinician con retroceso exponencial.

```
Supervisor
├── SensorTask     (reinicio: siempre, máx: 5, retroceso: 2^n * 100ms)
├── WatchdogTask   (reinicio: siempre, máx: ilimitado)
└── TelemetryTask  (reinicio: siempre, máx: 3, retroceso: 2^n * 500ms)
```

Parámetros clave:
- **max_restarts**: Prevenir bucles de reinicio infinitos (protección contra crash-loop).
- **backoff**: No ciclar repetidamente sobre un recurso roto; darle tiempo para recuperarse.
- **política de reinicio**: uno-por-uno (reiniciar solo la tarea que falló) suele ser correcto
  para tareas independientes; todos-por-uno es para conjuntos de tareas fuertemente acopladas.

Ver `examples/04_supervisor.rs`.

---

## Patrón Typestate para Gestión de Modos

En el software de naves espaciales, no todas las operaciones son válidas en todos los modos. No
se debe intentar una maniobra orbital en Modo Seguro. No se deben transmitir datos científicos
cuando el enlace no está establecido.

El **patrón typestate** codifica estas restricciones en el sistema de tipos de Rust. En lugar de
una comprobación en tiempo de ejecución `if mode == SafeMode { return Err(...) }`, el compilador
rechaza las operaciones ilegales en tiempo de compilación. No hay coste en tiempo de ejecución.

```rust
struct OBC<Mode> { _mode: PhantomData<Mode>, ... }

// Solo invocable cuando Mode = Nominal
impl OBC<Nominal> {
    fn fire_thruster(&self) { ... }
}

// Solo invocable cuando Mode = SafeMode
impl OBC<SafeMode> {
    fn run_diagnostics(&self) { ... }
}

// Una función que recibe OBC<SafeMode> no puede llamar a fire_thruster —
// simplemente no existe en ese tipo. Error de compilación, no error en ejecución.
```

Ver `examples/05_safe_state.rs`.

---

## FDIR vs Manejo de Errores

Estos son **diferentes niveles de abstracción** y frecuentemente se confunden:

| | Manejo de Errores | FDIR |
|---|---|---|
| **Nivel** | Función/módulo | Sistema |
| **Objetivo** | Propagar y manejar errores con elegancia | Mantener el sistema vivo y seguro |
| **Mecanismo** | `Result<T, E>`, `?`, `match` | Watchdogs, disyuntores, supervisores, tablas de salud |
| **Alcance** | Una operación | Subsistema completo o misión |
| **Horizonte temporal** | Milisegundos | Segundos a horas |
| **Actor** | El código que llama | Un sistema de monitorización autónomo |

Ejemplo: Una lectura de sensor devuelve `Err(ChecksumMismatch)`. El manejo de errores dice
"reintentar una vez, luego devolver Err por la pila de llamadas." FDIR dice "este sensor ha
devuelto errores 10 veces en los últimos 30 segundos; marcarlo como Degradado, cambiar al
respaldo y notificar a la tabla de salud."

---

## Requisitos FDIR de ECSS-E-ST-70-11C (Resumen)

La Cooperación Europea para la Normalización Espacial define requisitos para el software de a bordo.
Cláusulas clave relevantes para FDIR:

- **FDIR-1**: El OBSW deberá implementar un FDIR jerárquico con al menos tres niveles.
- **FDIR-2**: Cada nivel deberá tener una ruta de escalada definida.
- **FDIR-3**: El modo seguro deberá deshabilitar todas las funciones no críticas y minimizar el consumo de energía.
- **FDIR-4**: Todos los eventos de detección de fallos deberán ser registrados con marca de tiempo y contexto.
- **FDIR-5**: Las acciones de recuperación deberán ser idempotentes (seguras de repetir).
- **FDIR-6**: El sistema deberá entrar en modo seguro de forma autónoma dentro de un tiempo definido
  tras la pérdida de contacto con tierra.

Estos requisitos motivan directamente nuestros patrones: la tabla de salud satisface FDIR-4,
el modo seguro typestate satisface FDIR-3, el supervisor satisface FDIR-2.

---

## El Concepto de Estado Seguro

Un **estado seguro** es una configuración del sistema bien definida y de mínimo riesgo. El sistema
se retira a él cuando no sabe qué más hacer de forma segura.

Propiedades de un buen estado seguro:
1. **Alcanzable desde cualquier otro estado**: Siempre se puede llegar a él.
2. **Estable**: Una vez en estado seguro, se permanece allí hasta que se recibe un comando explícito para salir.
3. **Presupuesto de energía conocido**: Paneles solares desplegados, calefactores encendidos, no esenciales apagados.
4. **Comunicación preservada**: Se pueden seguir recibiendo comandos desde tierra.
5. **Ninguna acción peligrosa posible**: Propulsores deshabilitados, pirotécnicos asegurados.
6. **Telemetría continua**: Se puede ver qué falla desde tierra.

En nuestro código, `OBC<SafeMode>` hace cumplir las propiedades 4 y 5 en tiempo de compilación.

---

## Ejemplos

| Archivo | Concepto | Comando de ejecución |
|------|---------|-------------|
| `01_watchdog.rs` | Watchdog por software, temporizadores por tarea | `cargo run --example 01_watchdog` |
| `02_circuit_breaker.rs` | Máquina de estados del disyuntor | `cargo run --example 02_circuit_breaker` |
| `03_health_table.rs` | Tabla de salud compartida, acceso concurrente | `cargo run --example 03_health_table` |
| `04_supervisor.rs` | Supervisión de tareas con retroceso | `cargo run --example 04_supervisor` |
| `05_safe_state.rs` | Gestión de modos con typestate | `cargo run --example 05_safe_state` |

## Ejercicios

| Archivo | Tarea |
|------|------|
| `ex1_fdir_chain.rs` | Conectar todos los patrones alrededor de un demonio ADC |
| `ex1_fdir_chain_sol.rs` | Solución de referencia |
