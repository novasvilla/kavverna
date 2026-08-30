//! The tray, the panel and the settings window.
//!
//! Every feature runs on a thread of its own and Qt owns the main one, so nothing here is
//! shared without a mutex, and nothing reaches QML except through `CxxQtThread::queue`. The
//! `*_state` modules hold what a feature last published; the `*_view` modules push it across.

mod app_icon;
mod auto_clear;
mod awake_loop;
mod awake_state;
mod clipboard_state;
mod clipboard_view;
mod command;
mod features_view;
mod jiggle_state;
mod launch_at_login;
mod mixer_state;
mod mixer_view;
mod panel;
mod panel_anchor;
mod remote;
mod selftest;
mod settings;
mod shortcuts;
mod tray;
mod vitals_state;
mod vitals_view;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};
use feature_catalog::Feature;

fn main() {
    // Asked and answered without a window or a tray icon.
    match remote::Wanted::from_arguments(std::env::args()) {
        remote::Wanted::Version => return println!("{}", remote::version()),
        remote::Wanted::Usage => return print!("{}", remote::USAGE),
        remote::Wanted::Selftest => std::process::exit(selftest::run()),
        remote::Wanted::Unknown(what) => {
            eprintln!("not understood: {what}");
            eprint!("{}", remote::USAGE);
            std::process::exit(2);
        }
        _ => {}
    }

    // Everything of ours at info, with the libraries turned down, rather than naming our own
    // crates one by one. The list used to be by name and had already rotted: `kde_bridge` was
    // never on it, so global shortcuts registered and failed in silence.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(
            |_| tracing_subscriber::EnvFilter::new("info,zbus=warn,tracing=warn,ksni=warn"),
        ))
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "kavverna starting");

    let bus = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::error!(%err, "no runtime for the session bus");
            return;
        }
    };

    let wanted = remote::Wanted::from_arguments(std::env::args());

    match bus.block_on(remote::claim(&wanted)) {
        Ok(remote::Claim::AlreadyRunning) => {
            tracing::info!("another instance is already running, raised its panel instead");
            return;
        }
        Ok(remote::Claim::Ours) => match &wanted {
            remote::Wanted::Settings => panel::request(panel::Requested::Settings),
            remote::Wanted::Page(name) => panel::request(panel::Requested::Page(name.clone())),
            _ => {}
        },
        Err(err) => tracing::warn!(%err, "running without a remote interface: {}", err),
    }

    let (sender, requests) = std::sync::mpsc::channel();
    command::publish(sender);

    // A utility removed in the features list never starts, which is what makes that switch
    // mean something rather than only hiding a page.
    let clipboard = [
        Feature::ClipboardHistory,
        Feature::ClipboardAutoClear,
        Feature::CleanUrl,
        Feature::ClipboardTransform,
    ];
    let sound = [Feature::VolumeMixer, Feature::OutputSwitcher, Feature::MicrophoneTools];

    // Not gated on any one utility, unlike everything below it: showing the panel is a shortcut
    // in its own right, and the rest are filtered inside against what is installed.
    shortcuts::serve(bus.handle().clone());

    if settings::is_installed(Feature::ClipboardAutoClear) {
        auto_clear::serve(bus.handle().clone());
    }

    let tray = tray::show();
    if settings::any_installed(&[Feature::KeepAwake, Feature::MouseJiggle]) {
        std::thread::spawn(move || awake_loop::run(requests, tray));
    }
    if settings::any_installed(&sound) {
        std::thread::spawn(|| mixer_state::run(mixer_view::publish));
    }
    if settings::any_installed(&clipboard) {
        std::thread::spawn(|| clipboard_state::run(clipboard_view::publish));
    }
    if settings::is_installed(Feature::SystemMonitor) {
        std::thread::spawn(|| {
            vitals_state::run(std::time::Duration::from_secs(2), vitals_view::publish)
        });
    }

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/dev/kavverna/shell/qml/main.qml"));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }

    drop(bus);
}
