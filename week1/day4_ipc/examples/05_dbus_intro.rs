//! Ejemplo 05 — D-Bus con zbus
//!
//! D-Bus es el sistema IPC estándar en escritorios Linux y sistemas embebidos.
//! Características clave:
//!  - Registro de servicios: encontrar otros servicios por nombre conocido
//!  - Llamadas a métodos: solicitud/respuesta como una llamada de función entre procesos
//!  - Señales: eventos de difusión a todas las partes interesadas
//!  - Introspección: los servicios describen su interfaz (como un esquema de API)
//!
//! En OBCs de naves espaciales con Linux embebido, D-Bus se usa para:
//!  - Consultas de estado de salud: "¿cuál es el estado actual del controlador térmico?"
//!  - Descubrimiento de servicios: "¿está disponible el subsistema de comunicaciones?"
//!  - Notificaciones de cambio de modo: "el sistema está entrando en modo seguro"
//!
//! Ejecutar con:  cargo run --example 05_dbus_intro
//!            (requiere el daemon de sesión dbus, ej., `dbus-daemon --session --fork`)

use zbus::{connection, interface, proxy};

/// La interfaz D-Bus que expone nuestro servidor.
/// zbus genera todo el código de pegamento a partir de este trait.
struct HealthService {
    component_name: String,
    health_state: String,
}

#[interface(name = "org.spacecraft.Health")]
impl HealthService {
    /// Devuelve el estado de salud actual de este componente.
    async fn get_health(&self) -> String {
        format!("{}: {}", self.component_name, self.health_state)
    }

    /// Devuelve el nombre del componente.
    async fn get_name(&self) -> &str {
        &self.component_name
    }

    /// Simula establecer el estado de salud.
    async fn set_health(&mut self, state: String) {
        println!("[servidor] estado de salud cambiado: {}", state);
        self.health_state = state;
    }
}

/// Un proxy D-Bus para la interfaz HealthService.
/// Este es el código generado del lado del cliente.
#[proxy(
    interface = "org.spacecraft.Health",
    default_service = "org.spacecraft.ThermalDaemon",
    default_path = "/org/spacecraft/thermal"
)]
trait Health {
    async fn get_health(&self) -> zbus::Result<String>;
    async fn get_name(&self) -> zbus::Result<String>;
    async fn set_health(&self, state: String) -> zbus::Result<()>;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== IPC con D-Bus usando zbus ===\n");

    // Crear conexión al bus de sesión
    let conn = match connection::Builder::session()?.build().await {
        Ok(c) => c,
        Err(e) => {
            println!("No se puede conectar al bus de sesión D-Bus: {e}");
            println!("Iniciar un bus de sesión con: dbus-daemon --session --fork --print-address");
            println!("O establecer: export DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/dbus_test.sock");
            return Ok(());
        }
    };

    // Registrar nuestro servicio en el bus
    let health_svc = HealthService {
        component_name: "ThermalController".into(),
        health_state: "NOMINAL".into(),
    };

    conn.object_server().at("/org/spacecraft/thermal", health_svc).await?;
    conn.request_name("org.spacecraft.ThermalDaemon").await?;
    println!("Registrado como 'org.spacecraft.ThermalDaemon' en el bus de sesión");

    // Simular consultas del cliente (en un sistema real esto sería un proceso separado)
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let proxy = HealthProxy::new(&conn).await?;

    println!("\nConsultas del cliente:");
    let name   = proxy.get_name().await?;
    let health = proxy.get_health().await?;
    println!("  get_name():   {name}");
    println!("  get_health(): {health}");

    proxy.set_health("DEGRADED: temperatura alta".into()).await?;
    let health = proxy.get_health().await?;
    println!("  Después de set_health: {health}");

    println!("\nD-Bus es excelente para la introspección de servicios:");
    println!("  $ dbus-send --session --print-reply --dest=org.spacecraft.ThermalDaemon \\");
    println!("    /org/spacecraft/thermal org.spacecraft.Health.GetHealth");

    Ok(())
}
