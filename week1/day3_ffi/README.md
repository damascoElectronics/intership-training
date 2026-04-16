# Día 3 — C/Rust FFI: Llamar a Controladores C Heredados desde Rust

## Por qué esto importa en el software de naves espaciales

Las computadoras a bordo (OBCs) de naves espaciales acumulan décadas de controladores en C. El controlador UART para
tu interfaz CCSDS fue escrito en 2003. La biblioteca del controlador SRAM vino del proveedor del ASIC
como un archivo `.a` compilado con una cabecera. No los vas a reescribir — el solo
costo de calificación sería prohibitivo.

La Interfaz de Funciones Extranjeras (FFI) de Rust te permite llamar ese código C directamente, con cero
sobrecarga, mientras construyes una API Rust segura encima. Este es el patrón que te permite escribir
nuevo software de naves espaciales en Rust sin abandonar el ecosistema C existente.

---

## ABI vs API — por qué la distinción importa en FFI

**API** (Interfaz de Programación de Aplicaciones) es lo que ves en el código fuente: firmas de funciones,
nombres de tipos, valores de constantes. Es un contrato expresado en texto.

**ABI** (Interfaz Binaria de Aplicación) es el contrato a nivel de máquina: cómo se pasan los argumentos
(registros vs pila), cómo se distribuyen las estructuras en memoria (orden de campos, relleno, tamaño),
qué convención de llamada se usa, cómo regresan los valores de retorno. Es un contrato expresado en
bits.

Cuando llamas a una función C desde Rust, el compilador no tiene el código fuente C. Solo tiene
el contrato ABI. Si Rust distribuye una estructura diferente a C — por ejemplo, añadiendo bytes de relleno
donde C no espera ninguno — corromperás datos en el límite. Sin error del compilador. Solo bytes incorrectos.

**Por eso cada tipo que compartes a través del límite FFI debe usar `#[repr(C)]`.**

Sin `#[repr(C)]`, Rust es libre de reordenar campos para eficiencia de caché y añadir o eliminar
relleno según le convenga. Con `#[repr(C)]`, Rust usa las mismas reglas de distribución que C: campos en
orden de declaración, alineación natural, relleno estándar. La distribución binaria coincide exactamente.

---

## La convención de llamada C

En x86-64 Linux (el ABI System V AMD64, también usado en la mayoría de objetivos Linux embebidos):

- Los argumentos enteros/puntero van en RDI, RSI, RDX, RCX, R8, R9; el exceso va en la pila
- Los argumentos de punto flotante van en XMM0–XMM7
- El valor de retorno va en RAX (o XMM0 para float)
- El destinatario debe preservar RBX, RBP, R12–R15 (registros guardados por el destinatario)

En ARM Cortex-A (AAPCS64):
- Los argumentos enteros/puntero van en X0–X7
- El destinatario preserva X19–X28

Para objetivos embebidos sin sistema operativo (Cortex-M, RISC-V sin Linux), la convención de llamada
es más simple pero igualmente estandarizada. Rust respeta `extern "C"` con el significado de "usar el
ABI C de la plataforma", por lo que esto es automático — solo necesitas declararlo.

---

## `#[repr(C)]`: disciplina de distribución en el límite

```rust
// SIN #[repr(C)]: Rust puede reordenar campos y añadir relleno de forma impredecible.
struct Malo {
    bandera: u8,   // Rust podría colocar esto en el desplazamiento 4 para agruparlo con el u32
    valor: u32,
}

// CON #[repr(C)]: distribución compatible con C.
// bandera en desplazamiento 0, 3 bytes de relleno, valor en desplazamiento 4 — igual que una struct C.
#[repr(C)]
struct Bueno {
    bandera: u8,
    valor: u32,
}
```

Reglas para `#[repr(C)]`:
- Los campos aparecen en orden de declaración
- Cada campo está alineado a su propio requisito de alineación
- El tamaño de la estructura se rellena a un múltiplo de la alineación del campo más grande
- Esto es exactamente lo que `sizeof` y `offsetof` devuelven en C

También disponible:
- `#[repr(packed)]`: elimina el relleno (peligroso; puede causar fallos de acceso no alineado en algunas CPUs)
- `#[repr(u8)]` etc. en enumeraciones: controla el tamaño del discriminante

---

## Propiedad en el límite FFI

El sistema de propiedad de Rust no cruza el límite FFI. Cuando pasas un puntero a C:

```
Lado Rust                   | Lado C
----------------------------|----------------------------------
Posee la asignación         | Recibe un puntero crudo
¡Debe mantenerlo vivo!      | No sabe nada del tiempo de vida de Rust
```

**Reglas clave:**

1. **Tú mantienes la propiedad.** Si pasas `data.as_ptr()` a una función C, Rust sigue siendo
   propietario de `data`. Debes asegurarte de que `data` viva al menos mientras C esté usando el puntero.

2. **Memoria asignada por C.** Si C asigna memoria y devuelve un puntero, no puedes liberarla
   con el asignador de Rust. Debes llamar a la función C `free()` (o la función de liberación propia
   de la biblioteca) para liberarla.

3. **Peligro de tiempo de vida temporal.** Este es el error FFI más común en Rust:
   ```rust
   // INCORRECTO: CString se descarta en el punto y coma, el puntero queda colgante
   let ptr = CString::new("hola").unwrap().as_ptr();
   c_function(ptr); // ¡uso después de liberar!

   // CORRECTO: vincula el CString a una variable con nombre
   let cstr = CString::new("hola").unwrap();
   c_function(cstr.as_ptr()); // cstr vive aquí
   ```

4. **Punteros nulos.** Las funciones C frecuentemente devuelven NULL en caso de error. Debes verificar NULL
   antes de desreferenciar. `Option<NonNull<T>>` o `Option<extern "C" fn()>` de Rust modelan
   esto explícitamente.

---

## Dos direcciones de FFI

### Dirección 1: Llamar C desde Rust (bloques `extern "C"`)

```rust
// Declara la firma de la función C — Rust confía en que la indiques correctamente.
extern "C" {
    fn ccsds_pack(
        hdr: *mut CcsdsPrimaryHeader,
        apid: u16,
        seq_count: u16,
        data_len: u16,
        is_tc: i32,
    ) -> i32;
}

// Llámala en un bloque unsafe — estás afirmando: "he verificado las invariantes."
unsafe {
    let ret = ccsds_pack(&mut header, 0x100, 1, 4, 1);
    assert_eq!(ret, 0);
}
```

El bloque `extern "C"` es una declaración, no una definición. Le estás diciendo a Rust "confía en mí,
esta función existe en una biblioteca enlazada con esta firma."

### Dirección 2: Llamar Rust desde C (`#[no_mangle] extern "C"`)

```rust
/// # Safety
/// `output` debe apuntar a un buffer de al menos 6 bytes escribibles.
#[no_mangle]
pub unsafe extern "C" fn rust_validate_header(raw: *const u8, len: usize) -> i32 {
    if raw.is_null() { return -1; }
    if len < 6 { return -2; }
    // ... lógica de validación
    0
}
```

`#[no_mangle]` le dice al compilador de Rust que no aplique la codificación de nombres (que convertiría
`rust_validate_header` en algo como `_ZN7mypackage20rust_validate_header17h3a...`).
Sin él, C no puede encontrar la función por su nombre declarado.

`extern "C"` en una función Rust le dice a Rust que use la convención de llamada C para esa
función (por defecto Rust usa su propio ABI interno inestable).

---

## Integración con el sistema de construcción: el crate `cc` y `build.rs`

El sistema de construcción de Cargo ejecuta `build.rs` antes de la compilación. El crate `cc` dentro de `build.rs`
compila archivos fuente C usando el compilador C de la plataforma con los indicadores correctos:

```rust
// build.rs
fn main() {
    cc::Build::new()
        .file("c_libs/ccsds_framer.c")
        .flag("-Wall")           // opcional: advertencias adicionales
        .compile("ccsds_framer"); // produce libccsds_framer.a
    // cc emite automáticamente "cargo:rustc-link-lib=static=ccsds_framer"
    // Cargo lo enlaza en tu binario.
}
```

Para bibliotecas precompiladas (archivos `.a` del proveedor):
```rust
println!("cargo:rustc-link-search=native=/ruta/al/vendor/lib");
println!("cargo:rustc-link-lib=static=vendor_driver");
```

Para bibliotecas dinámicas del sistema:
```rust
println!("cargo:rustc-link-lib=dylib=pthread");
```

---

## Contratos de seguridad en bloques `unsafe`

`unsafe` en Rust no significa "este código es peligroso." Significa "yo, el programador, estoy
asumiendo la responsabilidad de mantener invariantes que el compilador no puede verificar automáticamente."

Cada bloque `unsafe` debe tener un comentario que explique exactamente qué contrato estás
afirmando. Esto no es solo estilo — es cómo los revisores de código (y tú en el futuro) saben qué
verificar cuando algo sale mal.

```rust
// Safety: `hdr` fue inicializado por ccsds_pack arriba y no ha sido movido.
// El puntero es válido, no nulo y correctamente alineado para CcsdsPrimaryHeader.
let raw = unsafe { &(*hdr_ptr).raw };
```

¿Qué invariantes eres TÚ responsable de mantener en los bloques unsafe de FFI?

1. **Validez del puntero**: el puntero no es nulo, está alineado y apunta a un objeto válido
   del tipo declarado
2. **Tiempo de vida**: el objeto apuntado sobrevive a la duración de la llamada C
3. **Acceso exclusivo**: si C muta a través del puntero, ningún otro código Rust mantiene una
   referencia al mismo dato
4. **Inicialización**: el dato apuntado está completamente inicializado antes de pasarlo a C
5. **Seguridad de hilos**: si C no es seguro para hilos, debes sincronizar del lado de Rust

---

## El modelo mental: el límite es un contrato, no un muro

Piensa en el bloque `extern "C"` como una promesa: "sé que esta función existe con esta
firma y este ABI C." El compilador de Rust te creerá completamente. Si mientes —
tipos de argumentos incorrectos, número equivocado de argumentos, convención de llamada incorrecta — obtendrás
corrupción silenciosa de memoria o un fallo en tiempo de ejecución. Sin error de tipo en tiempo de compilación.

Por eso el patrón de envoltura segura (Día 3, Ejemplo 02) es tan importante: aisla las
llamadas `unsafe` en una capa pequeña y cuidadosamente revisada, luego expone una API pública segura que
impone todas las precondiciones a nivel de tipos.

```
[ Biblioteca C: ccsds_framer.a ]
         ↑
[ Declaraciones FFI unsafe ]     ← pequeña; debe ser correcta
         ↑
[ Envoltura Rust segura ]        ← valida entradas, mapea errores
         ↑
[ resto de tu aplicación ]       ← 100% Rust seguro
```
