use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("com.crucible.gui")
            .qml_files(["qml/main.qml", "qml/Theme.qml", "qml/Card.qml"]),
    )
    .qt_module("Quick")
    .file("src/controller.rs")
    .build();
}
