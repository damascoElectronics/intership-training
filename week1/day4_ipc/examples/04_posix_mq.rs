//! Example 04 — POSIX Message Queues
//!
//! POSIX MQs are unique among IPC mechanisms because they support PRIORITY.
//! Higher-priority messages are always received before lower-priority ones,
//! regardless of arrival order.
//!
//! This is highly relevant for spacecraft software:
//!  - A TC(17,1) ping should be processed before a low-priority HK request
//!  - A fault event should preempt a routine telemetry flush
//!
//! Run with:  cargo run --example 04_posix_mq

use nix::mqueue::{mq_close, mq_open, mq_receive, mq_send, mq_unlink, MqAttr, OFlag};
use nix::sys::stat::Mode;
use std::ffi::CString;

const MQ_NAME: &str = "/day4_mq_demo";

// Priority levels (higher = more urgent, received first)
const PRIO_LOW:    u32 = 0;
const PRIO_MEDIUM: u32 = 5;
const PRIO_HIGH:   u32 = 10;

fn main() {
    println!("=== POSIX Message Queue priority demo ===\n");

    let mq_name = CString::new(MQ_NAME).unwrap();

    // Clean up any leftover queue from previous run
    let _ = mq_unlink(&mq_name);

    // Create the queue: max 10 messages, each up to 256 bytes
    let attrs = MqAttr::new(0, 10, 256, 0);
    let mq = mq_open(
        &mq_name,
        OFlag::O_CREAT | OFlag::O_RDWR,
        Mode::S_IRUSR | Mode::S_IWUSR,
        Some(&attrs),
    )
    .expect("create mq");

    println!("Created POSIX MQ '{MQ_NAME}' (max 10 msgs, 256 bytes each)");

    // Send messages in LOW → MEDIUM → HIGH order
    // They will be received in HIGH → MEDIUM → LOW order (priority queue)
    let messages = [
        (PRIO_LOW,    "TC(3,129) Request HK report [low priority]"),
        (PRIO_MEDIUM, "TC(5,1)   Log status event [medium priority]"),
        (PRIO_HIGH,   "TC(17,1)  Are-You-Alive ping [HIGH priority]"),
        (PRIO_LOW,    "TC(3,130) Enable periodic HK [low priority]"),
        (PRIO_HIGH,   "TC(9,1)   Synchronize time [HIGH priority]"),
    ];

    println!("\nSending messages in this order:");
    for (prio, msg) in &messages {
        println!("  [prio={prio:2}] {msg}");
        mq_send(mq, msg.as_bytes(), *prio).expect("mq_send");
    }

    println!("\nReceiving (priority order — NOT insertion order):");
    let mut buf = vec![0u8; 256];
    for _ in 0..messages.len() {
        let (len, prio) = mq_receive(mq, &mut buf, None).expect("mq_receive");
        let msg = std::str::from_utf8(&buf[..len]).unwrap();
        println!("  [prio={prio:2}] {msg}");
    }

    mq_close(mq).ok();
    mq_unlink(&mq_name).ok();

    println!();
    println!("Key insight: messages arrived in priority order regardless of send order.");
    println!("This is how you implement TC priority lanes in a spacecraft router.");
}
