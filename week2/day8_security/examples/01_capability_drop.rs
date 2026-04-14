//! Example 01 — Privilege dropping via setuid/setgid + PR_SET_NO_NEW_PRIVS
//!
//! The principle of least privilege: a daemon should hold only the capabilities
//! it NEEDS, for only as LONG as it needs them.
//!
//! Pattern for a real OBC daemon:
//!   1. Start as root (to open /dev/rawdevice, bind port 0-1023, etc.)
//!   2. Do the privileged operations
//!   3. Drop to an unprivileged user
//!   4. Set PR_SET_NO_NEW_PRIVS so child processes cannot regain privileges
//!
//! Run with:  cargo run --example 01_capability_drop

use nix::unistd::{Uid, Gid, setuid, setgid, getuid, getgid};

/// Sets PR_SET_NO_NEW_PRIVS (Linux-specific).
///
/// After this call, the process and all its children can never gain new
/// privileges — even if they execute a setuid binary.
fn set_no_new_privs() -> nix::Result<()> {
    // prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
    nix::sys::prctl::set_no_new_privs()
}

/// Drops root privileges by switching to uid/gid `nobody` (65534).
///
/// setgid must be called BEFORE setuid, because you need root to change GID.
fn drop_to_nobody() -> nix::Result<()> {
    let nobody_uid = Uid::from_raw(65534);
    let nobody_gid = Gid::from_raw(65534);
    setgid(nobody_gid)?; // must happen first
    setuid(nobody_uid)?;
    Ok(())
}

fn main() {
    println!("=== Privilege Dropping Demo ===\n");

    println!("Before: uid={} gid={}", getuid(), getgid());

    // Simulate doing something privileged here
    // (e.g., opening /dev/mem, binding to a raw socket)
    println!("Performing privileged initialization... (simulated)");

    // Set no-new-privs before dropping privileges
    match set_no_new_privs() {
        Ok(()) => println!("PR_SET_NO_NEW_PRIVS: set"),
        Err(e) => println!("PR_SET_NO_NEW_PRIVS failed (expected if not Linux): {e}"),
    }

    // Attempt to drop to nobody
    if getuid().is_root() {
        match drop_to_nobody() {
            Ok(()) => {
                println!("After drop: uid={} gid={}", getuid(), getgid());
                println!("Now running as unprivileged user.");
            }
            Err(e) => println!("drop_to_nobody failed: {e}"),
        }
    } else {
        println!("Not running as root — cannot demonstrate full drop.");
        println!("Current uid={} gid={}", getuid(), getgid());
        println!();
        println!("In a real daemon you would:");
        println!("  1. Start as root (systemd can do this)");
        println!("  2. Open privileged resources");
        println!("  3. Drop to a dedicated daemon user (e.g., 'obcdaemon')");
        println!("  4. The daemon's permissions are now permanently reduced");
    }

    println!();
    println!("Linux capabilities (more granular than root/non-root):");
    println!("  CAP_SYS_RAWIO  → access to raw hardware memory/ports");
    println!("  CAP_NET_RAW    → raw sockets (needed for CAN bus access)");
    println!("  CAP_SYS_TIME   → set system clock (PUS Service 9)");
    println!("  Use `capsh` or the `caps` crate for fine-grained control.");
}
