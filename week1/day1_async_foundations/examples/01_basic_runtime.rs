// Día 1, Ejemplo 1: Runtime Básico de Tokio
//
// Este ejemplo muestra qué hace realmente #[tokio::main] por debajo,
// e introduce el patrón básico async fn / .await.
//
// Si has usado FreeRTOS: piensa en el runtime de tokio como el kernel del RTOS,
// y en las funciones async como tareas. La diferencia clave es que las tareas de tokio
// son *cooperativas* — ceden el control voluntariamente en los puntos .await,
// en lugar de ser interrumpidas por una interrupción de temporizador.
//
// Ejecutar con:
//   cargo run --example 01_basic_runtime

// El atributo macro #[tokio::main] transforma nuestra async fn main() en
// una fn main() regular que construye un runtime de Tokio y bloquea sobre él.
//
// Expandiendo el macro manualmente:
//
//   #[tokio::main]
//   async fn main() { ... }
//
// se convierte aproximadamente en:
//
//   fn main() {
//       tokio::runtime::Builder::new_multi_thread()
//           .enable_all()   // habilita los drivers de tiempo e I/O
//           .build()
//           .expect("Falló la construcción del runtime de Tokio")
//           .block_on(async {
//               // el cuerpo de tu main async aquí
//           })
//   }
//
// `block_on` significa: ejecutar este future hasta completarse en el hilo OS actual,
// usando el reactor de este runtime para manejar eventos de I/O. Es una llamada
// síncrona — no retorna hasta que el future se completa.
//
// Para uso monohilo (p. ej., entornos tipo embedded, pruebas),
// puedes usar #[tokio::main(flavor = "current_thread")] que evita
// lanzar hilos OS adicionales por completo.
#[tokio::main]
async fn main() {
    println!("=== Ejemplo 01: Runtime Básico ===\n");

    // Llamar a una función async NO la ejecuta inmediatamente.
    // Devuelve un Future — una descripción del trabajo a realizar.
    // Solo cuando haces .await el runtime lo lleva a completarse.
    //
    // Esto es análogo a configurar una transferencia DMA en STM32:
    // HAL_SPI_Transmit_DMA() configura las cosas pero no bloquea.
    // La transferencia ocurre de forma asíncrona; recibes un callback o interrupción.
    // Aquí, .await es ese punto de "esperar a que termine".
    let result = compute_something(10).await;
    println!("compute_something(10) retornó: {}", result);

    // Los bloques async crean un future anónimo en línea.
    // Útil para trabajo async puntual sin definir una función con nombre.
    let inline_result = async {
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        42u32
    }
    .await;
    println!("Resultado del bloque async en línea: {}", inline_result);

    // Demostrar que las funciones async se componen naturalmente.
    // Cada .await es un posible punto de cesión donde otras tareas pueden ejecutarse.
    // En bare metal esto sería la cesión de tu planificador cooperativo.
    let total = add_async(compute_something(3).await, compute_something(7).await).await;
    println!("add_async(3+7 computados) = {}", total);

    // tokio::time::sleep cede la tarea actual por la duración indicada.
    // A diferencia de std::thread::sleep, esto NO bloquea el hilo OS —
    // otras tareas siguen ejecutándose mientras esperamos.
    // El reactor registra un temporizador y nos despierta cuando dispara.
    println!("\nDurmiendo 50ms (no bloqueante — otras tareas podrían ejecutarse)...");
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    println!("Despertado tras 50ms");

    println!("\nListo.");
}

// Una función async es azúcar sintáctico para una función que retorna
// impl Future<Output = T>. El compilador reescribe tu código secuencial
// en una máquina de estados (similar a como escribirías a mano una máquina
// de estados no bloqueante en C embebido, pero el compilador lo hace por ti).
//
// Cada punto .await es una transición de estado: el future almacena todas las
// variables locales que necesita a través de ese punto, luego se suspende.
async fn compute_something(input: u32) -> u32 {
    // Simular latencia de I/O (como esperar la respuesta de un sensor por UART).
    // En código real esto sería I/O async real, no un sleep.
    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;

    // El compilador captura `input` a través del .await anterior —
    // se almacena en la máquina de estados, no en la pila de llamadas.
    input * input
}

async fn add_async(a: u32, b: u32) -> u32 {
    // No todas las funciones async necesitan hacer .await a algo.
    // Ser async solo significa que el llamador puede hacernos .await uniformemente.
    a + b
}
