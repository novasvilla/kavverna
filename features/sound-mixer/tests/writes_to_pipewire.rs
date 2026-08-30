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

use sound_mixer::{MixerCommand, MixerCommands, MixerSnapshot, Volume};
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

fn source_muted(name: &str) -> Option<bool> {
    Some(pactl(&["get-source-mute", name]).trim() == "Mute: yes")
}

fn connect() -> (MixerCommands, Receiver<MixerSnapshot>) {
    sound_mixer::start().expect("no PipeWire session, so there is nothing to write to")
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

    let Some(stream) = snapshot.streams.first().cloned() else {
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
