// System information collection.

use serde::Serialize;
use std::fs;
use std::thread;

#[derive(Debug, Clone, Serialize)]
pub struct SysInfo {
    pub cpu_model: String,
    pub logical_cores: usize,
    pub ram_mib: u64,
    pub kernel: String,
    pub os: String,
    /// Stable, anonymized per-machine identifier. The same machine always
    /// reports the same value, so the score server can collapse a machine's
    /// repeated submissions into a single leaderboard row.
    pub machine_id: String,
}

impl SysInfo {
    pub fn collect() -> Self {
        SysInfo {
            cpu_model: read_cpu_model(),
            logical_cores: thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            ram_mib: read_mem_total_mib(),
            kernel: read_kernel(),
            os: read_os(),
            machine_id: read_machine_id(),
        }
    }

    pub fn print(&self) {
        println!("===================================================");
        println!("  CRUCIBLE — crux benchmark  (trial by fire)");
        println!("===================================================");
        println!("CPU    : {}", self.cpu_model);
        println!("Cores  : {}", self.logical_cores);
        println!("RAM    : {} MiB", self.ram_mib);
        println!("Kernel : {}", self.kernel);
        println!("OS     : {}", self.os);
        println!("===================================================");
        println!();
    }
}

fn read_cpu_model() -> String {
    if let Ok(s) = fs::read_to_string("/proc/cpuinfo") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("model name") {
                if let Some(idx) = rest.find(':') {
                    return rest[idx + 1..].trim().to_string();
                }
            }
        }
        // Fallback: ARM uses "Hardware" or "Processor"
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("Hardware") {
                if let Some(idx) = rest.find(':') {
                    return rest[idx + 1..].trim().to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

fn read_mem_total_mib() -> u64 {
    if let Ok(s) = fs::read_to_string("/proc/meminfo") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kib: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                return kib / 1024;
            }
        }
    }
    0
}

fn read_kernel() -> String {
    if let Ok(s) = fs::read_to_string("/proc/version") {
        // First two whitespace-separated tokens after "Linux version"
        let trimmed = s.trim();
        if let Some(rest) = trimmed.strip_prefix("Linux version ") {
            return rest
                .split_whitespace()
                .next()
                .unwrap_or("unknown")
                .to_string();
        }
        return trimmed.to_string();
    }
    "unknown".to_string()
}

fn read_os() -> String {
    if let Ok(s) = fs::read_to_string("/etc/os-release") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
                return rest.trim_matches('"').to_string();
            }
        }
    }
    "unknown".to_string()
}

/// Compute a stable, anonymized per-machine identifier.
///
/// Seeded from `/etc/machine-id` (or the D-Bus machine-id) when available, so
/// the raw id never leaves the box — only its hash does. Falls back to a hash
/// of stable hardware facts when no machine-id file exists. Deterministic: the
/// same machine yields the same 12-hex string on every run, which the score
/// server uses to dedupe submissions into one leaderboard row per machine.
fn read_machine_id() -> String {
    let seed = read_first_nonempty(&["/etc/machine-id", "/var/lib/dbus/machine-id"])
        .unwrap_or_else(|| format!("{}|{}|{}", read_cpu_model(), read_mem_total_mib(), read_os()));
    fnv1a_hex(&format!("crux:{seed}"))
}

fn read_first_nonempty(paths: &[&str]) -> Option<String> {
    for p in paths {
        if let Ok(s) = fs::read_to_string(p) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// 12-hex-char FNV-1a digest — enough to identify a machine, no crypto dep.
fn fnv1a_hex(s: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let hex = format!("{hash:016x}");
    hex[..12].to_string()
}
