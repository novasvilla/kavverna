use crate::{AppKey, Volume};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRole {
    Output,
    Input,
}

/// One application's audio, which may be one of several streams the same application owns.
#[derive(Debug, Clone)]
pub struct AudioStream {
    pub node_id: u32,
    pub key: AppKey,
    pub name: String,
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
    pub volume: Volume,
    pub muted: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Default)]
pub struct MixerSnapshot {
    pub streams: Vec<AudioStream>,
    pub outputs: Vec<AudioDevice>,
    pub inputs: Vec<AudioDevice>,
}

impl MixerSnapshot {
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
            volume: Volume::default(),
            muted,
            is_default,
        }
    }

    fn snapshot(outputs: Vec<AudioDevice>) -> MixerSnapshot {
        MixerSnapshot { streams: Vec::new(), outputs, inputs: Vec::new() }
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
        let mixer = snapshot(vec![device("speakers", false, false), device("headset", true, false)]);
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
    fn a_machine_with_no_inputs_is_not_reported_as_muted() {
        let mixer = MixerSnapshot::default();

        assert!(!mixer.every_input_muted());
    }

    #[test]
    fn every_input_muted_needs_all_of_them() {
        let mut mixer = MixerSnapshot::default();
        mixer.inputs = vec![device("mic", true, true), device("line", false, false)];
        assert!(!mixer.every_input_muted());

        mixer.inputs[1].muted = true;
        assert!(mixer.every_input_muted());
    }
}
