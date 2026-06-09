// CRUCIBLE — `crux`: a host-native benchmark + deep system-info tool.
//
//   crux                      run the full benchmark suite (default)
//   crux bench [cpu|mem|net|disk|all]
//   crux info                 deep system report (fastfetch, but deeper)
//   crux submit               run the full suite and submit to the score server
//   crux compare <a> <b>      diff two saved result JSON files
//   crux history [show <id>]  list / show locally recorded runs
//   crux uninstall            remove everything the installer placed
//
// Invoking the binary as `sysinfo` (e.g. via the install symlink) is equivalent
// to `crux info`.

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;
use std::time::Duration;

use crucible::bench::{self, Config, Suite};
use crucible::{compare, history, report, summary::Summary, sysinfo, uninstall, upload};

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

    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "Also write the report (or --json) to FILE"
    )]
    output: Option<PathBuf>,

    #[arg(long, global = true, help = "Do not share results (sharing is on by default)")]
    no_upload: bool,

    #[arg(long, global = true, help = "Do not record this run to local history")]
    no_history: bool,
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
    /// Run the full suite and submit it to the CRUCIBLE score server
    Submit,
    /// Compare two saved result JSON files (or history ids)
    Compare {
        /// First run: a JSON file path or a history id
        a: String,
        /// Second run: a JSON file path or a history id
        b: String,
    },
    /// List or show locally recorded runs
    History {
        #[command(subcommand)]
        cmd: Option<HistoryCmd>,
    },
    /// Remove everything the installer placed (binaries, man page, completions, GUI)
    Uninstall {
        /// Also delete local run history and data (~/.local/share/crucible)
        #[arg(long)]
        purge_data: bool,
    },
    /// Print a roff man page to stdout (used by packagers)
    #[command(hide = true)]
    Man,
    /// Print a shell completion script to stdout (used by packagers)
    #[command(hide = true)]
    Completions {
        /// Target shell
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum HistoryCmd {
    /// List recorded runs (default)
    List,
    /// Print the stored JSON for a run id
    Show {
        /// Run id (see `crux history`)
        id: String,
    },
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

// Free function rather than `impl From` because `Suite` lives in the external
// `crucible` crate (orphan rule forbids the trait impl here).
fn to_suite(s: Option<BenchSuite>) -> Suite {
    match s {
        None | Some(BenchSuite::All) => Suite::All,
        Some(BenchSuite::Cpu) => Suite::Cpu,
        Some(BenchSuite::Mem) => Suite::Mem,
        Some(BenchSuite::Net) => Suite::Net,
        Some(BenchSuite::Disk) => Suite::Disk,
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

    // Commands that produce output and exit immediately.
    match &cli.command {
        Some(Command::Info) => {
            report::run();
            return;
        }
        Some(Command::Man) => {
            let _ = clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout());
            return;
        }
        Some(Command::Completions { shell }) => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(*shell, &mut cmd, name, &mut std::io::stdout());
            return;
        }
        Some(Command::Compare { a, b }) => {
            std::process::exit(cmd_compare(a, b));
        }
        Some(Command::History { cmd }) => {
            std::process::exit(cmd_history(cmd.as_ref()));
        }
        Some(Command::Uninstall { purge_data }) => {
            cmd_uninstall(*purge_data);
            return;
        }
        _ => {}
    }

    // Remaining commands run a benchmark.
    let force_submit = matches!(cli.command, Some(Command::Submit));
    let suite: Suite = match &cli.command {
        Some(Command::Bench { suite }) => to_suite(*suite),
        Some(Command::Submit) | None => Suite::All,
        _ => unreachable!(),
    };
    run_benchmark(&cli, suite, force_submit);
}

/// Run the selected suite, print/write the report, share, and record history.
fn run_benchmark(cli: &Cli, suite: Suite, force_submit: bool) {
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
    let json = bench::to_json(&full).unwrap_or_default();

    if cli.json {
        println!("{}", json);
    } else {
        bench::print_results(&full, &cfg);
    }

    // --output: also write the report (or JSON) to a file.
    if let Some(path) = &cli.output {
        let body = if cli.json {
            json.clone()
        } else {
            bench::format_results(&full, &cfg)
        };
        match std::fs::write(path, body) {
            Ok(()) => println!("Saved to {}", path.display()),
            Err(e) => eprintln!("Could not write {}: {}", path.display(), e),
        }
    }

    // Record to local history (for `crux history` / compare / GUI analysis).
    if !cli.no_history {
        match history::record(&json) {
            Ok(e) => println!("Recorded run {} ({})", e.id, history::fmt_time(e.unix_time)),
            Err(e) => eprintln!("Could not record history: {}", e),
        }
    }

    // Share: server by default (paste.rs fallback); `submit` forces the server.
    if !cli.no_upload {
        println!();
        if force_submit {
            print!("Submitting to {} ... ", upload::server_base());
            match upload::submit(&json) {
                Ok(url) => println!("done\n  Results: {}", url),
                Err(e) => eprintln!("failed: {}", e),
            }
        } else {
            print!("Sharing results ... ");
            match upload::share(&json) {
                Ok(s) => println!(
                    "done\n  Results: {}  ({})\n  (use --no-upload to disable)",
                    s.url, s.backend
                ),
                Err(e) => eprintln!("failed: {}", e),
            }
        }
    }
}

fn load_run(id_or_path: &str) -> Result<String, String> {
    // A real file wins; otherwise treat it as a history id.
    if std::path::Path::new(id_or_path).is_file() {
        std::fs::read_to_string(id_or_path).map_err(|e| e.to_string())
    } else {
        history::load(id_or_path)
    }
}

fn cmd_compare(a: &str, b: &str) -> i32 {
    let (ja, jb) = match (load_run(a), load_run(b)) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(e), _) => {
            eprintln!("error: {}: {}", a, e);
            return 1;
        }
        (_, Err(e)) => {
            eprintln!("error: {}: {}", b, e);
            return 1;
        }
    };
    let color = atty_stdout();
    match compare::render_json(&ja, &jb, color) {
        Ok(s) => {
            print!("{}", s);
            0
        }
        Err(e) => {
            eprintln!("error: {}", e);
            1
        }
    }
}

fn cmd_history(cmd: Option<&HistoryCmd>) -> i32 {
    match cmd {
        Some(HistoryCmd::Show { id }) => match history::load(id) {
            Ok(json) => {
                print!("{}", json);
                0
            }
            Err(e) => {
                eprintln!("error: {}", e);
                1
            }
        },
        _ => {
            let entries = history::list();
            if entries.is_empty() {
                println!("No recorded runs yet. Run `crux` to create one.");
                println!("(history lives in {})", history::history_dir().display());
                return 0;
            }
            let (h_id, h_when, h_res) = ("ID", "WHEN", "RESULT");
            println!("{:<26} {:<22} {}", h_id, h_when, h_res);
            for e in entries {
                let s: &Summary = &e.summary;
                let mt = s.composite_mt.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into());
                let st = s.composite_st.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into());
                println!(
                    "{:<26} {:<22} {} (MT {} / ST {})",
                    e.id,
                    history::fmt_time(e.unix_time),
                    if s.cpu_model.is_empty() { "—" } else { &s.cpu_model },
                    mt,
                    st
                );
            }
            println!("\nCompare two:  crux compare <id-a> <id-b>");
            0
        }
    }
}

fn cmd_uninstall(purge_data: bool) {
    println!("Removing CRUCIBLE files installed by install.sh ...");
    let report = uninstall::run(purge_data);
    for p in &report.removed {
        println!("  removed {}", p);
    }
    for p in &report.failed {
        eprintln!("  could not remove {} (try sudo)", p);
    }
    println!("{}", report.summary());
    if !purge_data {
        println!("(local history kept — add --purge-data to delete it)");
    }
}

/// Cheap stdout-is-a-tty check for colour, without an extra dependency.
fn atty_stdout() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: isatty just inspects the fd.
        unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}
