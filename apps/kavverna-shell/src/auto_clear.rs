use crate::clipboard_state;
use clipboard_history::Command;
use kde_bridge::session::SessionEvent;

pub fn serve(runtime: tokio::runtime::Handle) {
    let (events, incoming) = std::sync::mpsc::channel();

    runtime.spawn(async move {
        if let Err(err) = kde_bridge::session::watch(events).await {
            tracing::error!(%err, "the clipboard will not clear on suspend or on lock");
        }
    });

    std::thread::spawn(move || {
        for event in incoming {
            let wanted = match event {
                SessionEvent::AboutToSuspend => clipboard_state::clears_on_suspend(),
                SessionEvent::ScreenLocked => clipboard_state::clears_on_screen_lock(),
            };
            if wanted {
                tracing::info!(?event, "emptying the clipboard");
                clipboard_state::send(Command::ClearClipboard);
            }
        }
    });
}
