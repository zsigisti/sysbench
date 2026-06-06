// sysbench — multi-module CPU / Memory / Network / Storage benchmark.

use clap::{Parser, Subcommand};
use serde::Serialize;
use std::time::Duration;

mod cpu;
mod disk;
mod mem;
mod net;
mod stats;
mod sysinfo;
mod upload;

#[derive(Parser)]
#[command(name = "sysbench", about = "System benchmark", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(long, global = true, help = "Output JSON")]
    json: bool,

    #[arg(long, global = true, default_value = "10", help = "Duration per test (seconds)")]
    duration: u64,

    #[arg(long, global = true, default_value = "5", help = "Runs per test")]
    runs: usize,

    #[arg(long, global = true, default_value = "4", help = "Parallel download streams")]
    streams: usize,

    #[arg(long, global = true, help = "Do not upload results (upload is on by default)")]
    no_upload: bool,
}

#[derive(Subcommand)]
enum Commands {
    All,
    Cpu,
    Mem,
    Net,
    Disk,
}

#[derive(Serialize)]
struct FullResults {
    sysinfo: sysinfo::SysInfo,
    cpu: Option<cpu::CpuResults>,
    mem: Option<mem::MemResults>,
    net: Option<net::NetResults>,
    disk: Option<DiskOutcome>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum DiskOutcome {
    Ok(disk::DiskResults),
    Err { error: String },
}

fn main() {
    let cli = Cli::parse();
    let cmd = cli.command.unwrap_or(Commands::All);
    let dur = Duration::from_secs(cli.duration);

    let info = sysinfo::SysInfo::collect();

    if cli.json {
        // In JSON mode, emit sysinfo as a comment line above the JSON.
        println!(
            "# sysbench v0.2 — {} | {} cores | {} MiB RAM | {} | {}",
            info.cpu_model, info.logical_cores, info.ram_mib, info.kernel, info.os
        );
    } else {
        info.print();
    }

    let mut cpu_res: Option<cpu::CpuResults> = None;
    let mut mem_res: Option<mem::MemResults> = None;
    let mut net_res: Option<net::NetResults> = None;
    let mut disk_res: Option<DiskOutcome> = None;

    match cmd {
        Commands::All => {
            cpu_res = Some(cpu::run(dur, cli.runs));
            mem_res = Some(mem::run());
            net_res = Some(net::run(cli.streams));
            disk_res = Some(match disk::run(info.ram_mib) {
                Ok(d) => DiskOutcome::Ok(d),
                Err(e) => DiskOutcome::Err { error: e.to_string() },
            });
        }
        Commands::Cpu => {
            cpu_res = Some(cpu::run(dur, cli.runs));
        }
        Commands::Mem => {
            mem_res = Some(mem::run());
        }
        Commands::Net => {
            net_res = Some(net::run(cli.streams));
        }
        Commands::Disk => {
            disk_res = Some(match disk::run(info.ram_mib) {
                Ok(d) => DiskOutcome::Ok(d),
                Err(e) => DiskOutcome::Err { error: e.to_string() },
            });
        }
    }

    let full = FullResults {
        sysinfo: info,
        cpu: cpu_res,
        mem: mem_res,
        net: net_res,
        disk: disk_res,
    };

    if cli.json {
        match serde_json::to_string_pretty(&full) {
            Ok(s) => println!("{}", s),
            Err(e) => eprintln!("JSON serialization failed: {}", e),
        }
    } else {
        if let Some(c) = &full.cpu {
            print_cpu(c, cli.duration, cli.runs);
        }
        if let Some(m) = &full.mem {
            print_mem(m);
        }
        if let Some(n) = &full.net {
            print_net(n, cli.streams);
        }
        if let Some(d) = &full.disk {
            print_disk(d);
        }
        println!("===================================================");
    }

    if !cli.no_upload {
        match serde_json::to_string_pretty(&full) {
            Ok(json) => {
                println!();
                print!("Uploading results to paste.rs ... ");
                match upload::upload(&json) {
                    Ok(url) => println!("done\n  Results: {}\n  (use --no-upload to disable)", url),
                    Err(e) => eprintln!("failed: {}", e),
                }
            }
            Err(e) => eprintln!("Upload skipped (JSON error): {}", e),
        }
    }
}

// ============================================================
// Human-readable printers
// ============================================================

fn fmt_score(score: &cpu::TestScore) -> String {
    format!("{:>8.2} ± {:>5.2} {}", score.median, score.stddev, score.unit)
}

fn print_cpu(r: &cpu::CpuResults, dur: u64, runs: usize) {
    println!();
    println!(
        "[1] CPU Benchmark  ({}s/run, {} runs, median ± stddev)",
        dur, runs
    );
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
            println!(
                "  [!] {} high variance ({:.1}%) — possible thermal throttling",
                name,
                if s.median > 0.0 {
                    100.0 * s.stddev / s.median
                } else {
                    0.0
                }
            );
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
            println!("[4] Storage  (file: {} MiB, {})", r.file_size_mib, mode);
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
