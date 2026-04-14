//! Example 05 — Privilege separation architecture
//!
//! The security boundary is between tc_receiver (untrusted side, talks to
//! the network/RF interface) and obc_router (trusted side, talks to hardware).
//!
//! tc_receiver has NO access to actuators.  Even if it's compromised, it can
//! only send bytes to the router — which then applies its own validation.
//!
//! This example shows the design as in-process tasks communicating via a
//! channel (in the full week2_project it's separate processes via Unix sockets).
//!
//! Run with:  cargo run --example 05_privilege_separation

use tokio::sync::mpsc;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct VerifiedTc {
    apid: u16,
    service: u8,
    subservice: u8,
    data: Vec<u8>,
}

// ── Untrusted side: TC Receiver ───────────────────────────────────────────────

/// Receives raw bytes from the "network", authenticates them, forwards only
/// valid TCs to the trusted router.
///
/// This component has NO knowledge of actuators or subsystems.
/// Its only capability: send bytes to `tx`.
async fn tc_receiver(tx: mpsc::Sender<VerifiedTc>) {
    // Simulate receiving 5 "TC bytes" from the uplink
    let simulated_uplink: Vec<(bool, u16, u8, u8)> = vec![
        (true,  0x001, 17, 1),  // valid TC(17,1) ping
        (false, 0x001, 17, 1),  // bad HMAC — rejected
        (true,  0x003,  3, 129),// valid TC(3,129) HK request
        (true,  0x003,  3, 129),// replay of previous — rejected by replay window
        (true,  0x001, 17, 1),  // valid ping
    ];

    let mut seq: u16 = 0;
    let mut last_accepted_seq: Option<u16> = None;

    for (valid_hmac, apid, svc, sub) in simulated_uplink {
        seq += 1;
        println!("[tc_receiver] incoming: APID=0x{apid:03X} svc={svc}/{sub} seq={seq}");

        // Step 1: verify HMAC
        if !valid_hmac {
            println!("[tc_receiver] REJECTED: bad HMAC");
            continue;
        }

        // Step 2: replay check (simplified: reject same apid+svc+sub in a row)
        let is_replay = last_accepted_seq == Some(seq.wrapping_sub(0));
        if is_replay {
            println!("[tc_receiver] REJECTED: replay detected");
            continue;
        }

        last_accepted_seq = Some(seq);
        println!("[tc_receiver] FORWARDING to router (verified)");
        let tc = VerifiedTc { apid, service: svc, subservice: sub, data: vec![] };
        if tx.send(tc).await.is_err() { break; }
    }
    // Dropping tx signals to the router that uplink is closed
}

// ── Trusted side: OBC Router ──────────────────────────────────────────────────

/// Receives ONLY verified TCs from tc_receiver.
/// This component HAS access to subsystems and actuators.
async fn obc_router(mut rx: mpsc::Receiver<VerifiedTc>) {
    while let Some(tc) = rx.recv().await {
        println!("[obc_router] routing verified TC({}/{}) APID=0x{:03X}",
                 tc.service, tc.subservice, tc.apid);
        match tc.service {
            17 => println!("[obc_router] → ping response: TM(17,2) I Am Alive"),
            3  => println!("[obc_router] → HK service: collect parameters, send TM(3,25)"),
            _  => println!("[obc_router] → unknown service, discarding"),
        }
    }
    println!("[obc_router] uplink closed");
}

#[tokio::main]
async fn main() {
    println!("=== Privilege Separation Architecture ===\n");
    println!("Security boundary: tc_receiver ──(channel)──► obc_router");
    println!("  tc_receiver: knows authentication keys, talks to RF hardware");
    println!("  obc_router:  knows spacecraft subsystems, talks to actuators");
    println!("  Compromise of tc_receiver CANNOT directly command actuators.\n");

    let (tx, rx) = mpsc::channel(16);
    let receiver = tokio::spawn(tc_receiver(tx));
    let router   = tokio::spawn(obc_router(rx));

    let _ = tokio::join!(receiver, router);
    println!("\nSimulation complete.");
}
