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

/// Run a suite by name and return the human-readable report as a String.
/// `kind` is one of: `all`, `cpu`, `mem`, `net`, `disk`, `info`. This is the
/// single entry point the GUI uses so it shares the CLI's exact measurements.
pub fn run_named(kind: &str) -> String {
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
        duration: Duration::from_secs(10),
        runs: 5,
        streams: 4,
        dir: None,
    };
    let info = sysinfo::SysInfo::collect();
    let full = bench::run(suite, &cfg, info);
    bench::format_results(&full, &cfg)
}
