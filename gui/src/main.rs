// crux-gui — Qt/QML front-end for the CRUCIBLE benchmark engine.

pub mod controller;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    // Force the Basic Quick Controls style. Without this, a distro style such as
    // KDE Breeze is auto-selected and overrides our custom `background` /
    // TextArea styling (it can't even accept `background: null`), producing
    // "Unable to assign ... to QQuickTextInput" errors and a broken look.
    // Set before the engine is created — Qt reads it during QML init.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        std::env::set_var("QT_QUICK_CONTROLS_STYLE", "Basic");
    }

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/com/crucible/gui/qml/main.qml"));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
