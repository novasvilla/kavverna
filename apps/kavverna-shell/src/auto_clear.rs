use crate::clipboard_state;
use clipboard_history::Command;
use kde_bridge::session::SessionEvent;

pub fn serve(runtime: tokio::runtime::Handle) {
    let (events, incoming) = std::sync::mpsc::channel();

    runtime.spawn(async move {
        let watching = kde_bridge::session::watch(events, clipboard_state::clears_on_suspend);
        if let Err(err) = watching.await {
            tracing::error!(%err, "the clipboard will not clear on suspend or on lock");
        }
    });

    std::thread::spawn(move || {
        for event in incoming {
            let wanted = match &event {
                SessionEvent::AboutToSuspend(_) => clipboard_state::clears_on_suspend(),
                SessionEvent::ScreenLocked => clipboard_state::clears_on_screen_lock(),
            };
            if wanted {
                tracing::info!(?event, "emptying the clipboard");
                clipboard_state::send(Command::ClearClipboard);
            }
            // A suspend carries logind's delay lock, and dropping it here is what lets the
            // machine go. Held until the clear has been asked for rather than released on the
            // watcher's thread, which is what made this a race.
            drop(event);
        }
    });
}
