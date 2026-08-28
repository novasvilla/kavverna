use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("dev.kavverna.shell").qml_file("qml/main.qml"))
        .file("src/panel.rs")
        .build();
}
