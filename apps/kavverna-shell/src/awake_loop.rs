use crate::command::Command;
use crate::tray::StatusIcon;
use crate::{awake_state, panel};
use keep_awake::KeepAwake;
use ksni::blocking::Handle;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

const TICK: Duration = Duration::from_secs(1);

/// Runs on its own thread with a current-thread runtime, so neither Qt's event loop nor
/// ksni's has to accommodate it.
pub fn run(commands: Receiver<Command>, tray: Option<Handle<StatusIcon>>) {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::error!(%err, "keep awake unavailable: no runtime");
            return;
        }
    };

    let mut keep_awake = match runtime.block_on(KeepAwake::connect()) {
        Ok(keep_awake) => keep_awake,
        Err(err) => {
            tracing::error!(%err, "keep awake unavailable: no session bus");
            return;
        }
    };

    loop {
        match commands.recv_timeout(TICK) {
            Ok(Command::Engage(hold, scope)) => {
                if let Err(err) = runtime.block_on(keep_awake.engage(hold, scope)) {
                    tracing::error!(%err, "could not engage keep awake");
                }
            }
            Ok(Command::Release) => runtime.block_on(keep_awake.release()),
            Err(RecvTimeoutError::Timeout) => {
                runtime.block_on(keep_awake.expire_if_due());
            }
            Err(RecvTimeoutError::Disconnected) => {
                runtime.block_on(keep_awake.release());
                return;
            }
        }

        let active = keep_awake.is_active();
        let remaining = keep_awake.remaining();

        awake_state::set_hold(active, remaining);
        panel::publish_awake(active, remaining);

        if let Some(tray) = tray.as_ref() {
            tray.update(move |icon: &mut StatusIcon| {
                icon.awake = active;
                icon.remaining = remaining;
            });
        }
    }
}
