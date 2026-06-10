use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    // Keep a single source of truth for the logo: copy the repo's canonical
    // assets/logo.svg next to the QML at build time so it can be bundled into
    // the module's Qt resources (and referenced as a relative "logo.svg").
    println!("cargo::rerun-if-changed=../assets/logo.svg");
    if let Err(e) = std::fs::copy("../assets/logo.svg", "qml/logo.svg") {
        println!("cargo::warning=could not copy logo.svg: {e}");
    }

    CxxQtBuilder::new_qml_module(
        QmlModule::new("com.crucible.gui")
            .qml_files([
                "qml/main.qml",
                "qml/Panel.qml",
                "qml/StatCard.qml",
                "qml/RenderBench.qml",
            ]),
    )
    .qrc_resources(["qml/logo.svg"])
    .qt_module("Quick")
    .file("src/controller.rs")
    .build();
}
