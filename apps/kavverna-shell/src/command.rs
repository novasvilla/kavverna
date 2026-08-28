use keep_awake::{Hold, Scope};
use std::sync::OnceLock;
use std::sync::mpsc::Sender;

pub enum Command {
    Engage(Hold, Scope),
    Extend(std::time::Duration),
    Release,
}

static COMMANDS: OnceLock<Sender<Command>> = OnceLock::new();

pub fn publish(sender: Sender<Command>) {
    let _ = COMMANDS.set(sender);
}

/// Silently does nothing before the keep awake thread is up, which only happens during the
/// few milliseconds of startup.
pub fn send(command: Command) {
    if let Some(sender) = COMMANDS.get() {
        let _ = sender.send(command);
    }
}
