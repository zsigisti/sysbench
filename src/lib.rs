//! CRUCIBLE engine — the shared library behind the `crux` CLI and the GUI.
//!
//! Everything benchmark- and report-related lives here so both front-ends use
//! exactly the same measurement code. The CLI ([`crux`](../main.rs)) and the
//! GUI (`gui/`) are thin shells over these modules.

pub mod affinity;
pub mod bench;
pub mod report;
pub mod stats;
pub mod sysinfo;
pub mod upload;

use std::time::Duration;

/// Tunables for a GUI/library run. Mirrors the CLI's global flags.
#[derive(Clone, Copy)]
pub struct RunOpts {
    pub duration_secs: u64,
    pub runs: usize,
    pub streams: usize,
}

impl Default for RunOpts {
    fn default() -> Self {
        Self {
            duration_secs: 10,
            runs: 5,
            streams: 4,
        }
    }
}

/// Run a single suite by name with explicit options and return the
/// human-readable report as a String. `kind` is one of `cpu`, `mem`, `net`,
/// `disk`, `all`, `info`. This is the shared entry point the GUI uses so it
/// produces exactly the CLI's measurements.
pub fn run_suite_text(kind: &str, opts: &RunOpts) -> String {
    if kind == "info" {
        return report::render(false);
    }
    let suite = match kind {
        "cpu" => bench::Suite::Cpu,
        "mem" => bench::Suite::Mem,
        "net" => bench::Suite::Net,
        "disk" => bench::Suite::Disk,
        _ => bench::Suite::All,
    };
    let cfg = bench::Config {
        duration: Duration::from_secs(opts.duration_secs.max(1)),
        runs: opts.runs.max(1),
        streams: opts.streams.max(1),
        dir: None,
    };
    let info = sysinfo::SysInfo::collect();
    let full = bench::run(suite, &cfg, info);
    bench::format_results(&full, &cfg)
}

/// Convenience: [`run_suite_text`] with default options.
pub fn run_named(kind: &str) -> String {
    run_suite_text(kind, &RunOpts::default())
}
