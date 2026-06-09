// Compare two benchmark runs.
//
// Works off the flat `Summary` digest so it tolerates schema differences and
// partial runs. Produces a human table (CLI + GUI) and a machine-readable list
// of deltas (GUI charts / analysis).

use serde::Serialize;

use crate::summary::Summary;

/// One metric compared across two runs.
#[derive(Debug, Clone, Serialize)]
pub struct Delta {
    pub metric: String,
    pub unit: String,
    pub a: Option<f64>,
    pub b: Option<f64>,
    /// Percent change from A to B (positive = B larger). `None` if either side
    /// is missing or A is zero.
    pub pct: Option<f64>,
    /// True when a *larger* number is better for this metric (so the GUI can
    /// colour wins/losses correctly — e.g. latency is lower-is-better).
    pub higher_is_better: bool,
}

/// All comparable metrics between two summaries.
pub fn deltas(a: &Summary, b: &Summary) -> Vec<Delta> {
    let mut out = Vec::new();
    let mut push = |metric: &str, unit: &str, hib: bool, av: Option<f64>, bv: Option<f64>| {
        let pct = match (av, bv) {
            (Some(x), Some(y)) if x != 0.0 => Some((y - x) / x.abs() * 100.0),
            _ => None,
        };
        out.push(Delta {
            metric: metric.to_string(),
            unit: unit.to_string(),
            a: av,
            b: bv,
            pct,
            higher_is_better: hib,
        });
    };
    push("CPU composite (ST)", "", true, a.composite_st, b.composite_st);
    push("CPU composite (MT)", "", true, a.composite_mt, b.composite_mt);
    push("MT speedup", "×", true, a.speedup, b.speedup);
    push("Memory Triad", "GB/s", true, a.mem_triad_gbs, b.mem_triad_gbs);
    push("Net download", "Mbps", true, a.net_down_mbps, b.net_down_mbps);
    push("Net upload", "Mbps", true, a.net_up_mbps, b.net_up_mbps);
    push("Net latency", "ms", false, a.net_latency_ms, b.net_latency_ms);
    push("Disk seq write", "MB/s", true, a.disk_seq_write_mbs, b.disk_seq_write_mbs);
    push("Disk seq read", "MB/s", true, a.disk_seq_read_mbs, b.disk_seq_read_mbs);
    out
}

fn cell(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.2}", x),
        None => "—".to_string(),
    }
}

/// Render a comparison table. `color` adds ANSI green/red on the % column.
pub fn render(a: &Summary, b: &Summary, color: bool) -> String {
    use std::fmt::Write as _;
    let (g, r, z) = if color {
        ("\x1b[32m", "\x1b[31m", "\x1b[0m")
    } else {
        ("", "", "")
    };
    let mut s = String::new();
    let _ = writeln!(s, "Comparison");
    let _ = writeln!(s, "  A: {}", a.headline());
    let _ = writeln!(s, "  B: {}", b.headline());
    let _ = writeln!(s);
    let _ = writeln!(s, "  {:<22} {:>12} {:>12} {:>10}", "Metric", "A", "B", "Change");
    let _ = writeln!(s, "  {:-<22} {:->12} {:->12} {:->10}", "", "", "", "");
    for d in deltas(a, b) {
        let change = match d.pct {
            Some(p) => {
                let better = if d.higher_is_better { p > 0.0 } else { p < 0.0 };
                let col = if p.abs() < 0.05 { z } else if better { g } else { r };
                format!("{}{:+.1}%{}", col, p, z)
            }
            None => "—".to_string(),
        };
        let label = if d.unit.is_empty() {
            d.metric.clone()
        } else {
            format!("{} ({})", d.metric, d.unit)
        };
        let _ = writeln!(s, "  {:<22} {:>12} {:>12} {:>10}", label, cell(d.a), cell(d.b), change);
    }
    s
}

/// Compare two raw JSON strings.
pub fn render_json(a_json: &str, b_json: &str, color: bool) -> Result<String, String> {
    let a = Summary::from_json(a_json)?;
    let b = Summary::from_json(b_json)?;
    Ok(render(&a, &b, color))
}
