//! Where each application plays and records, as the settings remember it. One entry per
//! routed application, "app key<TAB>device node.name<TAB>device description"; an application
//! with no entry follows the default, so returning to the default removes the entry. Pure, so
//! every rule here is tested without PipeWire.

use sound_mixer::ChosenDevice;

fn app_of(line: &str) -> Option<&str> {
    line.split('\t').next()
}

pub fn chosen(entries: &[String], app: &str) -> Option<ChosenDevice> {
    entries.iter().find_map(|line| {
        let mut parts = line.splitn(3, '\t');
        if parts.next()? != app {
            return None;
        }
        let name = parts.next()?.to_owned();
        // An entry written without a description still names its device.
        let description = parts.next().unwrap_or_default();
        let description =
            if description.is_empty() { name.clone() } else { description.to_owned() };
        Some(ChosenDevice { name, description })
    })
}

/// Replaces rather than grows: one route per application.
pub fn with_route(entries: Vec<String>, app: &str, device: &ChosenDevice) -> Vec<String> {
    let mut kept: Vec<String> =
        entries.into_iter().filter(|line| app_of(line) != Some(app)).collect();
    kept.push(format!("{app}\t{}\t{}", device.name, device.description));
    kept
}

pub fn without_route(entries: Vec<String>, app: &str) -> Vec<String> {
    entries.into_iter().filter(|line| app_of(line) != Some(app)).collect()
}

/// One stream that might need moving, and whether it may be.
pub struct StreamAt {
    pub node_id: u32,
    pub app: String,
    pub movable: bool,
}

/// The streams to move on this snapshot: every movable stream of an application whose chosen
/// device is present, and which either just appeared itself or whose device just returned.
/// A steady snapshot moves nothing, for the same reason the preferred input only claims on
/// arrival: reasserting constantly would make choosing anything else impossible.
pub fn moves_due(
    entries: &[String],
    streams: &[StreamAt],
    known_stream_ids: &[u32],
    devices_before: &[String],
    devices_now: &[String],
) -> Vec<(u32, String)> {
    streams
        .iter()
        .filter(|stream| stream.movable)
        .filter_map(|stream| {
            let choice = chosen(entries, &stream.app)?;
            if !devices_now.iter().any(|name| *name == choice.name) {
                return None;
            }
            let stream_is_new = !known_stream_ids.contains(&stream.node_id);
            let device_is_back = !devices_before.iter().any(|name| *name == choice.name);
            (stream_is_new || device_is_back).then_some((stream.node_id, choice.name))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headset() -> ChosenDevice {
        ChosenDevice { name: "alsa_output.usb-headset".into(), description: "USB headset".into() }
    }

    fn at(node_id: u32, app: &str) -> StreamAt {
        StreamAt { node_id, app: app.into(), movable: true }
    }

    #[test]
    fn a_route_is_stored_and_found_by_application() {
        let entries = with_route(Vec::new(), "spotify", &headset());
        assert_eq!(chosen(&entries, "spotify"), Some(headset()));
        assert_eq!(chosen(&entries, "firefox"), None);
    }

    #[test]
    fn choosing_again_replaces_rather_than_grows() {
        let other =
            ChosenDevice { name: "alsa_output.hdmi".into(), description: "Television".into() };
        let entries = with_route(with_route(Vec::new(), "spotify", &headset()), "spotify", &other);

        assert_eq!(entries.len(), 1);
        assert_eq!(chosen(&entries, "spotify"), Some(other));
    }

    #[test]
    fn choosing_the_default_removes_the_entry() {
        let entries = with_route(Vec::new(), "spotify", &headset());
        assert!(without_route(entries, "spotify").is_empty());
    }

    #[test]
    fn an_entry_without_a_description_still_reads() {
        let entries = vec!["spotify\talsa_output.usb-headset".to_owned()];
        let choice = chosen(&entries, "spotify").expect("a device");
        assert_eq!(choice.description, "alsa_output.usb-headset");
    }

    #[test]
    fn a_stream_that_just_appeared_is_routed_to_its_stored_device() {
        let entries = with_route(Vec::new(), "spotify", &headset());
        let due = moves_due(
            &entries,
            &[at(42, "spotify")],
            &[],
            &["alsa_output.usb-headset".into()],
            &["alsa_output.usb-headset".into()],
        );

        assert_eq!(due, vec![(42, "alsa_output.usb-headset".to_owned())]);
    }

    #[test]
    fn a_device_coming_back_moves_its_applications() {
        let entries = with_route(Vec::new(), "spotify", &headset());
        let due = moves_due(
            &entries,
            &[at(42, "spotify")],
            &[42],
            &[],
            &["alsa_output.usb-headset".into()],
        );

        assert_eq!(due, vec![(42, "alsa_output.usb-headset".to_owned())]);
    }

    #[test]
    fn a_steady_snapshot_moves_nothing() {
        let entries = with_route(Vec::new(), "spotify", &headset());
        let due = moves_due(
            &entries,
            &[at(42, "spotify")],
            &[42],
            &["alsa_output.usb-headset".into()],
            &["alsa_output.usb-headset".into()],
        );

        assert!(due.is_empty());
    }

    #[test]
    fn a_stored_device_still_away_moves_nothing() {
        let entries = with_route(Vec::new(), "spotify", &headset());
        let due = moves_due(&entries, &[at(42, "spotify")], &[], &[], &[]);

        assert!(due.is_empty());
    }

    #[test]
    fn an_anchored_stream_is_never_moved() {
        let entries = with_route(Vec::new(), "spotify", &headset());
        let held = StreamAt { node_id: 42, app: "spotify".into(), movable: false };
        let due = moves_due(&entries, &[held], &[], &[], &["alsa_output.usb-headset".into()]);

        assert!(due.is_empty());
    }
}
