use crate::{awake_state, mixer_state, panel, settings};
use feature_catalog::Feature;
use kde_bridge::shortcuts::{ALT, CONTROL, Shortcut};

const COMPONENT: &str = "kavverna";
const FRIENDLY: &str = "Kavverna";

const SHOW_PANEL: &str = "show-panel";
const SHOW_CLIPBOARD: &str = "show-clipboard";
const TOGGLE_AWAKE: &str = "toggle-awake";
const MUTE_EVERY_INPUT: &str = "mute-every-input";
const NEXT_OUTPUT: &str = "next-output";
const SHOW_SHELF: &str = "show-shelf";

/// Ctrl+Alt and a letter throughout, rather than the reference app's four-key combinations: it
/// is the shape a Linux user already reaches for, and Plasma leaves that row free. The key here
/// is only a default, so one somebody rebinds in System Settings is kept.
const KEYS: &[(&str, Option<Feature>, &str, i32)] = &[
    (SHOW_PANEL, None, "Show Kavverna", CONTROL | ALT | b'K' as i32),
    (
        SHOW_CLIPBOARD,
        Some(Feature::ClipboardHistory),
        "Show clipboard history",
        CONTROL | ALT | b'V' as i32,
    ),
    (TOGGLE_AWAKE, Some(Feature::KeepAwake), "Keep awake", CONTROL | ALT | b'A' as i32),
    (
        MUTE_EVERY_INPUT,
        Some(Feature::MicrophoneTools),
        "Mute every microphone",
        CONTROL | ALT | b'M' as i32,
    ),
    (NEXT_OUTPUT, Some(Feature::OutputSwitcher), "Next sound output", CONTROL | ALT | b'O' as i32),
    // Fires even mid-drag: KWin's drag filter eats only Escape, so this is the way to summon
    // the shelf with a file already in hand.
    (SHOW_SHELF, Some(Feature::Shelf), "Show the shelf", CONTROL | ALT | b'S' as i32),
];

/// A utility that was switched off registers nothing, so System Settings never lists a Kavverna
/// shortcut that does nothing when pressed.
fn installed() -> Vec<Shortcut> {
    KEYS.iter()
        .filter(|(_, feature, _, _)| feature.is_none_or(|feature| settings::is_installed(feature)))
        .map(|&(action, _, friendly, keys)| Shortcut { action, friendly, keys })
        .collect()
}

pub fn serve(runtime: tokio::runtime::Handle) {
    let shortcuts = installed();
    let (presses, incoming) = std::sync::mpsc::channel();

    runtime.spawn(async move {
        if let Err(err) =
            kde_bridge::shortcuts::serve(COMPONENT, FRIENDLY, &shortcuts, presses).await
        {
            tracing::error!(%err, "no global shortcuts");
        }
    });

    std::thread::spawn(move || {
        for action in incoming {
            tracing::info!(%action, "global shortcut");
            match action.as_str() {
                SHOW_PANEL => panel::toggle(),
                SHOW_CLIPBOARD => panel::open_page("clipboard"),
                TOGGLE_AWAKE => awake_state::toggle(),
                MUTE_EVERY_INPUT => {
                    mixer_state::mute_every_input(!mixer_state::every_input_muted())
                }
                NEXT_OUTPUT => {
                    mixer_state::cycle_output();
                }
                SHOW_SHELF => crate::shelf_view::toggle(),
                other => tracing::warn!(action = other, "a shortcut fired that nothing answers"),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shortcut whose action nobody handles registers a key that does nothing when pressed,
    /// which is worse than not registering it, because System Settings then shows it.
    #[test]
    fn every_declared_shortcut_has_somewhere_to_go() {
        let handled =
            [SHOW_PANEL, SHOW_CLIPBOARD, TOGGLE_AWAKE, MUTE_EVERY_INPUT, NEXT_OUTPUT, SHOW_SHELF];

        for (action, _, _, _) in KEYS {
            assert!(handled.contains(action), "{action} is registered and never answered");
        }
    }

    #[test]
    fn no_two_shortcuts_ask_for_the_same_key() {
        for (index, left) in KEYS.iter().enumerate() {
            for right in &KEYS[index + 1..] {
                assert_ne!(left.3, right.3, "{} and {} want the same key", left.0, right.0);
            }
        }
    }
}
