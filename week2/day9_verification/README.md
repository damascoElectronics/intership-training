# Día 9 — Verificación y pruebas

**Tema:** Pruebas basadas en propiedades, invariantes de máquinas de estado y seguridad de memoria con Miri.

Las pruebas unitarias comprueban casos específicos. Las pruebas basadas en propiedades comprueban *todos* los casos — el framework
genera miles de entradas aleatorias y busca contraejemplos. Para software embebido de seguridad crítica
esto marca la diferencia entre "funcionó en nuestras pruebas" y "funciona".

---

## Objetivos de aprendizaje

- Escribir propiedades `proptest!` para viajes de ida y vuelta de paquetes CCSDS
- Modelar una máquina de estados como una propiedad (cualquier secuencia válida de eventos debe obedecer los invariantes)
- Probar un codec (COBS) con propiedades de viaje de ida y vuelta encode/decode y de límites
- Entender qué comprueba Miri y cómo estructurar código unsafe para superarlo

---

## Ejemplos

| Fichero | Qué demuestra |
|---------|---------------|
| `01_proptest_packet.rs` | Viaje de ida y vuelta de APID, contador de secuencia, paquete completo; sin pánico con bytes arbitrarios |
| `02_proptest_state_machine.rs` | Estrategia de eventos de salud, invariante `failed_requires_manual_reset` |
| `03_property_tests.rs` | COBS encode/decode: `encoded_has_no_zeros`, `roundtrip`, `length_bound` |
| `04_miri_safety.rs` | `MaybeUninit`, procedencia de punteros; ejemplos UB comentados con explicaciones |

Ejecutar todas las pruebas (incluye proptest):
```
cargo test -p day9-verification
```

Ejecutar Miri en el ejemplo de seguridad (requiere nightly + `cargo +nightly miri`):
```
cargo +nightly miri test -p day9-verification
```

---

## Ejercicios

### Ejercicio 1 — Encontrar el bug en COBS (`ex1_proptest_codec.rs`)

El fichero contiene un encoder COBS deliberadamente defectuoso. Tu tarea:

1. Escribir una propiedad proptest `roundtrip`: `decode(encode(input)) == input`
2. Escribir una propiedad `no_zeros`: la salida codificada nunca contiene `0x00`
3. Ejecutar `cargo test` — proptest encontrará un caso que falla
4. Corregir el encoder
5. Añadir una prueba de regresión para la entrada específica que falló

**Pista:** el bug se manifiesta solo cuando una secuencia de bytes no nulos tiene exactamente 254 bytes de longitud.

Solución: `ex1_proptest_codec_sol.rs`

---

## Conceptos clave

### Qué hace proptest

```rust
proptest! {
    #[test]
    fn roundtrip(input in any::<Vec<u8>>()) {
        let encoded = encode(&input);
        let decoded = decode(&encoded).unwrap();
        prop_assert_eq!(decoded, input);
    }
}
```

La macro genera 256 valores `Vec<u8>` aleatorios (configurable). Cuando encuentra un fallo,
*reduce* — encuentra el caso mínimo que falla — y lo reporta. "Longitud de entrada 254, todos los bytes
= 0xFF" es más útil que "algún buffer de 10 000 bytes".

### COBS (Consistent Overhead Byte Stuffing)

COBS es un codec de enmarcado serie que elimina `0x00` de los datos codificados. Esto permite usar `0x00`
como delimitador de paquetes en un flujo de bytes, dando un enmarcado inequívoco sin escaping.

Reglas de codificación:
- Reemplazar cada `0x00` con un puntero hacia el siguiente `0x00` (o fin de trama)
- Máximo overhead: 1 byte por cada 254 bytes de payload (un byte de overhead al inicio, uno por cada bloque de 254)

### Miri

Miri es un intérprete para Rust MIR que detecta:
- Uso de memoria no inicializada
- Accesos a memoria fuera de límites
- Violaciones de aliasing de punteros (stacked borrows)
- Valores inválidos en posiciones tipadas

Ejecutarlo con `cargo +nightly miri test`. Es lento (10–100× la ejecución real) pero encuentra bugs
que los sanitizadores no detectan. Ideal para ejecutar sobre los módulos `unsafe` de tu código.

### Cuando proptest encuentra un bug

1. Examinar la **entrada reducida** — es el contraejemplo mínimo
2. Añadirla como caso de regresión `#[test]` inmediatamente (antes de corregir el bug)
3. Corregir el bug
4. Confirmar que tanto la prueba de regresión como proptest pasan

Este flujo de trabajo se llama *depuración orientada por pruebas*.
