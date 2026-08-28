//! Prints what the mixer session sees, so its view can be compared against `pactl`.

use sound_mixer::{MixerCommand, Volume};
use std::time::Duration;

fn main() {
    let (handle, changes) = match sound_mixer::start() {
        Ok(parts) => parts,
        Err(err) => {
            eprintln!("could not start: {err}");
            return;
        }
    };

    let mut latest = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if let Ok(snapshot) = changes.recv_timeout(Duration::from_millis(200)) {
            latest = Some(snapshot);
        }
    }

    let Some(snapshot) = latest.clone() else {
        eprintln!("no snapshot arrived");
        return;
    };

    println!("OUTPUTS");
    for device in &snapshot.outputs {
        let marker = if device.is_default { "*" } else { " " };
        println!(
            "  {marker} {:>3}% {}{}  [{}]",
            device.volume.percent().round(),
            if device.muted { "muted " } else { "" },
            device.description,
            device.name
        );
    }

    println!("INPUTS");
    for device in &snapshot.inputs {
        let marker = if device.is_default { "*" } else { " " };
        println!(
            "  {marker} {:>3}% {}{}",
            device.volume.percent().round(),
            if device.muted { "muted " } else { "" },
            device.description
        );
    }

    println!("STREAMS");
    for stream in &snapshot.streams {
        println!(
            "    {:>3}% {}{}  key={}",
            stream.volume.percent().round(),
            if stream.muted { "muted " } else { "" },
            stream.name,
            stream.key
        );
    }

    // A round trip through PipeWire, so the write path is exercised too.
    if let Some(stream) = snapshot.streams.first() {
        let original = stream.volume;
        println!("\nsetting {} to 40%", stream.name);
        handle.send(MixerCommand::SetVolume {
            node_id: stream.node_id,
            volume: Volume::from_percent(40.0),
        });
        std::thread::sleep(Duration::from_millis(600));

        while let Ok(fresh) = changes.try_recv() {
            latest = Some(fresh);
        }
        if let Some(updated) =
            latest.as_ref().and_then(|s| s.streams.iter().find(|s| s.node_id == stream.node_id))
        {
            println!("read back: {}%", updated.volume.percent().round());
        }

        handle.send(MixerCommand::SetVolume { node_id: stream.node_id, volume: original });
        std::thread::sleep(Duration::from_millis(400));
        println!("restored to {}%", original.percent().round());
    }
}
