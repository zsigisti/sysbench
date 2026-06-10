// A flat, front-end-friendly digest of a full results JSON.
//
// `FullResults` is a deep tree; for history rows, comparisons, and leaderboard
// entries we want a handful of headline numbers. This module extracts them from
// a parsed `serde_json::Value` so it stays decoupled from the exact struct
// layout (older/newer result files still parse).

use serde::Serialize;
use serde_json::Value;

/// Headline metrics pulled out of a full results JSON.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Summary {
    pub cpu_model: String,
    pub cores: u64,
    pub ram_mib: u64,
    pub kernel: String,
    pub os: String,

    pub composite_st: Option<f64>,
    pub composite_mt: Option<f64>,
    pub speedup: Option<f64>,

    pub mem_triad_gbs: Option<f64>,
    pub net_down_mbps: Option<f64>,
    pub net_up_mbps: Option<f64>,
    pub net_latency_ms: Option<f64>,
    pub disk_seq_write_mbs: Option<f64>,
    pub disk_seq_read_mbs: Option<f64>,

    // GPU render benchmark (GUI-only; absent from CLI runs).
    pub render_score: Option<f64>,
    pub render_fps: Option<f64>,
}

fn f(v: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = v;
    for k in path {
        cur = cur.get(k)?;
    }
    cur.as_f64()
}

fn s(v: &Value, path: &[&str]) -> String {
    let mut cur = v;
    for k in path {
        match cur.get(k) {
            Some(n) => cur = n,
            None => return String::new(),
        }
    }
    cur.as_str().unwrap_or("").to_string()
}

impl Summary {
    /// Parse a summary from a results JSON string. Missing fields stay `None`.
    pub fn from_json(json: &str) -> Result<Summary, String> {
        let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        Ok(Summary::from_value(&v))
    }

    pub fn from_value(v: &Value) -> Summary {
        Summary {
            cpu_model: s(v, &["sysinfo", "cpu_model"]),
            cores: f(v, &["sysinfo", "logical_cores"]).unwrap_or(0.0) as u64,
            ram_mib: f(v, &["sysinfo", "ram_mib"]).unwrap_or(0.0) as u64,
            kernel: s(v, &["sysinfo", "kernel"]),
            os: s(v, &["sysinfo", "os"]),

            composite_st: f(v, &["cpu", "composite_st"]),
            composite_mt: f(v, &["cpu", "composite_mt"]),
            speedup: f(v, &["cpu", "speedup"]),

            mem_triad_gbs: f(v, &["mem", "triad_gbs"]),
            net_down_mbps: f(v, &["net", "download_mbps", "Ok"]),
            net_up_mbps: f(v, &["net", "upload_mbps", "Ok"]),
            net_latency_ms: f(v, &["net", "latency", "Ok", "avg_ms"]),
            disk_seq_write_mbs: f(v, &["disk", "seq_write_mbs"]),
            disk_seq_read_mbs: f(v, &["disk", "seq_read_mbs"]),

            render_score: f(v, &["render", "score"]),
            render_fps: f(v, &["render", "fps"]),
        }
    }

    /// A one-line label for lists/menus.
    pub fn headline(&self) -> String {
        let cpu = if self.cpu_model.is_empty() {
            "unknown CPU".to_string()
        } else {
            self.cpu_model.clone()
        };
        match (self.composite_mt, self.composite_st) {
            (Some(mt), Some(st)) => format!("{} — MT {:.0} / ST {:.0}", cpu, mt, st),
            (Some(mt), None) => format!("{} — MT {:.0}", cpu, mt),
            (None, Some(st)) => format!("{} — ST {:.0}", cpu, st),
            _ => cpu,
        }
    }
}
