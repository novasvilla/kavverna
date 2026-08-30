use crate::{AppKey, Volume};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRole {
    Output,
    Input,
}

/// Why a stream refuses to be routed, when it does. Shown on the row rather than used to hide
/// it: a row that vanished would read as a bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// `node.dont-move`: the target metadata is ignored outright.
    RefusesToMove,
    /// `node.dont-reconnect`: the stream ends rather than change device.
    RefusesToReconnect,
}

/// One application's audio, which may be one of several streams the same application owns.
#[derive(Debug, Clone)]
pub struct AudioStream {
    pub node_id: u32,
    pub key: AppKey,
    pub name: String,
    /// The `Icon=` line of the desktop entry that named this stream, when one did.
    pub icon: Option<String>,
    /// Output plays, Input records: which kind of device the stream attaches to.
    pub role: DeviceRole,
    /// Recording what is playing, a monitor tap, which no microphone choice should touch.
    pub taps_playback: bool,
    /// Present when the stream refuses to be moved, with why.
    pub anchored: Option<Anchor>,
    pub volume: Volume,
    pub muted: bool,
    /// `node.name` of the device this stream plays through, when it has been pinned to one.
    pub target: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub node_id: u32,
    pub role: DeviceRole,
    /// Stable across restarts, unlike the node id, so this is what settings store.
    pub name: String,
    pub description: String,
    /// The graph-wide serial a numeric move target must name. Node ids are recycled and never
    /// match; a move written with one falls through silently to the default.
    pub serial: u64,
    pub volume: Volume,
    pub muted: bool,
    pub is_default: bool,
}

/// One row in the mixer. An application can hold several streams at once, and PipeWire gives
/// them nothing to tell apart: Vesktop's two playbacks are both called Playback. Showing them
/// separately gives two identical rows and no way to choose, so they move together.
#[derive(Debug, Clone)]
pub struct AudioApplication {
    pub key: AppKey,
    pub name: String,
    pub icon: Option<String>,
    pub node_ids: Vec<u32>,
    pub volume: Volume,
    pub muted: bool,
    /// Set only when every stream in the row is anchored; a mixed row keeps its selector and
    /// the streams that can move do.
    pub anchored: Option<Anchor>,
}

/// A saved routing choice, as the settings remember it: the device's stable name plus the
/// description it had, so an unplugged device is still shown by its human name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChosenDevice {
    pub name: String,
    pub description: String,
}

/// How one application's route reads on its row right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteState {
    /// The chosen device's description, or None when the application follows the default.
    pub chosen: Option<String>,
    /// The choice is away; sound falls back to the default until it returns.
    pub awaiting: bool,
}

#[derive(Debug, Clone, Default)]
pub struct MixerSnapshot {
    pub streams: Vec<AudioStream>,
    pub outputs: Vec<AudioDevice>,
    pub inputs: Vec<AudioDevice>,
}

impl MixerSnapshot {
    /// Grouped by application, in the order the streams first appear, so a new stream from an
    /// application already listed does not reorder the mixer under the pointer.
    pub fn applications(&self) -> Vec<AudioApplication> {
        self.grouped(DeviceRole::Output)
    }

    /// The applications recording right now. Monitor taps are left out: recording what is
    /// playing is not using a microphone, and no microphone choice should reach it.
    pub fn recorders(&self) -> Vec<AudioApplication> {
        self.grouped(DeviceRole::Input)
    }

    fn grouped(&self, role: DeviceRole) -> Vec<AudioApplication> {
        let mut rows: Vec<AudioApplication> = Vec::new();

        for stream in self.streams.iter().filter(|s| s.role == role && !s.taps_playback) {
            match rows.iter_mut().find(|row| row.key == stream.key) {
                Some(row) => {
                    row.node_ids.push(stream.node_id);
                    if stream.volume.percent() > row.volume.percent() {
                        row.volume = stream.volume;
                    }
                    row.muted = row.muted && stream.muted;
                    // Anchored only while every member is: prefer the harder reason so the
                    // row explains the worst case.
                    row.anchored = match (row.anchored, stream.anchored) {
                        (Some(Anchor::RefusesToMove), Some(_))
                        | (Some(_), Some(Anchor::RefusesToMove)) => Some(Anchor::RefusesToMove),
                        (Some(held), Some(_)) => Some(held),
                        _ => None,
                    };
                }
                None => rows.push(AudioApplication {
                    key: stream.key.clone(),
                    name: stream.name.clone(),
                    icon: stream.icon.clone(),
                    node_ids: vec![stream.node_id],
                    volume: stream.volume,
                    muted: stream.muted,
                    anchored: stream.anchored,
                }),
            }
        }

        rows
    }

    /// Every stream an application owns in the same role, so a change reaches all of them
    /// rather than whichever one happened to be listed, and muting an application's playback
    /// can never mute its microphone.
    pub fn streams_beside(&self, node_id: u32) -> Vec<u32> {
        let Some(owner) = self.streams.iter().find(|stream| stream.node_id == node_id) else {
            return vec![node_id];
        };
        self.streams
            .iter()
            .filter(|stream| stream.key == owner.key && stream.role == owner.role)
            .map(|stream| stream.node_id)
            .collect()
    }

    /// How an application's saved choice reads against what is plugged in right now. The
    /// choice never snaps back to default visually: away means awaiting, still named.
    pub fn route_state(&self, role: DeviceRole, chosen: Option<&ChosenDevice>) -> RouteState {
        let Some(choice) = chosen else {
            return RouteState { chosen: None, awaiting: false };
        };
        let devices = match role {
            DeviceRole::Output => &self.outputs,
            DeviceRole::Input => &self.inputs,
        };
        match devices.iter().find(|device| device.name == choice.name) {
            Some(present) => {
                RouteState { chosen: Some(present.description.clone()), awaiting: false }
            }
            None => RouteState { chosen: Some(choice.description.clone()), awaiting: true },
        }
    }

    pub fn default_output(&self) -> Option<&AudioDevice> {
        self.outputs.iter().find(|device| device.is_default)
    }

    pub fn default_input(&self) -> Option<&AudioDevice> {
        self.inputs.iter().find(|device| device.is_default)
    }

    pub fn output_named(&self, name: &str) -> Option<&AudioDevice> {
        self.outputs.iter().find(|device| device.name == name)
    }

    /// Every input is muted only when there is at least one to mute, so an empty machine
    /// does not report itself as silenced.
    pub fn every_input_muted(&self) -> bool {
        !self.inputs.is_empty() && self.inputs.iter().all(|device| device.muted)
    }

    /// The next output in the user's chosen cycle, wrapping at the end.
    pub fn next_in_cycle<'a>(&self, cycle: &'a [String]) -> Option<&'a String> {
        let present: Vec<&String> =
            cycle.iter().filter(|name| self.output_named(name).is_some()).collect();

        if present.is_empty() {
            return None;
        }

        let current = self.default_output().map(|device| &device.name);
        let at = current.and_then(|name| present.iter().position(|entry| *entry == name));

        Some(present[at.map_or(0, |index| (index + 1) % present.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, is_default: bool, muted: bool) -> AudioDevice {
        AudioDevice {
            node_id: 0,
            role: DeviceRole::Output,
            name: name.into(),
            description: name.into(),
            serial: 0,
            volume: Volume::default(),
            muted,
            is_default,
        }
    }

    fn snapshot(outputs: Vec<AudioDevice>) -> MixerSnapshot {
        MixerSnapshot { streams: Vec::new(), outputs, inputs: Vec::new() }
    }

    fn stream(node_id: u32, key: &str, name: &str, percent: f32, muted: bool) -> AudioStream {
        AudioStream {
            node_id,
            key: AppKey::from_refined(key),
            name: name.into(),
            icon: None,
            role: DeviceRole::Output,
            taps_playback: false,
            anchored: None,
            volume: Volume::from_percent(percent),
            muted,
            target: None,
        }
    }

    fn recording(node_id: u32, key: &str, taps_playback: bool) -> AudioStream {
        AudioStream {
            role: DeviceRole::Input,
            taps_playback,
            ..stream(node_id, key, key, 100.0, false)
        }
    }

    #[test]
    fn an_application_with_several_streams_is_one_row() {
        let mixer = MixerSnapshot {
            streams: vec![
                stream(67, "vesktop", "Vesktop", 60.0, false),
                stream(94, "firefox", "Firefox", 100.0, false),
                stream(111, "vesktop", "Vesktop", 40.0, false),
            ],
            ..Default::default()
        };

        let rows = mixer.applications();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "Vesktop");
        assert_eq!(rows[0].node_ids, vec![67, 111]);
        assert_eq!(rows[1].name, "Firefox");
    }

    #[test]
    fn a_row_shows_the_loudest_of_its_streams() {
        let mixer = MixerSnapshot {
            streams: vec![
                stream(1, "app", "App", 20.0, false),
                stream(2, "app", "App", 80.0, false),
            ],
            ..Default::default()
        };

        assert_eq!(mixer.applications()[0].volume.percent().round(), 80.0);
    }

    #[test]
    fn a_row_is_muted_only_when_all_of_its_streams_are() {
        let mut mixer = MixerSnapshot {
            streams: vec![
                stream(1, "app", "App", 50.0, true),
                stream(2, "app", "App", 50.0, false),
            ],
            ..Default::default()
        };
        assert!(!mixer.applications()[0].muted);

        mixer.streams[1].muted = true;
        assert!(mixer.applications()[0].muted);
    }

    #[test]
    fn a_change_reaches_every_stream_the_application_owns() {
        let mixer = MixerSnapshot {
            streams: vec![
                stream(67, "vesktop", "Vesktop", 50.0, false),
                stream(94, "firefox", "Firefox", 50.0, false),
                stream(111, "vesktop", "Vesktop", 50.0, false),
            ],
            ..Default::default()
        };

        assert_eq!(mixer.streams_beside(67), vec![67, 111]);
        assert_eq!(mixer.streams_beside(94), vec![94]);
        assert_eq!(mixer.streams_beside(999), vec![999]);
    }

    #[test]
    fn cycling_moves_to_the_next_chosen_output() {
        let mixer = snapshot(vec![
            device("speakers", true, false),
            device("headset", false, false),
            device("hdmi", false, false),
        ]);
        let cycle = vec!["speakers".to_owned(), "headset".to_owned()];

        assert_eq!(mixer.next_in_cycle(&cycle), Some(&"headset".to_owned()));
    }

    #[test]
    fn cycling_wraps_at_the_end() {
        let mixer =
            snapshot(vec![device("speakers", false, false), device("headset", true, false)]);
        let cycle = vec!["speakers".to_owned(), "headset".to_owned()];

        assert_eq!(mixer.next_in_cycle(&cycle), Some(&"speakers".to_owned()));
    }

    /// Unplugging a device should not strand the shortcut on a name that is no longer there.
    #[test]
    fn a_cycle_entry_that_has_gone_away_is_skipped() {
        let mixer = snapshot(vec![device("speakers", true, false)]);
        let cycle = vec!["speakers".to_owned(), "unplugged".to_owned()];

        assert_eq!(mixer.next_in_cycle(&cycle), Some(&"speakers".to_owned()));
    }

    #[test]
    fn cycling_with_nothing_present_does_nothing() {
        let mixer = snapshot(vec![device("speakers", true, false)]);

        assert_eq!(mixer.next_in_cycle(&["gone".to_owned()]), None);
        assert_eq!(mixer.next_in_cycle(&[]), None);
    }

    #[test]
    fn cycling_starts_somewhere_when_the_default_is_not_in_the_cycle() {
        let mixer = snapshot(vec![device("hdmi", true, false), device("headset", false, false)]);
        let cycle = vec!["headset".to_owned()];

        assert_eq!(mixer.next_in_cycle(&cycle), Some(&"headset".to_owned()));
    }

    #[test]
    fn a_recording_stream_is_a_recorder_row_not_an_application_row() {
        let mixer = MixerSnapshot {
            streams: vec![stream(1, "spotify", "Spotify", 80.0, false), recording(2, "obs", false)],
            ..Default::default()
        };

        assert_eq!(mixer.applications().len(), 1);
        assert_eq!(mixer.applications()[0].name, "Spotify");
        assert_eq!(mixer.recorders().len(), 1);
        assert_eq!(mixer.recorders()[0].key.as_str(), "obs");
    }

    #[test]
    fn a_stream_tapping_playback_is_not_a_recorder() {
        let mixer =
            MixerSnapshot { streams: vec![recording(1, "obs", true)], ..Default::default() };

        assert!(mixer.recorders().is_empty());
    }

    #[test]
    fn siblings_stay_on_their_own_side_of_the_microphone() {
        let mixer = MixerSnapshot {
            streams: vec![
                stream(1, "vesktop", "Vesktop", 50.0, false),
                recording(2, "vesktop", false),
                stream(3, "vesktop", "Vesktop", 50.0, false),
            ],
            ..Default::default()
        };

        assert_eq!(mixer.streams_beside(1), vec![1, 3]);
        assert_eq!(mixer.streams_beside(2), vec![2]);
    }

    #[test]
    fn a_route_choice_that_is_plugged_in_reads_as_that_device() {
        let mut speakers = device("speakers", true, false);
        speakers.description = "Living room speakers".into();
        let mixer = snapshot(vec![speakers]);
        let choice = ChosenDevice { name: "speakers".into(), description: "an old name".into() };

        let state = mixer.route_state(DeviceRole::Output, Some(&choice));
        assert_eq!(state.chosen.as_deref(), Some("Living room speakers"));
        assert!(!state.awaiting);
    }

    #[test]
    fn a_route_choice_that_is_away_reads_as_awaiting_under_its_own_name() {
        let mixer = snapshot(vec![device("speakers", true, false)]);
        let choice = ChosenDevice { name: "headset".into(), description: "USB headset".into() };

        let state = mixer.route_state(DeviceRole::Output, Some(&choice));
        assert_eq!(state.chosen.as_deref(), Some("USB headset"));
        assert!(state.awaiting);
    }

    #[test]
    fn no_route_choice_follows_the_default() {
        let mixer = snapshot(vec![device("speakers", true, false)]);

        let state = mixer.route_state(DeviceRole::Output, None);
        assert_eq!(state, RouteState { chosen: None, awaiting: false });
    }

    #[test]
    fn a_row_is_anchored_only_when_every_stream_is() {
        let mut held = stream(1, "app", "App", 50.0, false);
        held.anchored = Some(Anchor::RefusesToMove);
        let free = stream(2, "app", "App", 50.0, false);

        let mixer = MixerSnapshot { streams: vec![held.clone(), free], ..Default::default() };
        assert_eq!(mixer.applications()[0].anchored, None);

        let mut also_held = stream(2, "app", "App", 50.0, false);
        also_held.anchored = Some(Anchor::RefusesToReconnect);
        let mixer = MixerSnapshot { streams: vec![held, also_held], ..Default::default() };
        assert_eq!(mixer.applications()[0].anchored, Some(Anchor::RefusesToMove));
    }

    #[test]
    fn a_machine_with_no_inputs_is_not_reported_as_muted() {
        let mixer = MixerSnapshot::default();

        assert!(!mixer.every_input_muted());
    }

    #[test]
    fn every_input_muted_needs_all_of_them() {
        let mut mixer = MixerSnapshot {
            inputs: vec![device("mic", true, true), device("line", false, false)],
            ..Default::default()
        };
        assert!(!mixer.every_input_muted());

        mixer.inputs[1].muted = true;
        assert!(mixer.every_input_muted());
    }
}
