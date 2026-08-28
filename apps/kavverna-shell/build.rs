use cxx_qt_build::{CxxQtBuilder, QmlModule};

/// Mirrors the reference project's split: one file per panel section, one per settings page,
/// shared controls and design tokens of their own.
const QML: &[&str] = &[
    "qml/main.qml",
    "qml/Theme.qml",
    "qml/Shared/SectionLabel.qml",
    "qml/Shared/Card.qml",
    "qml/Shared/SettingRow.qml",
    "qml/Shared/ChoiceRow.qml",
    "qml/MenuPanel/EnergySection.qml",
    "qml/MenuPanel/SoundSection.qml",
    "qml/MenuPanel/ToolsSection.qml",
    "qml/Settings/SettingsPage.qml",
];

const RUST: &[&str] = &["src/panel.rs", "src/mixer_view.rs"];

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
