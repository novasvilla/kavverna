use cxx_qt_build::{CxxQtBuilder, QmlModule};

const QML: &[&str] = &["qml/main.qml"];
const RUST: &[&str] = &["src/panel.rs"];

fn main() {
    // Declared by hand because the builder only tracks the Rust sources, so without this a
    // QML edit silently keeps the previous interface in the binary.
    for file in QML.iter().chain(RUST) {
        println!("cargo::rerun-if-changed={file}");
    }

    CxxQtBuilder::new_qml_module(QmlModule::new("dev.kavverna.shell").qml_files(QML))
        .files(RUST)
        .build();
}
