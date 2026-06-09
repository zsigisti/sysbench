// Local run history.
//
// Every full benchmark can be recorded as a JSON file under the user's data
// directory. The CLI (`crux history`) and the GUI list and re-open these for
// offline analysis and comparison — no server required.
//
//   $XDG_DATA_HOME/crucible/history/<unixtime>-<host>.json   (else ~/.local/share)

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::summary::Summary;

/// One stored run: its id (filename stem), when it was taken, and its digest.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub id: String,
    pub path: String,
    pub unix_time: u64,
    pub summary: Summary,
}

/// `$XDG_DATA_HOME/crucible` (or `~/.local/share/crucible`).
pub fn data_dir() -> PathBuf {
    if let Ok(x) = std::env::var("XDG_DATA_HOME") {
        if !x.is_empty() {
            return PathBuf::from(x).join("crucible");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/crucible")
}

/// The history subdirectory, created if needed.
pub fn history_dir() -> PathBuf {
    data_dir().join("history")
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn slug(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_lowercase();
    if s.is_empty() { "host".to_string() } else { s }
}

/// Record a full results JSON to the history directory. Returns the new entry.
pub fn record(json: &str) -> Result<Entry, String> {
    let summ = Summary::from_json(json)?;
    let dir = history_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {}", dir.display(), e))?;
    let t = now_unix();
    let host = slug(summ.cpu_model.split_whitespace().take(3).collect::<Vec<_>>().join("-").as_str());
    let id = format!("{}-{}", t, host);
    let path = dir.join(format!("{}.json", id));
    fs::write(&path, json).map_err(|e| format!("write {}: {}", path.display(), e))?;
    Ok(Entry {
        id,
        path: path.display().to_string(),
        unix_time: t,
        summary: summ,
    })
}

fn entry_from_path(path: &Path) -> Option<Entry> {
    let id = path.file_stem()?.to_string_lossy().into_owned();
    let json = fs::read_to_string(path).ok()?;
    let summary = Summary::from_json(&json).ok()?;
    // Prefer the unix prefix in the filename; fall back to mtime.
    let unix_time = id
        .split('-')
        .next()
        .and_then(|p| p.parse::<u64>().ok())
        .or_else(|| {
            fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
        })
        .unwrap_or(0);
    Some(Entry {
        id,
        path: path.display().to_string(),
        unix_time,
        summary,
    })
}

/// All recorded runs, newest first.
pub fn list() -> Vec<Entry> {
    let dir = history_dir();
    let mut entries: Vec<Entry> = match fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
            .filter_map(|p| entry_from_path(&p))
            .collect(),
        Err(_) => Vec::new(),
    };
    entries.sort_by(|a, b| b.unix_time.cmp(&a.unix_time));
    entries
}

/// Format a Unix timestamp as `YYYY-MM-DD HH:MM UTC` without pulling in a date
/// crate (civil-from-days, valid for all dates we care about).
pub fn fmt_time(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    let (h, mi) = (secs / 3600, (secs % 3600) / 60);
    // days since 1970-01-01 -> civil date (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02} {:02}:{:02} UTC", y, m, d, h, mi)
}

/// Load the raw JSON for a run id (or a path).
pub fn load(id_or_path: &str) -> Result<String, String> {
    let direct = Path::new(id_or_path);
    if direct.is_file() {
        return fs::read_to_string(direct).map_err(|e| e.to_string());
    }
    let p = history_dir().join(format!("{}.json", id_or_path));
    fs::read_to_string(&p).map_err(|e| format!("no such run '{}' ({})", id_or_path, e))
}
