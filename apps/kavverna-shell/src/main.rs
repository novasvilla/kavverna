mod app_icon;
mod awake_loop;
mod awake_state;
mod command;
mod launch_at_login;
mod mixer_state;
mod mixer_view;
mod settings;
mod panel;
mod remote;
mod tray;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("kavverna_shell=info,keep_awake=info")),
        )
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "kavverna starting");

    let bus = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::error!(%err, "no runtime for the session bus");
            return;
        }
    };

    match bus.block_on(remote::claim()) {
        Ok(remote::Claim::AlreadyRunning) => {
            tracing::info!("another instance is already running, raised its panel instead");
            return;
        }
        Ok(remote::Claim::Ours) => {}
        Err(err) => tracing::warn!(%err, "running without a remote interface: {}", err),
    }

    let (sender, requests) = std::sync::mpsc::channel();
    command::publish(sender);

    let tray = tray::show();
    std::thread::spawn(move || awake_loop::run(requests, tray));
    std::thread::spawn(|| mixer_state::run(mixer_view::publish));

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
