//! Example 03 — Named pipes (FIFOs)
//!
//! A FIFO is the simplest unidirectional IPC mechanism.  Unlike an anonymous
//! pipe (used between parent and child processes), a named pipe lives in the
//! filesystem and can be opened by unrelated processes.
//!
//! Key behaviour: the open() call BLOCKS until both ends are open.
//! This is a common source of confusion — both the writer and reader must
//! open the FIFO before either proceeds.
//!
//! Run with:  cargo run --example 03_named_pipe

use nix::sys::stat::Mode;
use nix::unistd::mkfifo;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::fs::OpenOptions;

const FIFO_PATH: &str = "/tmp/day4_fifo_demo";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the FIFO if it doesn't exist
    let path = Path::new(FIFO_PATH);
    if !path.exists() {
        mkfifo(path, Mode::S_IRUSR | Mode::S_IWUSR)?;
        println!("Created FIFO at {FIFO_PATH}");
    }

    println!("Spawning producer and consumer tasks...");
    println!("Note: both tasks must open the FIFO before either can proceed.\n");

    // The producer and consumer must open in separate tasks — if you try to
    // open both ends sequentially in one task, you deadlock (each open blocks
    // until the other end is open).
    let producer = tokio::spawn(async {
        println!("[producer] opening FIFO for writing (will block until reader opens)...");
        let mut writer = OpenOptions::new()
            .write(true)
            .open(FIFO_PATH)
            .await
            .expect("open FIFO write");
        println!("[producer] FIFO open, sending messages");

        for i in 1..=5 {
            let msg = format!("message #{i} from producer\n");
            writer.write_all(msg.as_bytes()).await.expect("write");
            println!("[producer] sent: '{}'", msg.trim());
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        println!("[producer] done, closing FIFO");
        // Dropping writer closes the write end → consumer sees EOF
    });

    let consumer = tokio::spawn(async {
        println!("[consumer] opening FIFO for reading (will block until writer opens)...");
        let reader = OpenOptions::new()
            .read(true)
            .open(FIFO_PATH)
            .await
            .expect("open FIFO read");
        println!("[consumer] FIFO open, reading messages");

        let mut lines = BufReader::new(reader).lines();
        while let Some(line) = lines.next_line().await.expect("read") {
            println!("[consumer] received: '{line}'");
        }
        println!("[consumer] EOF — producer closed the write end");
    });

    producer.await?;
    consumer.await?;

    // Clean up
    let _ = std::fs::remove_file(FIFO_PATH);

    println!("\nWhen to use FIFOs:");
    println!("  ✓ Simple one-way data streams (e.g., log pipeline: daemon → logger)");
    println!("  ✓ Shell-friendly (cat, nc can read/write FIFOs)");
    println!("  ✗ Not suitable for bidirectional comms (need two FIFOs)");
    println!("  ✗ Not suitable for multiple producers (no framing guarantee)");

    Ok(())
}
