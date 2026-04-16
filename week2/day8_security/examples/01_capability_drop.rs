//! Ejemplo 01 — Descarte de privilegios mediante setuid/setgid + PR_SET_NO_NEW_PRIVS
//!
//! El principio de mínimo privilegio: un daemon debe mantener solo las capacidades
//! que NECESITA, durante el tiempo que las necesita.
//!
//! Patrón para un daemon OBC real:
//!   1. Arrancar como root (para abrir /dev/rawdevice, enlazar puerto 0-1023, etc.)
//!   2. Realizar las operaciones privilegiadas
//!   3. Descender a un usuario sin privilegios
//!   4. Establecer PR_SET_NO_NEW_PRIVS para que los procesos hijos no puedan recuperar privilegios
//!
//! Ejecutar con:  cargo run --example 01_capability_drop

use nix::unistd::{Uid, Gid, setuid, setgid, getuid, getgid};

/// Establece PR_SET_NO_NEW_PRIVS (específico de Linux).
///
/// Tras esta llamada, el proceso y todos sus hijos nunca podrán obtener nuevos
/// privilegios — aunque ejecuten un binario setuid.
fn set_no_new_privs() -> nix::Result<()> {
    // prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
    nix::sys::prctl::set_no_new_privs()
}

/// Descarta privilegios de root cambiando a uid/gid `nobody` (65534).
///
/// setgid debe llamarse ANTES que setuid, porque se necesita root para cambiar el GID.
fn drop_to_nobody() -> nix::Result<()> {
    let nobody_uid = Uid::from_raw(65534);
    let nobody_gid = Gid::from_raw(65534);
    setgid(nobody_gid)?; // debe ocurrir primero
    setuid(nobody_uid)?;
    Ok(())
}

fn main() {
    println!("=== Demo de descarte de privilegios ===\n");

    println!("Antes: uid={} gid={}", getuid(), getgid());

    // Simular aquí una operación privilegiada
    // (p. ej., abrir /dev/mem, enlazar a un socket raw)
    println!("Realizando inicialización privilegiada... (simulado)");

    // Establecer no-new-privs antes de descartar privilegios
    match set_no_new_privs() {
        Ok(()) => println!("PR_SET_NO_NEW_PRIVS: establecido"),
        Err(e) => println!("PR_SET_NO_NEW_PRIVS falló (esperado si no es Linux): {e}"),
    }

    // Intentar descender a nobody
    if getuid().is_root() {
        match drop_to_nobody() {
            Ok(()) => {
                println!("Tras el descarte: uid={} gid={}", getuid(), getgid());
                println!("Ahora ejecutándose como usuario sin privilegios.");
            }
            Err(e) => println!("drop_to_nobody falló: {e}"),
        }
    } else {
        println!("No se ejecuta como root — no se puede demostrar el descarte completo.");
        println!("uid actual={} gid actual={}", getuid(), getgid());
        println!();
        println!("En un daemon real se haría:");
        println!("  1. Arrancar como root (systemd puede hacer esto)");
        println!("  2. Abrir los recursos privilegiados");
        println!("  3. Descender a un usuario daemon dedicado (p. ej., 'obcdaemon')");
        println!("  4. Los permisos del daemon quedan reducidos permanentemente");
    }

    println!();
    println!("Capacidades Linux (más granulares que root/no-root):");
    println!("  CAP_SYS_RAWIO  → acceso a memoria/puertos hardware raw");
    println!("  CAP_NET_RAW    → sockets raw (necesario para acceso al bus CAN)");
    println!("  CAP_SYS_TIME   → establecer el reloj del sistema (Servicio PUS 9)");
    println!("  Usar `capsh` o el crate `caps` para control de grano fino.");
}
