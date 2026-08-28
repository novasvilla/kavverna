mod feature_list;
mod tray;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    tracing_subscriber::fmt::init();

    // Held for the process lifetime: dropping the handle unregisters the tray item.
    let _tray = tray::show("Kavverna is running".into());

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/dev/kavverna/shell/qml/main.qml"));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
