//! Example 05 — Reading /proc for health telemetry
//!
//! /proc is a virtual filesystem that exposes kernel data structures as files.
//! For a spacecraft daemon, it's a gold mine for housekeeping telemetry:
//!   - /proc/uptime: system uptime
//!   - /proc/self/status: memory usage of the current process
//!   - /proc/loadavg: CPU load
//!   - /proc/meminfo: system-wide memory
//!
//! No external crates needed — just text parsing.
//! This is the raw data that PUS Service 3 HK reports would include.
//!
//! Run with:  cargo run --example 05_procfs_reader

#[tokio::main]
async fn main() {
    println!("=== /proc health telemetry ===\n");

    let uptime   = read_uptime().await;
    let mem      = read_self_memory().await;
    let load     = read_load_avg().await;
    let mem_info = read_mem_info().await;

    println!("System uptime:      {:.1}s ({:.1} hours)", uptime, uptime / 3600.0);
    println!("Process VmRSS:      {} kB  (resident set size)", mem.rss_kb);
    println!("Process VmPeak:     {} kB  (peak virtual memory)", mem.vm_peak_kb);
    println!("Load average:       {:.2} {:.2} {:.2}  (1m 5m 15m)", load.1min, load.5min, load.15min);
    println!("RAM total:          {} kB", mem_info.total_kb);
    println!("RAM available:      {} kB  ({:.1}% free)",
             mem_info.available_kb,
             mem_info.available_kb as f64 / mem_info.total_kb as f64 * 100.0);

    println!();
    println!("This data would be packed into a TM(3,25) HK report in day 7.");
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn read_uptime() -> f64 {
    let s = tokio::fs::read_to_string("/proc/uptime").await.unwrap_or_default();
    s.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

#[derive(Default)]
struct ProcMemory {
    rss_kb:     u64,
    vm_peak_kb: u64,
}

async fn read_self_memory() -> ProcMemory {
    let s = tokio::fs::read_to_string("/proc/self/status").await.unwrap_or_default();
    let mut m = ProcMemory::default();
    for line in s.lines() {
        let mut parts = line.splitn(2, ':');
        let (key, val) = match (parts.next(), parts.next()) {
            (Some(k), Some(v)) => (k.trim(), v.trim()),
            _ => continue,
        };
        let kb: u64 = val.split_whitespace().next()
            .and_then(|v| v.parse().ok()).unwrap_or(0);
        match key {
            "VmRSS"  => m.rss_kb = kb,
            "VmPeak" => m.vm_peak_kb = kb,
            _ => {}
        }
    }
    m
}

struct LoadAvg { r#1min: f64, r#5min: f64, r#15min: f64 }

async fn read_load_avg() -> LoadAvg {
    let s = tokio::fs::read_to_string("/proc/loadavg").await.unwrap_or_default();
    let mut parts = s.split_whitespace();
    LoadAvg {
        r#1min:  parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
        r#5min:  parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
        r#15min: parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
    }
}

struct MemInfo { total_kb: u64, available_kb: u64 }

async fn read_mem_info() -> MemInfo {
    let s = tokio::fs::read_to_string("/proc/meminfo").await.unwrap_or_default();
    let mut total_kb = 0u64;
    let mut available_kb = 0u64;
    for line in s.lines() {
        if line.starts_with("MemTotal:") {
            total_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if line.starts_with("MemAvailable:") {
            available_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        }
    }
    MemInfo { total_kb, available_kb }
}
