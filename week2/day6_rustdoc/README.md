# Día 6: Rustdoc y Documentación de API

## Por qué la Documentación es Importante en Aeroespacial

En software de consumo, la mala documentación es inconveniente. En aeroespacial, puede ser crítica para la misión:

- **Tu API es un contrato.** El equipo de control de actitud que escribe código contra tu codec de paquetes no tiene idea de qué suposiciones hiciste, a menos que las escribas. Una suposición incorrecta significa un paquete perdido — o una nave espacial perdida.
- **La documentación ES parte del producto.** Los estándares de software de ESA, NASA y JPL (por ejemplo, JPL C Coding Standard, ECSS-E-ST-40C) todos requieren documentación formal de interfaces. El rustdoc de Rust hace esto ejecutable y verificable.
- **El revisor no eres tú.** Las revisiones de software de vuelo involucran múltiples equipos, juntas de seguridad y a veces reguladores. Ellos leerán tu documentación, no tu código fuente.
- **La latencia de revisión de código es alta.** En un proyecto de nave espacial, una pregunta como "¿qué pasa si seq_count se desborda?" puede tardar días en responderse a través de canales formales. Una buena documentación previene la pregunta.

La regla: **si un elemento público no tiene comentario de documentación, no existe en lo que respecta a los demás equipos.**

---

## Arquitectura de Rustdoc

Rustdoc es la herramienta oficial de documentación de Rust, integrada en la cadena de herramientas de Rust. Funciona de la siguiente manera:

1. **Analizando comentarios de documentación** (`///` y `//!`) como Markdown.
2. **Generando HTML** a partir de esos comentarios Markdown, enlazando automáticamente tipos y elementos.
3. **Extrayendo bloques de código** (bloques ` ```rust `) de los comentarios de documentación y compilándolos como pruebas.

Esto significa que la documentación y las pruebas están unificadas. Un comentario de documentación que describe "aquí hay un ejemplo" se convierte en una prueba que verifica que el ejemplo es correcto.

```
código fuente → analizador rustdoc → Markdown → documentación HTML
                                  ↘ bloques de código → cargo test
```

---

## Comentarios de Documentación Externos vs Internos

```rust
//! Este es un comentario de documentación INTERNO — documenta el elemento en el que está DENTRO.
//! Se usa al inicio de un archivo para documentar el módulo/crate.
//! Se aplica a: lib.rs (crate), mod.rs (módulo), inicio de un archivo.

/// Este es un comentario de documentación EXTERNO — documenta el elemento DEBAJO de él.
/// Se usa inmediatamente antes de un struct, fn, enum, trait, const, etc.
pub struct MyType { ... }
```

**Regla general:**
- `//!` al inicio de `lib.rs` → documenta el crate
- `//!` al inicio de `foo.rs` → documenta el módulo `foo`
- `///` antes de cualquier elemento `pub` → documenta ese elemento

---

## Secciones Estándar de Comentarios de Documentación

Un comentario de documentación bien estructurado sigue este patrón:

```rust
/// Resumen de una línea. Esto es lo que aparece en los resultados de búsqueda y en los tooltips.
///
/// Descripción más larga opcional. Explica el POR QUÉ, no solo el QUÉ. Describe las invariantes,
/// compromisos y decisiones que no son obvios a partir de la firma del tipo.
///
/// # Ejemplos
///
/// Al menos un ejemplo ejecutable. Estos son compilados y ejecutados por `cargo test`.
///
/// ```rust
/// let x = MyType::new(42)?;
/// assert_eq!(x.value(), 42);
/// # Ok::<(), MyError>(())  // línea oculta: hace que el operador ? funcione en doc test
/// ```
///
/// # Panics
///
/// Documenta cuándo esta función entra en pánico. Si nunca entra en pánico, dilo.
/// En código de seguridad crítica, los panics están generalmente prohibidos — pero la *promesa*
/// de "nunca entra en pánico" debe estar documentada y verificada.
///
/// # Errores
///
/// Documenta cada variante de error que puede ser devuelta. Enlaza al tipo de error.
/// Esta es la sección más importante para funciones que devuelven `Result`.
///
/// Devuelve [`MyError::InvalidInput`] si el valor está fuera de rango.
///
/// # Safety
///
/// REQUERIDO para `unsafe fn`. Describe cada precondición que el llamador debe cumplir.
/// No cumplir estas precondiciones es comportamiento indefinido.
pub fn documented_function(value: u32) -> Result<MyType, MyError> { ... }
```

---

## Doc Tests: Ejemplos que Compilan y se Ejecutan

Cada bloque ` ```rust ` en un comentario de documentación es un caso de prueba:

```rust
/// Analiza un u16 big-endian de un slice de bytes.
///
/// # Ejemplos
/// ```
/// use my_crate::parse_u16_be;
/// let bytes = [0x01, 0x02];
/// assert_eq!(parse_u16_be(&bytes), 0x0102);
/// ```
pub fn parse_u16_be(bytes: &[u8]) -> u16 { ... }
```

Ejecútalos con: `cargo test --doc`

### Líneas Ocultas

Las líneas que comienzan con `# ` están ocultas en el HTML pero se incluyen en la prueba:

```rust
/// ```
/// let result = fallible_fn()?;
/// assert_eq!(result, 42);
/// # Ok::<(), MyError>(())   // ← oculta; hace que ? sea válido en el nivel superior
/// ```
```

### Marcados como No Ejecutables

Usa ` ```rust,no_run ` cuando el código requiere recursos externos (hardware, red):

```rust
/// ```rust,no_run
/// // Esto abriría un puerto serie real — no se puede ejecutar en CI
/// let port = UartDriver::open("/dev/ttyS0")?;
/// # Ok::<(), std::io::Error>(())
/// ```
```

Usa ` ```rust,compile_fail ` para documentar que algo es intencionalmente un error de compilación:

```rust
/// ```rust,compile_fail
/// // Esto NO debe compilar — el APID es demasiado grande
/// let hdr = CcsdsPrimaryHeader::new_tc(0xFFFF, 0, 0);
/// ```
```

---

## Enlaces Intra-Documentación

Rustdoc resuelve automáticamente los enlaces a otros elementos del mismo crate (o dependencias):

```rust
/// Devuelve un [`CcsdsFrame`] o [`CcsdsError::InvalidApid`].
///
/// Ver también: [`codec::encode`] y [`codec::decode`].
```

Usa la sintaxis de backtick con corchetes: `` [`TypeName`] ``, `` [`module::function`] ``.

Estos se verifican en tiempo de compilación — un enlace roto es una advertencia (o error con `RUSTDOCFLAGS`).

---

## `#[doc(hidden)]`

Marca un elemento para que NO aparezca en la documentación generada:

```rust
#[doc(hidden)]
pub fn internal_helper() { ... }  // público para uso de macros, pero no parte de la API
```

Úsalo con moderación. Si algo es verdaderamente interno, hazlo `pub(crate)` en su lugar.

---

## `#![deny(missing_docs)]`

El lint de documentación más importante para crates de biblioteca. Agrégalo al inicio de `lib.rs`:

```rust
#![deny(missing_docs)]
```

Esto hace que el compilador **se niegue a compilar** si algún elemento público carece de comentario de documentación.

En CI, esto significa que las APIs no documentadas son un fallo de compilación, no una nota de revisión de código.

---

## Compilar y Ver la Documentación

```bash
# Compilar la documentación solo para este crate (sin deps — más rápido)
cargo doc --no-deps

# Compilar y abrir en el navegador
cargo doc --no-deps --open

# Compilar para un crate específico en un workspace
cargo doc -p day6-rustdoc --no-deps --open

# Ejecutar solo los doc tests
cargo test --doc

# Ejecutar todas las pruebas (unitarias + integración + doc tests)
cargo test
```

---

## CI: Tratar las Advertencias de Documentación como Errores

En el pipeline de CI de un proyecto real, establece:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

Esto convierte las advertencias de documentación (enlaces intra-doc rotos, Markdown malformado) en fallos de compilación.

Combinado con `#![deny(missing_docs)]`, esto te da:
- Cada elemento público documentado: impuesto por el compilador
- Todos los doc tests pasando: impuesto por `cargo test --doc`
- Sin enlaces rotos: impuesto por `RUSTDOCFLAGS="-D warnings"`

---

## Cómo la Buena Documentación se Conecta con el Trabajo

Las APIs aeroespaciales son consumidas por equipos que:
- Están en un edificio, ciudad o país diferente
- No pueden interrumpirte con preguntas durante las fases de integración
- Usarán tu código años después de que te hayas mudado a otro proyecto
- Deben justificar cada decisión de interfaz ante una junta de revisión de seguridad

Tu comentario de documentación es lo más cercano que tendrán a hacerte una pregunta y obtener una respuesta.
Escríbelo como si estuvieras explicando a un ingeniero competente que nunca ha visto esta base de código
y no puede contactarte.

**Lista de verificación para cada `fn` pública:**
- [ ] Resumen de una línea (modo imperativo: "Crea un...", "Devuelve el...", "Analiza un...")
- [ ] Al menos un bloque `# Ejemplos` que compile y se ejecute
- [ ] Sección `# Errores` listando cada variante `Err`
- [ ] Sección `# Panics` (o declaración explícita de que nunca entra en pánico)
- [ ] Sección `# Safety` si la función es `unsafe`
- [ ] Enlaces intra-doc para todos los tipos mencionados en el texto
