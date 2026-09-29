use crate::clipboard_state;
use clipboard_history::Command;
use kde_bridge::session::SessionEvent;
use std::sync::mpsc::sync_channel;
use std::time::Duration;

const CLEAR_DEADLINE: Duration = Duration::from_secs(2);

fn clear_before_release() -> bool {
    let (finished, confirmation) = sync_channel(1);
    if !clipboard_state::send(Command::ClearClipboard { finished: Some(finished) }) {
        return false;
    }
    confirmation.recv_timeout(CLEAR_DEADLINE).unwrap_or(false)
}

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
                if !clear_before_release() {
                    tracing::warn!(?event, "the clipboard clear was not confirmed before release");
                }
            }
            drop(event);
        }
    });
}
