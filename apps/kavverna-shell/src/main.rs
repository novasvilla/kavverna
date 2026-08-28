mod awake_loop;
mod feature_list;
mod tray;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    tracing_subscriber::fmt::init();

    let (commands, requests) = std::sync::mpsc::channel();
    let tray = tray::show(commands);
    std::thread::spawn(move || awake_loop::run(requests, tray));

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/dev/kavverna/shell/qml/main.qml"));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
