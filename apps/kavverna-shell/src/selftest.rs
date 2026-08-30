//! What this machine offers, one line each, so a report from an untested distribution arrives
//! as data instead of a conversation. The exit code is for scripts: zero when everything
//! Kavverna relies on answered, one otherwise.

use std::fmt::Write;

/// What the desktop is expected to offer, each checked by asking rather than assumed. `needed`
/// decides the exit code: ydotool is a trade-off nothing requires yet, so its absence is worth
/// a line and not a failure.
struct Check {
    what: &'static str,
    needed: bool,
    present: bool,
}

pub fn run() -> i32 {
    let mut checks = Vec::new();

    let advertised = clipboard_history::selection::advertised_globals();
    for protocol in [
        "ext_data_control_manager_v1",
        "zwp_primary_selection_device_manager_v1",
        "ext_idle_notifier_v1",
        "zwlr_layer_shell_v1",
    ] {
        checks.push(Check {
            what: protocol,
            needed: true,
            present: advertised.iter().any(|name| name == protocol),
        });
    }
    // Snippets will want this one day; nothing needs it yet.
    checks.push(Check {
        what: "zwp_input_method_v1",
        needed: false,
        present: advertised.iter().any(|name| name == "zwp_input_method_v1"),
    });

    for (what, present) in bus_answers() {
        checks.push(Check { what, needed: true, present });
    }
    checks.push(Check { what: "PipeWire", needed: true, present: pipewire_answers() });
    checks.push(Check {
        what: "SQLite with FTS5",
        needed: true,
        present: clipboard_history::store::fts5_available(),
    });
    checks.push(Check {
        what: "ydotool socket",
        needed: false,
        present: keep_awake::MouseJiggle::is_available(),
    });

    let mut out = String::new();
    let _ = writeln!(out, "Kavverna {} on {}", crate::remote::version(), distribution());
    for check in &checks {
        let verdict = match (check.present, check.needed) {
            (true, _) => "ok",
            (false, true) => "MISSING",
            (false, false) => "absent (optional)",
        };
        let _ = writeln!(out, "  {:<42} {verdict}", check.what);
    }
    print!("{out}");

    let missing = checks.iter().filter(|check| check.needed && !check.present).count();
    if missing > 0 {
        println!("{missing} of what Kavverna relies on did not answer.");
        return 1;
    }
    println!("Everything Kavverna relies on answered.");
    0
}

async fn has_owner(connection: Option<&zbus::Connection>, service: &str) -> bool {
    let probe = async {
        let name = zbus::names::BusName::try_from(service).ok()?;
        zbus::fdo::DBusProxy::new(connection?).await.ok()?.name_has_owner(name.into()).await.ok()
    };
    probe.await.unwrap_or(false)
}

/// Every bus question inside one runtime. A zbus connection runs its plumbing on the runtime
/// it was created in, so a connection made in one and asked in another waits forever.
fn bus_answers() -> Vec<(&'static str, bool)> {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
        return Vec::new();
    };

    runtime.block_on(async {
        let session = zbus::Connection::session().await.ok();
        let system = zbus::Connection::system().await.ok();

        let mut answers = Vec::new();
        for (what, service) in [
            ("KGlobalAccel", "org.kde.kglobalaccel"),
            ("PowerDevil", "org.kde.Solid.PowerManagement"),
            ("ScreenSaver", "org.freedesktop.ScreenSaver"),
            ("Notifications", "org.freedesktop.Notifications"),
        ] {
            answers.push((what, has_owner(session.as_ref(), service).await));
        }
        answers.push(("logind", has_owner(system.as_ref(), "org.freedesktop.login1").await));
        answers
    })
}

fn pipewire_answers() -> bool {
    sound_mixer::start().is_ok()
}

/// Which distribution a report comes from, since untested rather than refused is the point of
/// this whole command.
fn distribution() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|release| {
            release.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=").map(|name| name.trim_matches('"').to_owned())
            })
        })
        .unwrap_or_else(|| "an unnamed distribution".into())
}
