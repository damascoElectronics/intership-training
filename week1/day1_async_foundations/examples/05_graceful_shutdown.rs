// Day 1, Example 5: Graceful Shutdown
//
// Every production daemon must handle SIGTERM (systemd stopping the service)
// and SIGINT (Ctrl+C in dev). If you don't, the process gets killed with
// SIGKILL after a timeout, leaving hardware in an undefined state.
//
// On bare metal you might use a GPIO interrupt or watchdog. In Linux:
// - systemd sends SIGTERM when stopping a service
// - You have `TimeoutStopSec` seconds to exit, then you get SIGKILL
// - A clean exit (exit code 0) tells systemd "healthy stop"
// - An unclean exit (crash/SIGKILL) triggers restart policies
//
// The pattern implemented here is used in real space ground software:
// 1. Install signal handlers for SIGTERM + SIGINT
// 2. Broadcast shutdown via CancellationToken
// 3. Each task winds down, flushes data, releases hardware
// 4. Main waits with a hard timeout (don't hang forever)
// 5. Clean exit
//
// Run with:
//   cargo run --example 05_graceful_shutdown
// Then press Ctrl+C to trigger graceful shutdown.
// Or send: kill -SIGTERM <pid>

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

// Represents buffered telemetry that must be flushed on shutdown.
// In a real system this might be queued CCSDS packets.
struct TelemetryBuffer {
    packets: Vec<String>,
}

impl TelemetryBuffer {
    fn new() -> Self {
        Self { packets: Vec::new() }
    }

    fn push(&mut self, pkt: String) {
        self.packets.push(pkt);
    }

    async fn flush(&mut self) {
        // Simulate writing buffered telemetry to a file or sending over a socket
        if self.packets.is_empty() {
            println!("  Flush: buffer empty, nothing to do");
            return;
        }
        println!("  Flush: writing {} buffered packets...", self.packets.len());
        tokio::time::sleep(Duration::from_millis(20)).await; // simulate I/O
        println!("  Flush: done");
        self.packets.clear();
    }
}

#[tokio::main]
async fn main() {
    println!("=== Example 05: Graceful Shutdown ===");
    println!("Press Ctrl+C to trigger graceful shutdown\n");

    // The root cancellation token. We cancel this to initiate shutdown.
    // Every task gets a clone — they all see cancellation simultaneously.
    let shutdown_token = CancellationToken::new();

    // A channel for collecting telemetry from all tasks.
    // Bounded: if the receiver can't keep up, senders block (backpressure).
    let (telem_tx, telem_rx) = mpsc::channel::<String>(64);

    // ── Spawn worker tasks ──────────────────────────────────────────────────

    let heartbeat_handle = tokio::spawn(heartbeat_task(
        shutdown_token.clone(),
        telem_tx.clone(),
    ));

    let sensor_handle = tokio::spawn(sensor_task(
        shutdown_token.clone(),
        telem_tx.clone(),
    ));

    let telem_handle = tokio::spawn(telemetry_aggregator(
        shutdown_token.clone(),
        telem_rx,
    ));

    // Drop our copy of telem_tx so the aggregator knows when all producers quit
    drop(telem_tx);

    // ── Install signal handlers ─────────────────────────────────────────────
    //
    // tokio::signal::ctrl_c() is a cross-platform Ctrl+C handler.
    // On Unix, it installs a SIGINT handler.
    //
    // For SIGTERM (the signal systemd sends), we need the Unix-specific API.
    // Using tokio::signal::unix::signal() requires the "signal" feature of tokio.

    #[cfg(unix)]
    let shutdown_reason = {
        use tokio::signal::unix::{signal, SignalKind};

        // Install both SIGTERM and SIGINT handlers.
        // signal() returns a stream — we recv() from it to wait for the signal.
        let mut sigterm = signal(SignalKind::terminate())
            .expect("Failed to install SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt())
            .expect("Failed to install SIGINT handler");

        // Also set up a 3-second auto-shutdown for demo purposes
        // (so the example terminates without requiring manual Ctrl+C)
        let auto_shutdown = tokio::time::sleep(Duration::from_secs(3));

        // Race: whichever arrives first triggers shutdown.
        // In a real daemon you'd omit the auto_shutdown branch.
        tokio::select! {
            _ = sigterm.recv() => "SIGTERM",
            _ = sigint.recv()  => "SIGINT (Ctrl+C)",
            _ = auto_shutdown  => "auto-shutdown (demo mode)",
        }
    };

    // On non-Unix platforms, just use ctrl_c
    #[cfg(not(unix))]
    let shutdown_reason = {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => "SIGINT (Ctrl+C)",
            _ = tokio::time::sleep(Duration::from_secs(3)) => "auto-shutdown (demo mode)",
        }
    };

    // ── Initiate graceful shutdown ──────────────────────────────────────────

    println!("\n[SHUTDOWN] Received: {shutdown_reason}");
    println!("[SHUTDOWN] Broadcasting cancellation to all tasks...");

    // This single call notifies ALL tasks that hold a clone of this token.
    // They will each complete their current unit of work and then exit.
    shutdown_token.cancel();

    // ── Wait for all tasks to exit (with hard timeout) ──────────────────────
    //
    // systemd's default TimeoutStopSec is 90s. We give our tasks 5s.
    // If a task hangs (e.g., blocked on hardware), we don't want to block forever.

    println!("[SHUTDOWN] Waiting for tasks to exit (5s timeout)...");

    let all_tasks = async {
        // Join all task handles. If any panicked, propagate the error.
        let _ = heartbeat_handle.await;
        let _ = sensor_handle.await;
        let _ = telem_handle.await;
    };

    match timeout(Duration::from_secs(5), all_tasks).await {
        Ok(()) => println!("[SHUTDOWN] All tasks exited cleanly"),
        Err(_) => {
            println!("[SHUTDOWN] WARNING: Some tasks did not exit within timeout");
            println!("[SHUTDOWN] Proceeding anyway (they will be killed on process exit)");
        }
    }

    println!("[SHUTDOWN] Daemon stopped cleanly. Goodbye.");
    // process::exit(0) is called implicitly — systemd sees exit code 0
}

// ─────────────────────────────────────────────────────────────────────────────
// Worker tasks: each follows the same pattern
//   - do work in a loop
//   - check cancellation token
//   - on cancellation: flush/cleanup, then return
// ─────────────────────────────────────────────────────────────────────────────

async fn heartbeat_task(token: CancellationToken, tx: mpsc::Sender<String>) {
    let mut seq = 0u32;
    println!("[heartbeat] started");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                // Shutdown requested. Send a final "shutdown heartbeat" if possible.
                let final_hb = format!("HB seq={seq} status=SHUTDOWN");
                // try_send doesn't block — if channel is full, we skip it
                let _ = tx.try_send(final_hb);
                println!("[heartbeat] stopped (sent {seq} heartbeats)");
                return;
            }
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                seq += 1;
                let hb = format!("HB seq={seq} status=NOMINAL");
                println!("[heartbeat] tick {seq}");
                if tx.send(hb).await.is_err() {
                    println!("[heartbeat] telemetry channel closed, stopping");
                    return;
                }
            }
        }
    }
}

async fn sensor_task(token: CancellationToken, tx: mpsc::Sender<String>) {
    let mut buffer = TelemetryBuffer::new();
    println!("[sensor] started");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                // On shutdown: flush whatever is buffered before exiting.
                // This is critical — in a space system, you can't lose in-flight data.
                println!("[sensor] shutdown received, flushing buffer...");
                buffer.flush().await;
                println!("[sensor] stopped");
                return;
            }
            _ = tokio::time::sleep(Duration::from_millis(300)) => {
                // Collect sensor reading into buffer
                let pkt = format!("TM temp={:.1}", 20.0 + (rand_f32() * 5.0));
                buffer.push(pkt.clone());
                println!("[sensor] buffered: {pkt}");

                // Flush when buffer has enough packets
                if buffer.packets.len() >= 3 {
                    buffer.flush().await;
                    // In real code, flushed data would go somewhere (file, socket)
                    // Here we just send a notification
                    let _ = tx.try_send("TM flush complete".into());
                }
            }
        }
    }
}

async fn telemetry_aggregator(token: CancellationToken, mut rx: mpsc::Receiver<String>) {
    let mut total_received = 0usize;
    println!("[telem] started");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                // Drain remaining messages in channel before stopping.
                // After cancellation, producers might send a few final messages.
                // We give them a short window to finish.
                println!("[telem] shutdown — draining channel...");
                // Use close() + drain loop to process remaining messages
                rx.close(); // stop accepting new sends
                while let Ok(msg) = rx.try_recv() {
                    println!("[telem] final msg: {msg}");
                    total_received += 1;
                }
                println!("[telem] stopped (total received: {total_received})");
                return;
            }
            msg = rx.recv() => {
                match msg {
                    Some(pkt) => {
                        total_received += 1;
                        // In real code: write to database, forward to ground station, etc.
                    }
                    None => {
                        // All senders dropped — channel closed
                        println!("[telem] channel closed, stopping (total: {total_received})");
                        return;
                    }
                }
            }
        }
    }
}

// Minimal pseudo-random float for demo purposes (no rand dependency needed)
fn rand_f32() -> f32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 1000) as f32 / 1000.0
}
