use cxx_qt_build::{CxxQtBuilder, QmlModule};

// One file per panel section, one per settings page, and the shared controls and design tokens
// they are built from.
const QML: &[&str] = &[
    "qml/main.qml",
    "qml/Theme.qml",
    "qml/Shared/SectionLabel.qml",
    "qml/Shared/Card.qml",
    "qml/Shared/SettingRow.qml",
    "qml/Shared/ChoiceRow.qml",
    "qml/Shared/PillButton.qml",
    "qml/Shared/Toggle.qml",
    "qml/Shared/Tick.qml",
    "qml/MenuPanel/EnergySection.qml",
    "qml/MenuPanel/SoundSection.qml",
    "qml/MenuPanel/MonitoringSection.qml",
    "qml/MenuPanel/ClipboardSection.qml",
    "qml/MenuPanel/ToolsSection.qml",
    "qml/Settings/SettingsPage.qml",
    "qml/Settings/FeaturesCard.qml",
];

const RUST: &[&str] = &[
    "src/panel.rs",
    "src/mixer_view.rs",
    "src/vitals_view.rs",
    "src/clipboard_view.rs",
    "src/features_view.rs",
];

fn main() {
    // Declared by hand because the builder only tracks the Rust sources, so without this a
    // QML edit silently keeps the previous interface in the binary.
    for file in QML.iter().chain(RUST) {
        println!("cargo::rerun-if-changed={file}");
    }

    // Cargo versions are three numbers. The fourth the release scheme asks for is the build
    // that produced the binary, stamped by CI and zero for anything built by hand.
    let build = std::env::var("KAVVERNA_BUILD").unwrap_or_else(|_| "0".to_owned());
    println!("cargo::rerun-if-env-changed=KAVVERNA_BUILD");
    println!("cargo::rustc-env=KAVVERNA_BUILD={build}");

    CxxQtBuilder::new_qml_module(QmlModule::new("dev.kavverna.shell").qml_files(QML))
        .files(RUST)
        .build();
}
