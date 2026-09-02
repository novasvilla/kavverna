use crate::{mixer_state, routes, settings};
use cxx_qt::Threading;
use cxx_qt_lib::{QList, QString, QStringList};
use sound_mixer::{
    Anchor, ChosenDevice, DeviceRole, MixerCommand, MixerSnapshot, StreamTarget, Volume,
};
use std::sync::Mutex;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;
        type QList_bool = cxx_qt_lib::QList<bool>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, available)]
        #[qproperty(QStringList, stream_names)]
        #[qproperty(QStringList, stream_icons)]
        #[qproperty(QList_i32, stream_ids)]
        #[qproperty(QList_i32, stream_volumes)]
        #[qproperty(QList_bool, stream_muted)]
        #[qproperty(QStringList, stream_route_labels)]
        #[qproperty(QList_i32, stream_route_device_ids)]
        #[qproperty(QStringList, stream_anchors)]
        #[qproperty(QStringList, recorder_names)]
        #[qproperty(QStringList, recorder_icons)]
        #[qproperty(QList_i32, recorder_ids)]
        #[qproperty(QStringList, recorder_route_labels)]
        #[qproperty(QList_i32, recorder_route_device_ids)]
        #[qproperty(QStringList, recorder_anchors)]
        #[qproperty(QStringList, output_names)]
        #[qproperty(QList_i32, output_ids)]
        #[qproperty(QList_i32, output_volumes)]
        #[qproperty(QList_bool, output_muted)]
        #[qproperty(QList_bool, output_in_cycle)]
        #[qproperty(QString, default_output)]
        #[qproperty(i32, default_output_id)]
        #[qproperty(QStringList, input_names)]
        #[qproperty(QList_i32, input_ids)]
        #[qproperty(QList_bool, input_preferred)]
        #[qproperty(QString, default_input)]
        #[qproperty(i32, default_input_id)]
        #[qproperty(bool, inputs_muted)]
        type MixerView = super::MixerViewRust;
    }

    impl cxx_qt::Threading for MixerView {}

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut MixerView>);
        #[qinvokable]
        fn set_stream_volume(self: Pin<&mut MixerView>, node_id: i32, percent: i32);
        #[qinvokable]
        fn mute_stream(self: Pin<&mut MixerView>, node_id: i32, muted: bool);
        #[qinvokable]
        fn set_output_volume(self: Pin<&mut MixerView>, node_id: i32, percent: i32);
        #[qinvokable]
        fn mute_output(self: Pin<&mut MixerView>, node_id: i32, muted: bool);
        #[qinvokable]
        fn mute_every_input(self: Pin<&mut MixerView>, muted: bool);
        #[qinvokable]
        fn make_default_output(self: Pin<&mut MixerView>, node_id: i32);
        #[qinvokable]
        fn make_default_input(self: Pin<&mut MixerView>, node_id: i32);
        #[qinvokable]
        fn choose_output_in_cycle(self: Pin<&mut MixerView>, node_id: i32, included: bool);
        #[qinvokable]
        fn choose_preferred_input(self: Pin<&mut MixerView>, node_id: i32, preferred: bool);
        #[qinvokable]
        fn route_stream(self: Pin<&mut MixerView>, node_id: i32, device_node_id: i32);
        #[qinvokable]
        fn route_stream_to_default(self: Pin<&mut MixerView>, node_id: i32);
        #[qinvokable]
        fn route_recorder(self: Pin<&mut MixerView>, node_id: i32, device_node_id: i32);
        #[qinvokable]
        fn route_recorder_to_default(self: Pin<&mut MixerView>, node_id: i32);
    }
}

use core::pin::Pin;

static VIEW: Mutex<Option<cxx_qt::CxxQtThread<qobject::MixerView>>> = Mutex::new(None);

#[derive(Default)]
pub struct MixerViewRust {
    available: bool,
    stream_names: QStringList,
    stream_icons: QStringList,
    stream_ids: QList<i32>,
    stream_volumes: QList<i32>,
    stream_muted: QList<bool>,
    stream_route_labels: QStringList,
    stream_route_device_ids: QList<i32>,
    stream_anchors: QStringList,
    recorder_names: QStringList,
    recorder_icons: QStringList,
    recorder_ids: QList<i32>,
    recorder_route_labels: QStringList,
    recorder_route_device_ids: QList<i32>,
    recorder_anchors: QStringList,
    output_names: QStringList,
    output_ids: QList<i32>,
    output_volumes: QList<i32>,
    output_muted: QList<bool>,
    output_in_cycle: QList<bool>,
    default_output: QString,
    default_output_id: i32,
    input_names: QStringList,
    input_ids: QList<i32>,
    input_preferred: QList<bool>,
    default_input: QString,
    default_input_id: i32,
    inputs_muted: bool,
}

impl qobject::MixerView {
    fn attach(mut self: Pin<&mut Self>) {
        let thread = self.as_mut().qt_thread();
        if let Ok(mut view) = VIEW.lock() {
            *view = Some(thread);
        }
        self.apply(mixer_state::get());
    }

    fn set_stream_volume(self: Pin<&mut Self>, node_id: i32, percent: i32) {
        let Some(volume) = Volume::for_application(percent) else {
            tracing::debug!(node_id, percent, "stream volume outside its range, ignored");
            return;
        };
        for sibling in siblings(node_id) {
            send_volume(sibling, volume);
        }
    }

    fn mute_stream(self: Pin<&mut Self>, node_id: i32, muted: bool) {
        for sibling in siblings(node_id) {
            send_mute(sibling, muted);
        }
    }

    fn set_output_volume(self: Pin<&mut Self>, node_id: i32, percent: i32) {
        match Volume::for_device(percent) {
            Some(volume) => send_volume(node_id, volume),
            None => tracing::debug!(node_id, percent, "output volume outside its range, ignored"),
        }
    }

    fn mute_output(self: Pin<&mut Self>, node_id: i32, muted: bool) {
        send_mute(node_id, muted);
    }

    fn mute_every_input(self: Pin<&mut Self>, muted: bool) {
        for device in mixer_state::get().inputs {
            mixer_state::send(MixerCommand::SetMute { node_id: device.node_id, muted });
        }
    }

    fn make_default_output(self: Pin<&mut Self>, node_id: i32) {
        if let Some(name) = device_name(node_id, true) {
            mixer_state::send(MixerCommand::MakeDefaultOutput(name));
        }
    }

    fn make_default_input(self: Pin<&mut Self>, node_id: i32) {
        if let Some(name) = device_name(node_id, false) {
            mixer_state::send(MixerCommand::MakeDefaultInput(name));
        }
    }

    fn choose_output_in_cycle(mut self: Pin<&mut Self>, node_id: i32, included: bool) {
        let Some(name) = device_name(node_id, true) else {
            return;
        };

        let mut cycle =
            settings::texts_at(settings::OUTPUT_CYCLE).unwrap_or_else(|| every_device_name(true));
        cycle.retain(|chosen| *chosen != name);
        if included {
            cycle.push(name);
        }

        settings::put_texts(settings::OUTPUT_CYCLE, &cycle);
        self.as_mut().apply(mixer_state::get());
    }

    /// One preferred input at a time, so choosing another replaces it rather than growing a
    /// list nothing knows how to pick from.
    fn choose_preferred_input(mut self: Pin<&mut Self>, node_id: i32, preferred: bool) {
        let Some(name) = device_name(node_id, false) else {
            return;
        };

        settings::put_text(settings::PREFERRED_INPUT, if preferred { &name } else { "" });
        if preferred {
            mixer_state::send(MixerCommand::MakeDefaultInput(name));
        }
        self.as_mut().apply(mixer_state::get());
    }

    fn route_stream(self: Pin<&mut Self>, node_id: i32, device_node_id: i32) {
        self.route(node_id, Some(device_node_id), DeviceRole::Output);
    }

    fn route_stream_to_default(self: Pin<&mut Self>, node_id: i32) {
        self.route(node_id, None, DeviceRole::Output);
    }

    fn route_recorder(self: Pin<&mut Self>, node_id: i32, device_node_id: i32) {
        self.route(node_id, Some(device_node_id), DeviceRole::Input);
    }

    fn route_recorder_to_default(self: Pin<&mut Self>, node_id: i32) {
        self.route(node_id, None, DeviceRole::Input);
    }

    /// Persists first, then moves every movable stream the application owns in this role. The
    /// order matters: a persisted choice survives a move that races the stream ending.
    fn route(
        mut self: Pin<&mut Self>,
        node_id: i32,
        device_node_id: Option<i32>,
        role: DeviceRole,
    ) {
        let Ok(stream_id) = u32::try_from(node_id) else {
            return;
        };
        let snapshot = mixer_state::get();
        let Some(owner) = snapshot.streams.iter().find(|s| s.node_id == stream_id) else {
            return;
        };
        let app = owner.key.as_str().to_owned();
        let key = match role {
            DeviceRole::Output => settings::PLAYBACK_ROUTES,
            DeviceRole::Input => settings::RECORDING_ROUTES,
        };
        let entries = settings::texts_at(key).unwrap_or_default();

        let target = match device_node_id {
            None => {
                settings::put_texts(key, &routes::without_route(entries, &app));
                StreamTarget::FollowDefault
            }
            Some(device_id) => {
                let Some(device) = device_named(device_id, role == DeviceRole::Output) else {
                    return;
                };
                settings::put_texts(key, &routes::with_route(entries, &app, &device));
                StreamTarget::Device(device.name)
            }
        };

        for sibling in snapshot.streams_beside(stream_id) {
            let anchored = snapshot
                .streams
                .iter()
                .find(|s| s.node_id == sibling)
                .is_some_and(|s| s.anchored.is_some());
            if !anchored {
                mixer_state::send(MixerCommand::MoveStream {
                    node_id: sibling,
                    to: target.clone(),
                });
            }
        }

        self.as_mut().apply(mixer_state::get());
    }

    fn apply(mut self: Pin<&mut Self>, snapshot: MixerSnapshot) {
        let mut names = QStringList::default();
        let mut icons = QStringList::default();
        let mut ids = QList::<i32>::default();
        let mut volumes = QList::<i32>::default();
        let mut muted = QList::<bool>::default();
        let mut route_labels = QStringList::default();
        let mut route_ids = QList::<i32>::default();
        let mut anchors = QStringList::default();

        let playback_routes = settings::texts_at(settings::PLAYBACK_ROUTES).unwrap_or_default();

        for application in &snapshot.applications() {
            names.append(QString::from(&application.name));
            // An application the desktop does not know gets the generic one rather than a hole
            // where every other row has a picture.
            icons.append(QString::from(
                application.icon.as_deref().unwrap_or("application-x-executable"),
            ));
            ids.append(application.node_ids[0] as i32);
            volumes.append(application.volume.percent().round() as i32);
            muted.append(application.muted);

            let (label, id, reason) =
                route_row(&snapshot, DeviceRole::Output, &playback_routes, application);
            route_labels.append(label);
            route_ids.append(id);
            anchors.append(reason);
        }

        self.as_mut().set_stream_names(names);
        self.as_mut().set_stream_icons(icons);
        self.as_mut().set_stream_ids(ids);
        self.as_mut().set_stream_volumes(volumes);
        self.as_mut().set_stream_muted(muted);
        self.as_mut().set_stream_route_labels(route_labels);
        self.as_mut().set_stream_route_device_ids(route_ids);
        self.as_mut().set_stream_anchors(anchors);

        let mut names = QStringList::default();
        let mut icons = QStringList::default();
        let mut ids = QList::<i32>::default();
        let mut route_labels = QStringList::default();
        let mut route_ids = QList::<i32>::default();
        let mut anchors = QStringList::default();

        let recording_routes = settings::texts_at(settings::RECORDING_ROUTES).unwrap_or_default();

        for application in &snapshot.recorders() {
            names.append(QString::from(&application.name));
            icons.append(QString::from(
                application.icon.as_deref().unwrap_or("application-x-executable"),
            ));
            ids.append(application.node_ids[0] as i32);

            let (label, id, reason) =
                route_row(&snapshot, DeviceRole::Input, &recording_routes, application);
            route_labels.append(label);
            route_ids.append(id);
            anchors.append(reason);
        }

        self.as_mut().set_recorder_names(names);
        self.as_mut().set_recorder_icons(icons);
        self.as_mut().set_recorder_ids(ids);
        self.as_mut().set_recorder_route_labels(route_labels);
        self.as_mut().set_recorder_route_device_ids(route_ids);
        self.as_mut().set_recorder_anchors(anchors);

        let mut names = QStringList::default();
        let mut ids = QList::<i32>::default();
        let mut volumes = QList::<i32>::default();
        let mut muted = QList::<bool>::default();

        // Absent means every output is in the cycle, which is what somebody who has never opened
        // this expects, and is why it is not stored as a list of all of them on first run.
        let chosen = settings::texts_at(settings::OUTPUT_CYCLE);
        let mut in_cycle = QList::<bool>::default();

        for device in &snapshot.outputs {
            names.append(QString::from(&device.description));
            ids.append(device.node_id as i32);
            volumes.append(device.volume.percent().round() as i32);
            muted.append(device.muted);
            in_cycle.append(chosen.as_ref().is_none_or(|chosen| chosen.contains(&device.name)));
        }

        self.as_mut().set_output_names(names);
        self.as_mut().set_output_ids(ids);
        self.as_mut().set_output_volumes(volumes);
        self.as_mut().set_output_muted(muted);
        self.as_mut().set_output_in_cycle(in_cycle);

        let default_output = snapshot.default_output();
        self.as_mut().set_default_output(QString::from(
            default_output.map_or("No output", |device| device.description.as_str()),
        ));
        self.as_mut()
            .set_default_output_id(default_output.map_or(-1, |device| device.node_id as i32));

        let preferred = settings::text_at(settings::PREFERRED_INPUT, "");
        let mut names = QStringList::default();
        let mut ids = QList::<i32>::default();
        let mut pinned = QList::<bool>::default();
        for device in &snapshot.inputs {
            names.append(QString::from(&device.description));
            ids.append(device.node_id as i32);
            pinned.append(device.name == preferred);
        }

        let default_input = snapshot.default_input();
        self.as_mut().set_input_names(names);
        self.as_mut().set_input_ids(ids);
        self.as_mut().set_input_preferred(pinned);
        self.as_mut().set_default_input(QString::from(
            default_input.map_or("No input", |device| device.description.as_str()),
        ));
        self.as_mut()
            .set_default_input_id(default_input.map_or(-1, |device| device.node_id as i32));
        self.as_mut().set_inputs_muted(snapshot.every_input_muted());
        self.as_mut().set_available(mixer_state::is_running());
    }
}

/// An application's streams move together, since PipeWire gives them nothing to tell apart.
fn siblings(node_id: i32) -> Vec<i32> {
    let Ok(node_id) = u32::try_from(node_id) else {
        return Vec::new();
    };
    mixer_state::get().streams_beside(node_id).into_iter().map(|id| id as i32).collect()
}

/// Devices are addressed by name rather than node id, which is recycled between runs.
fn device_name(node_id: i32, output: bool) -> Option<String> {
    let snapshot = mixer_state::get();
    let devices = if output { snapshot.outputs } else { snapshot.inputs };
    devices.into_iter().find(|device| device.node_id as i32 == node_id).map(|device| device.name)
}

/// Name and description together, which is what a stored route carries so an unplugged device
/// still shows as itself.
fn device_named(node_id: i32, output: bool) -> Option<ChosenDevice> {
    let snapshot = mixer_state::get();
    let devices = if output { snapshot.outputs } else { snapshot.inputs };
    devices
        .into_iter()
        .find(|device| device.node_id as i32 == node_id)
        .map(|device| ChosenDevice { name: device.name, description: device.description })
}

/// The three parallel values one application row needs to say where it plays: the line under
/// the name, the picker's current mark (-1 default, -2 chosen but away), and the reason it
/// cannot be moved when it cannot.
fn route_row(
    snapshot: &MixerSnapshot,
    role: DeviceRole,
    entries: &[String],
    application: &sound_mixer::AudioApplication,
) -> (QString, i32, QString) {
    let reason = QString::from(application.anchored.map_or("", |anchor| match anchor {
        Anchor::RefusesToMove => "This stream chooses its own device",
        Anchor::RefusesToReconnect => "Moving this stream would cut it off",
    }));

    let choice = routes::chosen(entries, application.key.as_str());
    let state = snapshot.route_state(role, choice.as_ref());
    let label = QString::from(state.chosen.as_deref().unwrap_or("System default"));
    let devices = match role {
        DeviceRole::Output => &snapshot.outputs,
        DeviceRole::Input => &snapshot.inputs,
    };
    let id = if state.awaiting {
        -2
    } else {
        choice
            .as_ref()
            .and_then(|choice| devices.iter().find(|device| device.name == choice.name))
            .map_or(-1, |device| device.node_id as i32)
    };

    (label, id, reason)
}

fn every_device_name(output: bool) -> Vec<String> {
    let snapshot = mixer_state::get();
    let devices = if output { snapshot.outputs } else { snapshot.inputs };
    devices.into_iter().map(|device| device.name).collect()
}

fn send_volume(node_id: i32, volume: Volume) {
    if let Ok(node_id) = u32::try_from(node_id) {
        mixer_state::send(MixerCommand::SetVolume { node_id, volume });
    }
}

fn send_mute(node_id: i32, muted: bool) {
    if let Ok(node_id) = u32::try_from(node_id) {
        mixer_state::send(MixerCommand::SetMute { node_id, muted });
    }
}

pub fn publish() {
    let Ok(view) = VIEW.lock() else {
        return;
    };
    if let Some(thread) = view.as_ref() {
        let _ = thread.queue(|view| view.apply(mixer_state::get()));
    }
}
