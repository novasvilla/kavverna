//! What the mixer writes, read back with `pactl` rather than with the mixer's own snapshot.
//!
//! Reading a write back through the library that performed it proves only that the library
//! agrees with itself. That is how per application routing came to be described in a feature
//! summary while not working: the snapshot said the write had landed and nothing else was asked.
//! `pactl` reaches the same daemon through the PulseAudio compatibility layer, which is a
//! different client and a different code path, so it can disagree.
//!
//! Every test here changes the machine's real audio and puts it back. They need a live PipeWire
//! session, so they carry `#[ignore]` and run with `-- --include-ignored`.

use sound_mixer::{DeviceRole, MixerCommand, MixerCommands, MixerSnapshot, StreamTarget, Volume};
use std::sync::mpsc::Receiver;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// One at a time. These drive the machine's real mixer, and two of them running at once would
/// each see the other's writes.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn in_turn() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `LC_ALL=C` because the percentages and decibels come back with a comma for a decimal point
/// under a Spanish locale, and this parses them.
fn pactl(arguments: &[&str]) -> String {
    let output = std::process::Command::new("pactl")
        .env("LC_ALL", "C")
        .args(arguments)
        .output()
        .expect("pactl is not installed, so there is nothing to check the mixer against");

    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The first percentage on a `Volume:` line. Every channel is written together, so one is enough.
fn first_percent(line: &str) -> Option<u32> {
    line.split('/').nth(1)?.trim().trim_end_matches('%').trim().parse().ok()
}

/// `pactl` keys a stream by its own serial and carries the PipeWire node id beside it as
/// `object.id`, which is what the mixer addresses. So the two can be lined up exactly rather
/// than by guessing from an application name.
fn stream_reading(node_id: u32) -> Option<(u32, bool)> {
    let listing = pactl(&["list", "sink-inputs"]);

    for block in listing.split("Sink Input #").skip(1) {
        let mut percent = None;
        let mut muted = None;
        for line in block.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("Volume:") {
                percent = first_percent(rest);
            } else if let Some(rest) = line.strip_prefix("Mute:") {
                muted = Some(rest.trim() == "yes");
            } else if line.starts_with("object.id = ") {
                let found: u32 =
                    line.trim_start_matches("object.id = ").trim_matches('"').parse().ok()?;
                if found == node_id {
                    return percent.zip(muted);
                }
            }
        }
    }

    None
}

fn sink_percent(name: &str) -> Option<u32> {
    first_percent(pactl(&["get-sink-volume", name]).lines().next()?.strip_prefix("Volume:")?)
}

/// The device a stream is attached to right now, and whether a target was pinned on it before
/// this test touched anything. `pactl`'s sink and source indexes are PipeWire object serials,
/// which is also what the first column of the short listings carries, so name and serial line
/// up exactly.
fn stream_attachment(listing_of: &str, node_id: u32) -> Option<(u32, bool)> {
    let listing = pactl(&["list", listing_of]);
    let heading = if listing_of == "sink-inputs" { "Sink Input #" } else { "Source Output #" };
    let device_line = if listing_of == "sink-inputs" { "Sink:" } else { "Source:" };

    for block in listing.split(heading).skip(1) {
        let mut serial = None;
        let mut pinned = false;
        let mut ours = false;
        for line in block.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix(device_line) {
                serial = rest.trim().parse().ok();
            } else if line.starts_with("target.object = ") {
                pinned = true;
            } else if line.starts_with("object.id = ") {
                let found: Option<u32> =
                    line.trim_start_matches("object.id = ").trim_matches('"').parse().ok();
                ours = found == Some(node_id);
            }
        }
        if ours {
            return serial.map(|serial| (serial, pinned));
        }
    }

    None
}

/// serial to name, from the short listing, so a move's readback can be matched to the device
/// the mixer aimed at.
fn devices_by_serial(kind: &str) -> Vec<(u32, String)> {
    pactl(&["list", kind, "short"])
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let serial = fields.next()?.trim().parse().ok()?;
            Some((serial, fields.next()?.trim().to_owned()))
        })
        .collect()
}

fn source_muted(name: &str) -> Option<bool> {
    Some(pactl(&["get-source-mute", name]).trim() == "Mute: yes")
}

fn connect() -> (MixerCommands, Receiver<MixerSnapshot>) {
    sound_mixer::start().expect("no PipeWire session, so there is nothing to write to")
}

/// The playback stream a test may safely disturb. A stream started for the run is preferred,
/// so `paplay --volume=0 silence.wav &` keeps the tests off whatever the person at the
/// machine is actually listening to; whatever plays first is the fallback.
fn playback_subject(snapshot: &MixerSnapshot) -> Option<sound_mixer::AudioStream> {
    let movable =
        |s: &&sound_mixer::AudioStream| s.role == DeviceRole::Output && s.anchored.is_none();
    snapshot
        .streams
        .iter()
        .filter(movable)
        .find(|s| s.key.as_str().contains("paplay") || s.key.as_str().contains("pw-play"))
        .or_else(|| snapshot.streams.iter().find(movable))
        .cloned()
}

/// The snapshot after the writes have been through the graph. Waits for a quiet moment rather
/// than a fixed sleep, so a busy machine is given the time it needs.
fn settle(changes: &Receiver<MixerSnapshot>) -> MixerSnapshot {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut latest = None;
    while Instant::now() < deadline {
        match changes.recv_timeout(Duration::from_millis(400)) {
            Ok(snapshot) => latest = Some(snapshot),
            Err(_) if latest.is_some() => break,
            Err(_) => {}
        }
    }
    latest.expect("no snapshot arrived, so the session never came up")
}

#[test]
#[ignore = "changes the machine's real audio and needs a live PipeWire session"]
fn a_stream_volume_reaches_pipewire() {
    let _turn = in_turn();
    let (commands, changes) = connect();
    let snapshot = settle(&changes);

    // A playback stream: capture streams are tracked too now, and sink-inputs is the only
    // listing this readback reaches.
    let Some(stream) = playback_subject(&snapshot) else {
        eprintln!("nothing is playing, so there is no stream to move");
        return;
    };
    let was = stream_reading(stream.node_id)
        .map(|(percent, _)| Volume::from_percent(percent as f32))
        .unwrap_or(stream.volume);

    commands.send(MixerCommand::SetVolume {
        node_id: stream.node_id,
        volume: Volume::from_percent(40.0),
    });
    settle(&changes);

    let landed = stream_reading(stream.node_id);
    commands.send(MixerCommand::SetVolume { node_id: stream.node_id, volume: was });
    settle(&changes);

    let (percent, _) = landed.expect("pactl does not know the stream the mixer just wrote to");
    assert!(
        (39..=41).contains(&percent),
        "the mixer set 40% and pactl reads {percent}% on node {}",
        stream.node_id
    );
}

/// Every output, not only the default one. A card with its volume in hardware behaves
/// differently from one without, and testing whichever happened to be default would have found
/// that on one machine and missed it on the next.
#[test]
#[ignore = "changes the machine's real audio and needs a live PipeWire session"]
fn an_output_volume_reaches_pipewire() {
    let _turn = in_turn();
    let (commands, changes) = connect();
    let snapshot = settle(&changes);

    if snapshot.outputs.is_empty() {
        eprintln!("no outputs, so there is nothing to write to");
        return;
    }

    let mut disagreed = Vec::new();
    for device in &snapshot.outputs {
        // The before value comes from pactl, the same place the after value does. Taking it
        // from our own snapshot once restored a level the snapshot had not learned yet.
        let Some(was) = sink_percent(&device.name) else {
            continue;
        };
        commands.send(MixerCommand::SetVolume {
            node_id: device.node_id,
            volume: Volume::from_percent(35.0),
        });
        settle(&changes);

        let landed = sink_percent(&device.name);
        commands.send(MixerCommand::SetVolume {
            node_id: device.node_id,
            volume: Volume::from_percent(was as f32),
        });
        settle(&changes);

        if !landed.is_some_and(|percent| (34..=36).contains(&percent)) {
            disagreed.push(format!("{} reads {landed:?} after 35% was written", device.name));
        }
    }

    assert!(disagreed.is_empty(), "the mixer wrote 35% and pactl disagrees: {disagreed:#?}");
}

/// The move that sank the first attempt at routing: a numeric target is matched only against
/// `object.serial`, so this reads the landing back by serial from the same listing pactl uses.
#[test]
#[ignore = "changes the machine's real audio and needs a live PipeWire session"]
fn a_stream_move_reaches_pipewire() {
    let _turn = in_turn();
    let (commands, changes) = connect();
    let snapshot = settle(&changes);

    let Some(stream) = playback_subject(&snapshot) else {
        eprintln!("nothing movable is playing, so there is nothing to move");
        return;
    };
    let Some((was_on, was_pinned)) = stream_attachment("sink-inputs", stream.node_id) else {
        eprintln!("pactl does not list the stream, so there is nothing to line up");
        return;
    };

    let sinks = devices_by_serial("sinks");
    let Some((_, target_name)) = sinks.iter().find(|(serial, _)| *serial != was_on).cloned() else {
        eprintln!("only one output, so there is nowhere to move to");
        return;
    };
    let target_serial =
        sinks.iter().find(|(_, name)| *name == target_name).map(|(serial, _)| *serial);

    commands.send(MixerCommand::MoveStream {
        node_id: stream.node_id,
        to: StreamTarget::Device(target_name.clone()),
    });
    settle(&changes);
    let landed = stream_attachment("sink-inputs", stream.node_id).map(|(serial, _)| serial);

    // Back where it was: to the very sink it sat on when pinned before, and back to
    // following the default otherwise, which also clears the pin this test just created in
    // WirePlumber's own store.
    let back = if was_pinned {
        sinks.iter().find(|(serial, _)| *serial == was_on).map(|(_, name)| name.clone())
    } else {
        None
    };
    commands.send(MixerCommand::MoveStream {
        node_id: stream.node_id,
        to: back.map_or(StreamTarget::FollowDefault, StreamTarget::Device),
    });
    settle(&changes);

    assert_eq!(
        landed, target_serial,
        "the mixer moved node {} to {target_name} and pactl reads {landed:?}",
        stream.node_id
    );
}

/// The recording twin: the same metadata write against a capture stream, read back from the
/// source-outputs listing.
#[test]
#[ignore = "changes the machine's real audio and needs a live PipeWire session"]
fn a_recording_stream_move_reaches_pipewire() {
    let _turn = in_turn();
    let (commands, changes) = connect();
    let snapshot = settle(&changes);

    let Some(stream) = snapshot
        .streams
        .iter()
        .find(|s| s.role == DeviceRole::Input && !s.taps_playback && s.anchored.is_none())
        .cloned()
    else {
        eprintln!("nothing is recording; open a microphone test and rerun for this to bite");
        return;
    };
    let Some((was_on, was_pinned)) = stream_attachment("source-outputs", stream.node_id) else {
        eprintln!("pactl does not list the recording stream");
        return;
    };

    let sources: Vec<(u32, String)> = devices_by_serial("sources")
        .into_iter()
        .filter(|(_, name)| !name.ends_with(".monitor"))
        .collect();
    let Some((target_serial, target_name)) =
        sources.iter().find(|(serial, _)| *serial != was_on).cloned()
    else {
        eprintln!("only one microphone, so there is nowhere to move to");
        return;
    };

    commands.send(MixerCommand::MoveStream {
        node_id: stream.node_id,
        to: StreamTarget::Device(target_name.clone()),
    });
    settle(&changes);
    let landed = stream_attachment("source-outputs", stream.node_id).map(|(serial, _)| serial);

    let back = if was_pinned {
        sources.iter().find(|(serial, _)| *serial == was_on).map(|(_, name)| name.clone())
    } else {
        None
    };
    commands.send(MixerCommand::MoveStream {
        node_id: stream.node_id,
        to: back.map_or(StreamTarget::FollowDefault, StreamTarget::Device),
    });
    settle(&changes);

    assert_eq!(
        landed,
        Some(target_serial),
        "the mixer moved recording node {} to {target_name} and pactl reads {landed:?}",
        stream.node_id
    );
}

/// The one that matters. Mute all reports success from our own snapshot whatever happens, and
/// on this machine a USB headset was not being muted while the snapshot said it was. Named for
/// what it proves rather than for the defect, since the defect is meant to stop existing.
#[test]
#[ignore = "changes the machine's real audio and needs a live PipeWire session"]
fn muting_every_input_reaches_pipewire() {
    let _turn = in_turn();
    let (commands, changes) = connect();
    let snapshot = settle(&changes);

    if snapshot.inputs.is_empty() {
        eprintln!("no inputs, so there is nothing to mute");
        return;
    }

    let was: Vec<bool> = snapshot
        .inputs
        .iter()
        .map(|device| source_muted(&device.name).unwrap_or(device.muted))
        .collect();
    for device in &snapshot.inputs {
        commands.send(MixerCommand::SetMute { node_id: device.node_id, muted: true });
    }
    settle(&changes);

    let disagreed: Vec<String> = snapshot
        .inputs
        .iter()
        .filter(|device| source_muted(&device.name) != Some(true))
        .map(|device| device.name.clone())
        .collect();

    for (device, muted) in snapshot.inputs.iter().zip(was) {
        commands.send(MixerCommand::SetMute { node_id: device.node_id, muted });
    }
    settle(&changes);

    assert!(
        disagreed.is_empty(),
        "the mixer muted every input and pactl still reads these as unmuted: {disagreed:?}"
    );
}
