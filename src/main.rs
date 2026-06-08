// CRUCIBLE — `crux`: a host-native benchmark + deep system-info tool.
//
//   crux                      run the full benchmark suite (default)
//   crux bench [cpu|mem|net|disk|all]
//   crux info                 deep system report (fastfetch, but deeper)
//
// Invoking the binary as `sysinfo` (e.g. via the install symlink) is equivalent
// to `crux info`.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

mod affinity;
mod bench;
mod report;
mod stats;
mod sysinfo;
mod upload;

use bench::{Config, Suite};

#[derive(Parser)]
#[command(
    name = "crux",
    about = "CRUCIBLE — host-native CPU/memory/network/storage benchmark & deep system report",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(long, global = true, help = "Output machine-readable JSON")]
    json: bool,

    #[arg(long, global = true, default_value = "10", help = "Seconds per CPU test")]
    duration: u64,

    #[arg(long, global = true, default_value = "5", help = "Runs per CPU test")]
    runs: usize,

    #[arg(long, global = true, default_value = "4", help = "Parallel download streams")]
    streams: usize,

    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Scratch directory for the disk test (default: CWD)"
    )]
    dir: Option<PathBuf>,

    #[arg(long, global = true, help = "Do not upload results (upload is on by default)")]
    no_upload: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Run benchmarks (this is the default when no subcommand is given)
    Bench {
        #[command(subcommand)]
        suite: Option<BenchSuite>,
    },
    /// Deep system report (fastfetch, but deeper) — no benchmarking, no upload
    Info,
}

#[derive(Subcommand, Clone, Copy)]
enum BenchSuite {
    /// CPU, memory, network and storage (default)
    All,
    /// CPU only
    Cpu,
    /// Memory bandwidth only
    Mem,
    /// Network only
    Net,
    /// Storage only
    Disk,
}

impl From<Option<BenchSuite>> for Suite {
    fn from(s: Option<BenchSuite>) -> Self {
        match s {
            None | Some(BenchSuite::All) => Suite::All,
            Some(BenchSuite::Cpu) => Suite::Cpu,
            Some(BenchSuite::Mem) => Suite::Mem,
            Some(BenchSuite::Net) => Suite::Net,
            Some(BenchSuite::Disk) => Suite::Disk,
        }
    }
}

/// True if the binary was invoked under the `sysinfo` alias name.
fn invoked_as_sysinfo() -> bool {
    std::env::args_os()
        .next()
        .map(PathBuf::from)
        .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
        .map(|name| name == "sysinfo")
        .unwrap_or(false)
}

fn main() {
    // `sysinfo` alias short-circuits to the report.
    if invoked_as_sysinfo() {
        report::run();
        return;
    }

    let cli = Cli::parse();

    if let Some(Command::Info) = cli.command {
        report::run();
        return;
    }

    let suite: Suite = match cli.command {
        Some(Command::Bench { suite }) => suite.into(),
        None => Suite::All,
        Some(Command::Info) => unreachable!(),
    };

    let cfg = Config {
        duration: Duration::from_secs(cli.duration),
        runs: cli.runs,
        streams: cli.streams,
        dir: cli.dir.clone(),
    };

    let info = sysinfo::SysInfo::collect();
    if cli.json {
        println!(
            "# CRUCIBLE crux — {} | {} cores | {} MiB RAM | {} | {}",
            info.cpu_model, info.logical_cores, info.ram_mib, info.kernel, info.os
        );
    } else {
        info.print();
    }

    let full = bench::run(suite, &cfg, info);

    if cli.json {
        match serde_json::to_string_pretty(&full) {
            Ok(s) => println!("{}", s),
            Err(e) => eprintln!("JSON serialization failed: {}", e),
        }
    } else {
        bench::print_results(&full, &cfg);
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
