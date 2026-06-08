// The QObject the QML talks to. Holds UI state and drives the shared `crucible`
// engine on background threads, marshalling everything back onto the Qt event
// loop via `qt_thread().queue(...)`.

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
        type Controller = super::ControllerRust;

        /// Run a suite: "all" | "cpu" | "mem" | "net" | "disk" | "info".
        /// For benchmark suites, `duration`/`runs` come from the UI settings.
        #[qinvokable]
        fn run(self: Pin<&mut Controller>, kind: QString, duration: i32, runs: i32);

        /// Upload the current output to paste.rs and expose the URL.
        #[qinvokable]
        fn share(self: Pin<&mut Controller>);

        /// Clear the output pane and any share URL.
        #[qinvokable]
        fn clear(self: Pin<&mut Controller>);
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
}

impl Default for ControllerRust {
    fn default() -> Self {
        Self {
            status: QString::from("Ready."),
            running: false,
            output: QString::from(""),
            share_url: QString::from(""),
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
        self.as_mut().set_output(QString::from(""));

        // For "all", run each suite in turn and stream its output in as it
        // finishes (nicer than one long opaque wait). Otherwise run the one.
        let suites: Vec<String> = match kind.as_str() {
            "all" => ["cpu", "mem", "net", "disk"].iter().map(|s| s.to_string()).collect(),
            other => vec![other.to_string()],
        };

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            for kind in &suites {
                let kind = kind.as_str();
                let label = suite_label(kind);
                let q = qt.clone();
                let _ = q.queue(move |mut o: Pin<&mut qobject::Controller>| {
                    o.as_mut().set_status(QString::from(format!("Running {} …", label).as_str()));
                });

                let text = crucible::run_suite_text(kind, &opts);

                let q = qt.clone();
                let _ = q.queue(move |mut o: Pin<&mut qobject::Controller>| {
                    let mut cur = o.output().to_string();
                    cur.push_str(&text);
                    o.as_mut().set_output(QString::from(cur.as_str()));
                });
            }
            let _ = qt.queue(|mut o: Pin<&mut qobject::Controller>| {
                o.as_mut().set_status(QString::from("Done."));
                o.as_mut().set_running(false);
            });
        });
    }

    pub fn share(mut self: Pin<&mut Self>) {
        if *self.running() {
            return;
        }
        let body = self.output().to_string();
        if body.trim().is_empty() {
            self.as_mut().set_status(QString::from("Nothing to share yet — run a benchmark first."));
            return;
        }
        self.as_mut().set_running(true);
        self.as_mut().set_status(QString::from("Uploading to paste.rs …"));

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crucible::upload::upload(&body);
            let _ = qt.queue(move |mut o: Pin<&mut qobject::Controller>| {
                match result {
                    Ok(url) => {
                        o.as_mut().set_share_url(QString::from(url.as_str()));
                        o.as_mut().set_status(QString::from("Shared."));
                    }
                    Err(e) => {
                        o.as_mut().set_status(QString::from(format!("Upload failed: {}", e).as_str()));
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
        self.as_mut().set_status(QString::from("Ready."));
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
