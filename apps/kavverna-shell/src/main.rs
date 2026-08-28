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
mod remote;
mod settings;
mod shortcuts;
mod tray;
mod vitals_state;
mod vitals_view;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};
use feature_catalog::Feature;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(
            |_| {
                tracing_subscriber::EnvFilter::new(
                    "kavverna_shell=info,keep_awake=info,clipboard_history=info",
                )
            },
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
            remote::Wanted::Panel => {}
            remote::Wanted::Settings => panel::request(panel::Requested::Settings),
            remote::Wanted::Page(name) => panel::request(panel::Requested::Page(name.clone())),
        },
        Err(err) => tracing::warn!(%err, "running without a remote interface: {}", err),
    }

    let (sender, requests) = std::sync::mpsc::channel();
    command::publish(sender);

    // A utility removed in the features list never starts, which is what makes that switch
    // mean something rather than only hiding a page.
    let clipboard = [Feature::ClipboardHistory, Feature::ClipboardAutoClear, Feature::CleanUrl];
    let sound = [Feature::VolumeMixer, Feature::OutputSwitcher, Feature::MicrophoneTools];

    if settings::is_installed(Feature::ClipboardHistory) {
        shortcuts::serve(bus.handle().clone());
    }
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
