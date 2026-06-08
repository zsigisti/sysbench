// Formatting helpers and small /proc, /sys readers for the `crux info` report.

use std::fs;
use std::process::Command;

/// ANSI styling that honours NO_COLOR and dumb terminals.
pub struct Style {
    pub on: bool,
}

impl Style {
    pub fn new() -> Self {
        let on = std::env::var_os("NO_COLOR").is_none()
            && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true);
        Style { on }
    }
    /// Construct with an explicit colour setting (the GUI forces `false`).
    pub fn with(on: bool) -> Self {
        Style { on }
    }
    pub fn paint(&self, code: &str, s: &str) -> String {
        if self.on {
            format!("\x1b[{}m{}\x1b[0m", code, s)
        } else {
            s.to_string()
        }
    }
    pub fn title(&self, s: &str) -> String {
        self.paint("1;36", s) // bold cyan
    }
    pub fn key(&self, s: &str) -> String {
        self.paint("1;32", s) // bold green
    }
    pub fn accent(&self, s: &str) -> String {
        self.paint("1;35", s) // bold magenta
    }
}

pub fn human_bytes(b: u64) -> String {
    const U: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", b, U[i])
    } else {
        format!("{:.2} {}", v, U[i])
    }
}

/// A coloured usage bar: green < 75%, yellow < 90%, red otherwise.
pub fn bar(used: u64, total: u64, width: usize, st: &Style) -> String {
    if total == 0 {
        return String::new();
    }
    let frac = (used as f64 / total as f64).clamp(0.0, 1.0);
    let filled = (frac * width as f64).round() as usize;
    let pct = frac * 100.0;
    let colour = if pct > 90.0 {
        "1;31"
    } else if pct > 75.0 {
        "1;33"
    } else {
        "1;32"
    };
    let fill = st.paint(colour, &"━".repeat(filled));
    let empty = "─".repeat(width.saturating_sub(filled));
    format!("[{}{}] {:.0}%", fill, empty, pct)
}

/// Compress an IPv6 address with the longest zero-run replaced by `::`.
pub fn format_ipv6(b: &[u8; 16]) -> String {
    let groups: [u16; 8] = std::array::from_fn(|i| ((b[2 * i] as u16) << 8) | b[2 * i + 1] as u16);
    let (mut best_start, mut best_len) = (0usize, 0usize);
    let (mut cur_start, mut cur_len) = (0usize, 0usize);
    for (i, &g) in groups.iter().enumerate() {
        if g == 0 {
            if cur_len == 0 {
                cur_start = i;
            }
            cur_len += 1;
            if cur_len > best_len {
                best_len = cur_len;
                best_start = cur_start;
            }
        } else {
            cur_len = 0;
        }
    }
    if best_len < 2 {
        return groups
            .iter()
            .map(|g| format!("{:x}", g))
            .collect::<Vec<_>>()
            .join(":");
    }
    let mut out = String::new();
    let mut i = 0;
    while i < 8 {
        if i == best_start {
            out.push_str("::");
            i += best_len;
            continue;
        }
        if !out.is_empty() && !out.ends_with(':') {
            out.push(':');
        }
        out.push_str(&format!("{:x}", groups[i]));
        i += 1;
    }
    out
}

// ---------- tiny file / command helpers ----------

pub fn read(p: &str) -> Option<String> {
    fs::read_to_string(p).ok()
}

pub fn first_line(p: &str) -> Option<String> {
    read(p).map(|s| s.lines().next().unwrap_or("").trim().to_string())
}

pub fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

pub fn meminfo_kib(key: &str) -> Option<u64> {
    let s = read("/proc/meminfo")?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            return rest.split_whitespace().next().and_then(|v| v.parse().ok());
        }
    }
    None
}
