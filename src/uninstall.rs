// Self-uninstall.
//
// Removes every artifact the installer (`install.sh`) creates, from both the
// per-user (`~/.local`, `~/.config`) and system (`/usr/local`, `/usr`)
// locations — whatever exists and is writable. Used by `crux uninstall` and the
// GUI's uninstall button so the tool can remove itself without the script.
//
// Note: a running binary can delete its own file on Unix; the process keeps
// running until exit, so the active `crux`/`crux-gui` still finishes cleanly.

use std::fs;
use std::path::PathBuf;

/// Outcome of an uninstall sweep.
#[derive(Debug, Default)]
pub struct Report {
    /// Paths that were removed.
    pub removed: Vec<String>,
    /// Paths that exist but could not be removed (e.g. permission denied).
    pub failed: Vec<String>,
}

impl Report {
    pub fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.failed.is_empty()
    }

    /// A short human summary suitable for the CLI or a GUI status line.
    pub fn summary(&self) -> String {
        if self.removed.is_empty() && self.failed.is_empty() {
            "Nothing found to remove (already uninstalled, or installed elsewhere).".to_string()
        } else if self.failed.is_empty() {
            format!("Uninstalled CRUCIBLE — {} item(s) removed.", self.removed.len())
        } else {
            format!(
                "Removed {} item(s); {} needed elevated permissions (re-run with sudo).",
                self.removed.len(),
                self.failed.len()
            )
        }
    }
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

/// All paths the installer may have created.
fn targets() -> Vec<PathBuf> {
    let h = home();
    let mut v: Vec<PathBuf> = Vec::new();

    // binaries + the sysinfo alias
    for d in ["/usr/local/bin", "/usr/bin"] {
        for n in ["crux", "sysinfo", "crux-gui"] {
            v.push(PathBuf::from(d).join(n));
        }
    }
    for n in ["crux", "sysinfo", "crux-gui"] {
        v.push(h.join(".local/bin").join(n));
    }

    // man page
    v.push(h.join(".local/share/man/man1/crux.1"));
    v.push(PathBuf::from("/usr/local/share/man/man1/crux.1"));
    v.push(PathBuf::from("/usr/share/man/man1/crux.1"));

    // shell completions
    v.push(h.join(".local/share/bash-completion/completions/crux"));
    v.push(PathBuf::from("/usr/share/bash-completion/completions/crux"));
    v.push(h.join(".local/share/zsh/site-functions/_crux"));
    v.push(PathBuf::from("/usr/share/zsh/site-functions/_crux"));
    v.push(h.join(".config/fish/completions/crux.fish"));
    v.push(PathBuf::from("/usr/share/fish/vendor_completions.d/crux.fish"));

    // desktop entry + icon
    v.push(h.join(".local/share/applications/crux-gui.desktop"));
    v.push(PathBuf::from("/usr/share/applications/crux-gui.desktop"));
    v.push(h.join(".local/share/icons/hicolor/scalable/apps/crucible.svg"));
    v.push(PathBuf::from("/usr/share/icons/hicolor/scalable/apps/crucible.svg"));

    v
}

/// Remove all installer artifacts. Local run history (`~/.local/share/crucible`)
/// is preserved unless `purge_data` is true.
pub fn run(purge_data: bool) -> Report {
    let mut report = Report::default();
    for path in targets() {
        let exists = path.exists() || fs::symlink_metadata(&path).is_ok();
        if !exists {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => report.removed.push(path.display().to_string()),
            Err(_) => report.failed.push(path.display().to_string()),
        }
    }
    if purge_data {
        let dir = crate::history::data_dir();
        if dir.exists() {
            match fs::remove_dir_all(&dir) {
                Ok(()) => report.removed.push(dir.display().to_string()),
                Err(_) => report.failed.push(dir.display().to_string()),
            }
        }
    }
    report
}
