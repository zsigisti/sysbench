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
}

impl SysInfo {
    pub fn collect() -> Self {
        SysInfo {
            cpu_model: read_cpu_model(),
            logical_cores: thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            ram_mib: read_mem_total_mib(),
            kernel: read_kernel(),
            os: read_os(),
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
