use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::path::{Path, PathBuf};

const QML_ROOT: &str = "qml";

const RUST: &[&str] = &[
    "src/panel.rs",
    "src/mixer_view.rs",
    "src/vitals_view.rs",
    "src/clipboard_view.rs",
    "src/features_view.rs",
];

/// Found rather than listed. The list used to be written out by hand and a new component was
/// simply absent from the module, which fails at load with no error naming the file, since a QML
/// failure is otherwise silent. Sorted so two builds of the same tree produce the same module.
fn qml_files(root: &Path, found: &mut Vec<PathBuf>, directories: &mut Vec<PathBuf>) {
    directories.push(root.to_path_buf());

    let Ok(entries) = std::fs::read_dir(root) else {
        panic!("{} is missing, so there is no interface to build", root.display());
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            qml_files(&path, found, directories);
        } else if path.extension().is_some_and(|extension| extension == "qml") {
            found.push(path);
        }
    }
}

fn main() {
    let (mut qml, mut directories) = (Vec::new(), Vec::new());
    qml_files(Path::new(QML_ROOT), &mut qml, &mut directories);
    qml.sort();

    assert!(!qml.is_empty(), "no QML found under {QML_ROOT}");

    // The builder only tracks the Rust sources, so without this a QML edit silently keeps the
    // previous interface in the binary. The directories are watched as well as the files, or a
    // component added after the last build would not trigger one.
    for directory in &directories {
        println!("cargo::rerun-if-changed={}", directory.display());
    }
    for file in qml
        .iter()
        .map(|path| path.display().to_string())
        .chain(RUST.iter().map(|f| (*f).to_owned()))
    {
        println!("cargo::rerun-if-changed={file}");
    }

    // Cargo versions are three numbers. The fourth the release scheme asks for is the build
    // that produced the binary, stamped by CI and zero for anything built by hand.
    let build = std::env::var("KAVVERNA_BUILD").unwrap_or_else(|_| "0".to_owned());
    println!("cargo::rerun-if-env-changed=KAVVERNA_BUILD");
    println!("cargo::rustc-env=KAVVERNA_BUILD={build}");

    CxxQtBuilder::new_qml_module(QmlModule::new("dev.kavverna.shell").qml_files(&qml))
        .files(RUST)
        .build();
}
