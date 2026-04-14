//! Example 05 — D-Bus with zbus
//!
//! D-Bus is the standard IPC system on Linux desktops and embedded systems.
//! Key features:
//!  - Service registry: find other services by well-known name
//!  - Method calls: request/response like a function call across processes
//!  - Signals: broadcast events to all interested parties
//!  - Introspection: services describe their interface (like an API schema)
//!
//! On spacecraft OBCs running embedded Linux, D-Bus is used for:
//!  - Health status queries: "what is the thermal controller's current state?"
//!  - Service discovery: "is the comms subsystem available?"
//!  - Mode change notifications: "system is entering safe mode"
//!
//! Run with:  cargo run --example 05_dbus_intro
//!            (requires dbus session daemon, e.g., `dbus-daemon --session --fork`)

use zbus::{connection, interface, proxy};

/// The D-Bus interface our server exposes.
/// zbus generates all the glue code from this trait.
struct HealthService {
    component_name: String,
    health_state: String,
}

#[interface(name = "org.spacecraft.Health")]
impl HealthService {
    /// Returns the current health state of this component.
    async fn get_health(&self) -> String {
        format!("{}: {}", self.component_name, self.health_state)
    }

    /// Returns the component name.
    async fn get_name(&self) -> &str {
        &self.component_name
    }

    /// Simulates setting the health state.
    async fn set_health(&mut self, state: String) {
        println!("[server] health state changed: {}", state);
        self.health_state = state;
    }
}

/// A D-Bus proxy for the HealthService interface.
/// This is the client-side generated code.
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
    println!("=== D-Bus IPC with zbus ===\n");

    // Create connection to the session bus
    let conn = match connection::Builder::session()?.build().await {
        Ok(c) => c,
        Err(e) => {
            println!("Cannot connect to D-Bus session bus: {e}");
            println!("Start a session bus with: dbus-daemon --session --fork --print-address");
            println!("Or set: export DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/dbus_test.sock");
            return Ok(());
        }
    };

    // Register our service on the bus
    let health_svc = HealthService {
        component_name: "ThermalController".into(),
        health_state: "NOMINAL".into(),
    };

    conn.object_server().at("/org/spacecraft/thermal", health_svc).await?;
    conn.request_name("org.spacecraft.ThermalDaemon").await?;
    println!("Registered as 'org.spacecraft.ThermalDaemon' on session bus");

    // Simulate client queries (in a real system this would be a separate process)
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let proxy = HealthProxy::new(&conn).await?;

    println!("\nClient queries:");
    let name   = proxy.get_name().await?;
    let health = proxy.get_health().await?;
    println!("  get_name():   {name}");
    println!("  get_health(): {health}");

    proxy.set_health("DEGRADED: high temperature".into()).await?;
    let health = proxy.get_health().await?;
    println!("  After set_health: {health}");

    println!("\nD-Bus is excellent for service introspection:");
    println!("  $ dbus-send --session --print-reply --dest=org.spacecraft.ThermalDaemon \\");
    println!("    /org/spacecraft/thermal org.spacecraft.Health.GetHealth");

    Ok(())
}
