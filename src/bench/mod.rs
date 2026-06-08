// Benchmark orchestration: runs the selected suites and renders human output.

pub mod cpu;
pub mod disk;
pub mod mem;
pub mod net;

use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

use crate::sysinfo::SysInfo;

/// Which suite(s) to run.
#[derive(Clone, Copy)]
pub enum Suite {
    All,
    Cpu,
    Mem,
    Net,
    Disk,
}

/// Tunables shared across suites.
#[derive(Clone)]
pub struct Config {
    pub duration: Duration,
    pub runs: usize,
    pub streams: usize,
    pub dir: Option<PathBuf>,
}

#[derive(Serialize)]
pub struct FullResults {
    pub sysinfo: SysInfo,
    pub cpu: Option<cpu::CpuResults>,
    pub mem: Option<mem::MemResults>,
    pub net: Option<net::NetResults>,
    pub disk: Option<DiskOutcome>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum DiskOutcome {
    Ok(disk::DiskResults),
    Err { error: String },
}

fn run_disk(cfg: &Config, ram_mib: u64) -> DiskOutcome {
    match disk::run(ram_mib, cfg.dir.clone()) {
        Ok(d) => DiskOutcome::Ok(d),
        Err(e) => DiskOutcome::Err { error: e.to_string() },
    }
}

/// Run the requested suite(s) and collect results alongside `info`.
pub fn run(suite: Suite, cfg: &Config, info: SysInfo) -> FullResults {
    let mut r = FullResults {
        sysinfo: info,
        cpu: None,
        mem: None,
        net: None,
        disk: None,
    };
    let ram_mib = r.sysinfo.ram_mib;
    match suite {
        Suite::All => {
            r.cpu = Some(cpu::run(cfg.duration, cfg.runs));
            r.mem = Some(mem::run());
            r.net = Some(net::run(cfg.streams));
            r.disk = Some(run_disk(cfg, ram_mib));
        }
        Suite::Cpu => r.cpu = Some(cpu::run(cfg.duration, cfg.runs)),
        Suite::Mem => r.mem = Some(mem::run()),
        Suite::Net => r.net = Some(net::run(cfg.streams)),
        Suite::Disk => r.disk = Some(run_disk(cfg, ram_mib)),
    }
    r
}

// ============================================================
// Human-readable printers
// ============================================================

pub fn print_results(full: &FullResults, cfg: &Config) {
    if let Some(c) = &full.cpu {
        print_cpu(c, cfg.duration.as_secs(), cfg.runs);
    }
    if let Some(m) = &full.mem {
        print_mem(m);
    }
    if let Some(n) = &full.net {
        print_net(n, cfg.streams);
    }
    if let Some(d) = &full.disk {
        print_disk(d);
    }
    println!("===================================================");
}

fn fmt_score(score: &cpu::TestScore) -> String {
    format!("{:>8.2} ± {:>5.2} {}", score.median, score.stddev, score.unit)
}

fn print_cpu(r: &cpu::CpuResults, dur: u64, runs: usize) {
    println!();
    println!("[1] CPU Benchmark  ({}s/run, {} runs, median ± stddev)", dur, runs);
    println!("  --- Single-threaded ---");
    println!("  BBP-π       : {}", fmt_score(&r.bbp_st));
    println!("  SHA-256     : {}", fmt_score(&r.sha256_st));
    println!("  MatMul      : {}", fmt_score(&r.matmul_st));
    println!("  LZ4         : {}", fmt_score(&r.lz4_st));
    println!("  Sort        : {}", fmt_score(&r.sort_st));
    println!("  Composite ST score: {:.0}", r.composite_st);
    println!();
    println!("  --- Multi-threaded ({} threads) ---", r.threads);
    println!("  BBP-π       : {}", fmt_score(&r.bbp_mt));
    println!("  SHA-256     : {}", fmt_score(&r.sha256_mt));
    println!("  MatMul      : {}", fmt_score(&r.matmul_mt));
    println!("  LZ4         : {}", fmt_score(&r.lz4_mt));
    println!("  Sort        : {}", fmt_score(&r.sort_mt));
    println!("  Composite MT score: {:.0}", r.composite_mt);
    println!("  Speedup: {:.2}×", r.speedup);

    let pairs: [(&str, &cpu::TestScore); 10] = [
        ("BBP-ST", &r.bbp_st),
        ("SHA256-ST", &r.sha256_st),
        ("MatMul-ST", &r.matmul_st),
        ("LZ4-ST", &r.lz4_st),
        ("Sort-ST", &r.sort_st),
        ("BBP-MT", &r.bbp_mt),
        ("SHA256-MT", &r.sha256_mt),
        ("MatMul-MT", &r.matmul_mt),
        ("LZ4-MT", &r.lz4_mt),
        ("Sort-MT", &r.sort_mt),
    ];
    let mut warned = false;
    for (name, s) in &pairs {
        if s.high_variance {
            if !warned {
                println!();
                warned = true;
            }
            let pct = if s.median > 0.0 {
                100.0 * s.stddev / s.median
            } else {
                0.0
            };
            println!("  [!] {} high variance ({:.1}%) — possible thermal throttling", name, pct);
        }
    }
}

fn print_mem(m: &mem::MemResults) {
    println!();
    println!("[2] Memory Bandwidth (STREAM, 256 MiB arrays)");
    println!("  Copy  : {:>6.2} GB/s", m.copy_gbs);
    println!("  Scale : {:>6.2} GB/s", m.scale_gbs);
    println!("  Add   : {:>6.2} GB/s", m.add_gbs);
    println!("  Triad : {:>6.2} GB/s", m.triad_gbs);
}

fn print_net(n: &net::NetResults, streams: usize) {
    println!();
    println!("[3] Network — Cloudflare");
    match &n.latency {
        Ok(l) => println!(
            "  Latency : {:.2} ms avg | {:.2} min | {:.2} max | ±{:.2} stddev | {:.2} jitter",
            l.avg_ms, l.min_ms, l.max_ms, l.stddev_ms, l.jitter_ms
        ),
        Err(e) => println!("  Latency : failed: {}", e),
    }
    match &n.download_mbps {
        Ok(v) => println!("  Download: {:.2} Mbps  ({} streams, 10s measured)", v, streams),
        Err(e) => println!("  Download: failed: {}", e),
    }
    match &n.upload_mbps {
        Ok(v) => println!("  Upload  : {:.2} Mbps", v),
        Err(e) => println!("  Upload  : failed: {}", e),
    }
}

fn print_disk(d: &DiskOutcome) {
    println!();
    match d {
        DiskOutcome::Ok(r) => {
            #[cfg(target_os = "linux")]
            let mode = "O_DIRECT";
            #[cfg(not(target_os = "linux"))]
            let mode = "buffered";
            println!("[4] Storage  (file: {} MiB, {}, dir: {})", r.file_size_mib, mode, r.dir);
            if r.on_tmpfs {
                println!("  [!] scratch dir is on tmpfs (RAM) — these are MEMORY speeds, not disk.");
                println!("      Re-run from a real disk: `crux bench disk --dir /path/on/disk`");
            }
            println!("  Seq Write  : {:>8.1} MB/s", r.seq_write_mbs);
            if r.seq_read_cached {
                println!("  Seq Read   : {:>8.1} MB/s  [!] likely cached", r.seq_read_mbs);
            } else {
                println!("  Seq Read   : {:>8.1} MB/s", r.seq_read_mbs);
            }
            println!(
                "  Rand 4K R  :   p50={:>4.0} µs   p99={:>4.0} µs",
                r.rand_read_p50_us, r.rand_read_p99_us
            );
            println!(
                "  Rand 4K W  :   p50={:>4.0} µs   p99={:>4.0} µs",
                r.rand_write_p50_us, r.rand_write_p99_us
            );
        }
        DiskOutcome::Err { error } => {
            println!("[4] Storage  : failed: {}", error);
        }
    }
}
