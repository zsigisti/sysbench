// The QObject the QML talks to. Holds UI state and drives the shared `crucible`
// engine on background threads, marshalling everything back onto the Qt event
// loop via `qt_thread().queue(...)`.
//
// Exposes: benchmark runs (with live streaming for the full suite), sharing
// (server + paste.rs fallback) and submit (server only), file export, local run
// history + comparison/analysis, system facts, a theme flag, and self-uninstall.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status)]
        #[qproperty(bool, running)]
        #[qproperty(QString, output)]
        #[qproperty(QString, share_url)]
        #[qproperty(QString, backend)]
        #[qproperty(bool, dark)]
        #[qproperty(bool, has_results)]
        #[qproperty(QString, sys_facts)]
        #[qproperty(QString, history)]
        #[qproperty(QString, analysis)]
        #[qproperty(QString, compare_text)]
        // The JSON of the last completed run; kept as a property so it survives
        // across the bridge for export/share/submit. Not shown in the UI.
        #[qproperty(QString, last_json)]
        // The last render-benchmark result JSON (from RenderBench.qml).
        #[qproperty(QString, render_result)]
        // Renderer preference: "auto" | "opengl" | "vulkan" (persisted; applied
        // as QSG_RHI_BACKEND on the next launch).
        #[qproperty(QString, render_backend)]
        // The preference the running process was started with — when it differs
        // from render_backend the UI offers "Restart to apply".
        #[qproperty(QString, boot_backend)]
        #[qproperty(QString, app_version)]
        type Controller = super::ControllerRust;

        /// Run a suite: "all" | "cpu" | "mem" | "net" | "disk" | "info".
        #[qinvokable]
        fn run(self: Pin<&mut Controller>, kind: QString, duration: i32, runs: i32);

        /// Share results: CRUCIBLE server first, paste.rs fallback.
        #[qinvokable]
        fn share(self: Pin<&mut Controller>);

        /// Submit results to the CRUCIBLE score server (no fallback).
        #[qinvokable]
        fn submit(self: Pin<&mut Controller>);

        /// Clear the output pane and any share URL.
        #[qinvokable]
        fn clear(self: Pin<&mut Controller>);

        /// Write the current report text to `path` (a plain path or file:// URL).
        #[qinvokable]
        fn export_report(self: Pin<&mut Controller>, path: QString);

        /// Write the current results JSON to `path`.
        #[qinvokable]
        fn export_json(self: Pin<&mut Controller>, path: QString);

        /// Reload `history` + `analysis` from disk.
        #[qinvokable]
        fn refresh_history(self: Pin<&mut Controller>);

        /// Compare two recorded runs by id; result goes to `compare_text`.
        #[qinvokable]
        fn compare_runs(self: Pin<&mut Controller>, a: QString, b: QString);

        /// Remove installed files. `purge` also deletes local history.
        #[qinvokable]
        fn uninstall(self: Pin<&mut Controller>, purge: bool);

        /// Record a finished GPU render benchmark (JSON from RenderBench.qml):
        /// merges a "render" section into the run JSON and records to history.
        #[qinvokable]
        fn record_render(self: Pin<&mut Controller>, json: QString);

        /// Persist the renderer choice ("auto" | "opengl" | "vulkan").
        #[qinvokable]
        fn choose_render_backend(self: Pin<&mut Controller>, backend: QString);

        /// Flip + persist the theme.
        #[qinvokable]
        fn set_dark_pref(self: Pin<&mut Controller>, dark: bool);

        /// Relaunch the app (after changing the renderer backend).
        #[qinvokable]
        fn restart(self: Pin<&mut Controller>);
    }

    impl cxx_qt::Threading for Controller {}
}

use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::QString;

pub struct ControllerRust {
    status: QString,
    running: bool,
    output: QString,
    share_url: QString,
    backend: QString,
    dark: bool,
    has_results: bool,
    sys_facts: QString,
    history: QString,
    analysis: QString,
    compare_text: QString,
    last_json: QString,
    render_result: QString,
    render_backend: QString,
    boot_backend: QString,
    app_version: QString,
}

impl Default for ControllerRust {
    fn default() -> Self {
        let (hist, analysis) = history_payload();
        let prefs = crate::prefs::load();
        Self {
            status: QString::from("Ready."),
            running: false,
            output: QString::from(""),
            share_url: QString::from(""),
            backend: QString::from(""),
            dark: prefs.dark,
            has_results: false,
            sys_facts: QString::from(sys_facts_json().as_str()),
            history: QString::from(hist.as_str()),
            analysis: QString::from(analysis.as_str()),
            compare_text: QString::from(""),
            last_json: QString::from(""),
            render_result: QString::from(""),
            render_backend: QString::from(prefs.render_backend.as_str()),
            boot_backend: QString::from(prefs.render_backend.as_str()),
            app_version: QString::from(env!("CARGO_PKG_VERSION")),
        }
    }
}

impl qobject::Controller {
    pub fn run(mut self: Pin<&mut Self>, kind: QString, duration: i32, runs: i32) {
        if *self.running() {
            return;
        }
        let kind = kind.to_string();
        let opts = crucible::RunOpts {
            duration_secs: duration.max(1) as u64,
            runs: runs.max(1) as usize,
            streams: 4,
        };

        self.as_mut().set_running(true);
        self.as_mut().set_share_url(QString::from(""));
        self.as_mut().set_backend(QString::from(""));
        self.as_mut().set_output(QString::from(""));

        // "info" is not a benchmark — just render the report, no JSON/history.
        if kind == "info" {
            let qt = self.qt_thread();
            std::thread::spawn(move || {
                let text = crucible::report::render(false);
                let _ = qt.queue(move |mut o: Pin<&mut qobject::Controller>| {
                    o.as_mut().set_output(QString::from(text.as_str()));
                    o.as_mut().set_status(QString::from("System info."));
                    o.as_mut().set_has_results(false);
                    o.as_mut().set_running(false);
                });
            });
            return;
        }

        // For "all", stream each suite as it finishes and merge the structured
        // results into one record; otherwise run the single suite.
        let suites: Vec<String> = match kind.as_str() {
            "all" => ["cpu", "mem", "net", "disk"].iter().map(|s| s.to_string()).collect(),
            other => vec![other.to_string()],
        };

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let mut parts = Vec::new();
            for kind in &suites {
                let label = suite_label(kind);
                let q = qt.clone();
                let _ = q.queue(move |mut o: Pin<&mut qobject::Controller>| {
                    o.as_mut()
                        .set_status(QString::from(format!("Running {} …", label).as_str()));
                });

                let out = crucible::run_suite_collect(kind, &opts);
                let text = out.text;
                parts.push(out.results);

                let q = qt.clone();
                let _ = q.queue(move |mut o: Pin<&mut qobject::Controller>| {
                    let mut cur = o.output().to_string();
                    cur.push_str(&text);
                    o.as_mut().set_output(QString::from(cur.as_str()));
                });
            }

            // Merge + record to history.
            let merged = crucible::bench::merge(parts);
            let json = merged
                .as_ref()
                .and_then(|m| crucible::bench::to_json(m).ok())
                .unwrap_or_default();
            let recorded = if json.is_empty() {
                Err("no results".to_string())
            } else {
                crucible::history::record(&json).map(|e| e.id)
            };
            let (hist, analysis) = history_payload();

            let has = !json.is_empty();
            let _ = qt.queue(move |mut o: Pin<&mut qobject::Controller>| {
                o.as_mut().set_last_json(QString::from(json.as_str()));
                o.as_mut().set_has_results(has);
                o.as_mut().set_history(QString::from(hist.as_str()));
                o.as_mut().set_analysis(QString::from(analysis.as_str()));
                let msg = match recorded {
                    Ok(id) => format!("Done — recorded {}.", id),
                    Err(e) => format!("Done (history not saved: {}).", e),
                };
                o.as_mut().set_status(QString::from(msg.as_str()));
                o.as_mut().set_running(false);
            });
        });
    }

    pub fn share(mut self: Pin<&mut Self>) {
        let body = self.share_body();
        let Some(body) = body else {
            self.as_mut()
                .set_status(QString::from("Nothing to share yet — run a benchmark first."));
            return;
        };
        if *self.running() {
            return;
        }
        self.as_mut().set_running(true);
        self.as_mut().set_status(QString::from("Sharing …"));

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crucible::upload::share(&body);
            let _ = qt.queue(move |mut o: Pin<&mut qobject::Controller>| {
                match result {
                    Ok(s) => {
                        o.as_mut().set_share_url(QString::from(s.url.as_str()));
                        o.as_mut().set_backend(QString::from(s.backend));
                        o.as_mut()
                            .set_status(QString::from(format!("Shared via {}.", s.backend).as_str()));
                    }
                    Err(e) => {
                        o.as_mut().set_status(QString::from(format!("Share failed: {}", e).as_str()));
                    }
                }
                o.as_mut().set_running(false);
            });
        });
    }

    pub fn submit(mut self: Pin<&mut Self>) {
        let Some(body) = self.share_body() else {
            self.as_mut()
                .set_status(QString::from("Nothing to submit yet — run a benchmark first."));
            return;
        };
        if *self.running() {
            return;
        }
        self.as_mut().set_running(true);
        self.as_mut()
            .set_status(QString::from(format!("Submitting to {} …", crucible::upload::server_base()).as_str()));

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crucible::upload::submit(&body);
            let _ = qt.queue(move |mut o: Pin<&mut qobject::Controller>| {
                match result {
                    Ok(url) => {
                        o.as_mut().set_share_url(QString::from(url.as_str()));
                        o.as_mut().set_backend(QString::from("crux"));
                        o.as_mut().set_status(QString::from("Submitted to the score server."));
                    }
                    Err(e) => {
                        o.as_mut().set_status(QString::from(format!("Submit failed: {}", e).as_str()));
                    }
                }
                o.as_mut().set_running(false);
            });
        });
    }

    pub fn clear(mut self: Pin<&mut Self>) {
        if *self.running() {
            return;
        }
        self.as_mut().set_output(QString::from(""));
        self.as_mut().set_share_url(QString::from(""));
        self.as_mut().set_backend(QString::from(""));
        self.as_mut().set_has_results(false);
        self.as_mut().set_last_json(QString::from(""));
        self.as_mut().set_render_result(QString::from(""));
        self.as_mut().set_status(QString::from("Ready."));
    }

    pub fn export_report(mut self: Pin<&mut Self>, path: QString) {
        let p = strip_file_url(&path.to_string());
        let body = self.output().to_string();
        self.as_mut().set_status(QString::from(match std::fs::write(&p, body) {
            Ok(()) => format!("Saved report to {}", p),
            Err(e) => format!("Save failed: {}", e),
        }.as_str()));
    }

    pub fn export_json(mut self: Pin<&mut Self>, path: QString) {
        let p = strip_file_url(&path.to_string());
        let body = self.last_json().to_string();
        if body.is_empty() {
            self.as_mut().set_status(QString::from("No JSON to export — run a benchmark first."));
            return;
        }
        self.as_mut().set_status(QString::from(match std::fs::write(&p, body) {
            Ok(()) => format!("Saved JSON to {}", p),
            Err(e) => format!("Save failed: {}", e),
        }.as_str()));
    }

    pub fn refresh_history(mut self: Pin<&mut Self>) {
        let (hist, analysis) = history_payload();
        self.as_mut().set_history(QString::from(hist.as_str()));
        self.as_mut().set_analysis(QString::from(analysis.as_str()));
    }

    pub fn compare_runs(mut self: Pin<&mut Self>, a: QString, b: QString) {
        let (a, b) = (a.to_string(), b.to_string());
        if a.is_empty() || b.is_empty() || a == b {
            self.as_mut()
                .set_compare_text(QString::from("Pick two different runs to compare."));
            return;
        }
        let text = match (crucible::history::load(&a), crucible::history::load(&b)) {
            (Ok(ja), Ok(jb)) => crucible::compare::render_json(&ja, &jb, false)
                .unwrap_or_else(|e| format!("Compare failed: {}", e)),
            _ => "Could not load one of the runs.".to_string(),
        };
        self.as_mut().set_compare_text(QString::from(text.as_str()));
    }

    pub fn record_render(mut self: Pin<&mut Self>, json: QString) {
        let raw = json.to_string();
        let parsed: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                self.as_mut()
                    .set_status(QString::from(format!("Render result rejected: {}", e).as_str()));
                return;
            }
        };
        self.as_mut().set_render_result(QString::from(raw.as_str()));

        // Merge a "render" section into the run JSON so Share/Submit/Export and
        // history all carry it. A render-only run still needs sysinfo (it holds
        // the stable machine_id the score server keys on).
        let cur = self.last_json().to_string();
        let mut root: serde_json::Value =
            serde_json::from_str(&cur).unwrap_or(serde_json::Value::Null);
        if !root.is_object() {
            root = serde_json::json!({ "sysinfo": crucible::sysinfo::SysInfo::collect() });
        }
        root["render"] = parsed.clone();
        let merged = root.to_string();

        let recorded = crucible::history::record(&merged).map(|e| e.id);
        let (hist, analysis) = history_payload();

        self.as_mut().set_last_json(QString::from(merged.as_str()));
        self.as_mut().set_has_results(true);
        self.as_mut().set_history(QString::from(hist.as_str()));
        self.as_mut().set_analysis(QString::from(analysis.as_str()));

        let score = parsed.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
        let fps = parsed.get("fps").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let api = parsed.get("api").and_then(|v| v.as_str()).unwrap_or("?");
        let msg = match recorded {
            Ok(id) => format!(
                "Render done — score {} ({:.0} fps, {}); recorded {}.",
                score, fps, api, id
            ),
            Err(e) => format!(
                "Render done — score {} ({:.0} fps, {}); history not saved: {}.",
                score, fps, api, e
            ),
        };
        self.as_mut().set_status(QString::from(msg.as_str()));
    }

    pub fn choose_render_backend(mut self: Pin<&mut Self>, backend: QString) {
        let b = backend.to_string();
        if !matches!(b.as_str(), "auto" | "opengl" | "vulkan") {
            return;
        }
        self.as_mut().set_render_backend(QString::from(b.as_str()));
        let p = crate::prefs::Prefs {
            dark: *self.dark(),
            render_backend: b,
        };
        if let Err(e) = crate::prefs::save(&p) {
            self.as_mut()
                .set_status(QString::from(format!("Could not save preferences: {}", e).as_str()));
        }
    }

    pub fn set_dark_pref(mut self: Pin<&mut Self>, dark: bool) {
        self.as_mut().set_dark(dark);
        let p = crate::prefs::Prefs {
            dark,
            render_backend: self.render_backend().to_string(),
        };
        if let Err(e) = crate::prefs::save(&p) {
            self.as_mut()
                .set_status(QString::from(format!("Could not save preferences: {}", e).as_str()));
        }
    }

    pub fn restart(self: Pin<&mut Self>) {
        if let Ok(exe) = std::env::current_exe() {
            // the child re-derives the backend from prefs, not our (stale) env
            let _ = std::process::Command::new(exe)
                .env_remove("QSG_RHI_BACKEND")
                .spawn();
        }
        std::process::exit(0);
    }

    pub fn uninstall(mut self: Pin<&mut Self>, purge: bool) {
        if *self.running() {
            return;
        }
        self.as_mut().set_running(true);
        self.as_mut().set_status(QString::from("Uninstalling …"));
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let report = crucible::uninstall::run(purge);
            let msg = report.summary();
            let _ = qt.queue(move |mut o: Pin<&mut qobject::Controller>| {
                o.as_mut().set_status(QString::from(msg.as_str()));
                o.as_mut().set_running(false);
            });
        });
    }
}

impl qobject::Controller {
    /// The body to share/submit: prefer the structured JSON, else the text.
    fn share_body(&self) -> Option<String> {
        let json = self.last_json().to_string();
        if !json.is_empty() {
            Some(json)
        } else {
            let t = self.output().to_string();
            if t.trim().is_empty() {
                None
            } else {
                Some(t)
            }
        }
    }
}

fn suite_label(kind: &str) -> &'static str {
    match kind {
        "cpu" => "CPU",
        "mem" => "memory",
        "net" => "network",
        "disk" => "storage",
        "info" => "system info",
        _ => "benchmark",
    }
}

fn strip_file_url(p: &str) -> String {
    p.strip_prefix("file://").unwrap_or(p).to_string()
}

/// JSON of the current machine's headline facts (for the System tab cards).
fn sys_facts_json() -> String {
    let i = crucible::sysinfo::SysInfo::collect();
    serde_json::json!({
        "cpu_model": i.cpu_model,
        "logical_cores": i.logical_cores,
        "ram_mib": i.ram_mib,
        "kernel": i.kernel,
        "os": i.os,
        "machine_id": i.machine_id,
    })
    .to_string()
}

/// (history array JSON, analysis object JSON) built from disk.
fn history_payload() -> (String, String) {
    let entries = crucible::history::list();
    let arr: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            let s = &e.summary;
            serde_json::json!({
                "id": e.id,
                "when": crucible::history::fmt_time(e.unix_time),
                "cpu": s.cpu_model,
                "headline": s.headline(),
                "mt": s.composite_mt,
                "st": s.composite_st,
                "triad": s.mem_triad_gbs,
                "down": s.net_down_mbps,
                "up": s.net_up_mbps,
                "lat": s.net_latency_ms,
                "write": s.disk_seq_write_mbs,
                "render": s.render_score,
            })
        })
        .collect();

    let best = |f: fn(&crucible::summary::Summary) -> Option<f64>| -> Option<f64> {
        entries
            .iter()
            .filter_map(|e| f(&e.summary))
            .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |x| x.max(v))))
    };
    let analysis = serde_json::json!({
        "count": entries.len(),
        "best_mt": best(|s| s.composite_mt),
        "best_st": best(|s| s.composite_st),
        "best_triad": best(|s| s.mem_triad_gbs),
        "best_down": best(|s| s.net_down_mbps),
        "best_render": best(|s| s.render_score),
    });

    (serde_json::Value::Array(arr).to_string(), analysis.to_string())
}
