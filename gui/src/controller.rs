// The QObject the QML talks to. Holds UI state (status / running / output) and
// runs the shared `crucible` engine on a background thread, marshalling results
// back onto the Qt event loop via `qt_thread().queue(...)`.

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
        type Controller = super::ControllerRust;

        /// Run a suite by name: "all" | "cpu" | "mem" | "net" | "disk" | "info".
        #[qinvokable]
        fn run(self: Pin<&mut Controller>, kind: QString);
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
}

impl Default for ControllerRust {
    fn default() -> Self {
        Self {
            status: QString::from("Ready."),
            running: false,
            output: QString::from(""),
        }
    }
}

impl qobject::Controller {
    pub fn run(self: Pin<&mut Self>, kind: QString) {
        let kind = kind.to_string();
        let mut this = self;
        this.as_mut().set_running(true);
        this.as_mut().set_status(QString::from(format!("Running {} …", kind).as_str()));
        this.as_mut().set_output(QString::from(""));

        let qt_thread = this.qt_thread();
        std::thread::spawn(move || {
            let text = crucible::run_named(&kind);
            let _ = qt_thread.queue(move |mut qobj: Pin<&mut qobject::Controller>| {
                qobj.as_mut().set_output(QString::from(text.as_str()));
                qobj.as_mut().set_status(QString::from("Done."));
                qobj.as_mut().set_running(false);
            });
        });
    }
}
