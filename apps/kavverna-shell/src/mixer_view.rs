use crate::mixer_state;
use cxx_qt::Threading;
use cxx_qt_lib::{QList, QString, QStringList};
use sound_mixer::{MixerCommand, MixerSnapshot, Volume};
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
        #[qproperty(QList_i32, stream_ids)]
        #[qproperty(QList_i32, stream_volumes)]
        #[qproperty(QList_bool, stream_muted)]
        #[qproperty(QStringList, output_names)]
        #[qproperty(QList_i32, output_ids)]
        #[qproperty(QList_i32, output_volumes)]
        #[qproperty(QList_bool, output_muted)]
        #[qproperty(QString, default_output)]
        #[qproperty(i32, default_output_id)]
        #[qproperty(QStringList, input_names)]
        #[qproperty(QList_i32, input_ids)]
        #[qproperty(QString, default_input)]
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
    }
}

use core::pin::Pin;

static VIEW: Mutex<Option<cxx_qt::CxxQtThread<qobject::MixerView>>> = Mutex::new(None);

#[derive(Default)]
pub struct MixerViewRust {
    available: bool,
    stream_names: QStringList,
    stream_ids: QList<i32>,
    stream_volumes: QList<i32>,
    stream_muted: QList<bool>,
    output_names: QStringList,
    output_ids: QList<i32>,
    output_volumes: QList<i32>,
    output_muted: QList<bool>,
    default_output: QString,
    default_output_id: i32,
    input_names: QStringList,
    input_ids: QList<i32>,
    default_input: QString,
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
        for sibling in siblings(node_id) {
            send_volume(sibling, percent);
        }
    }

    fn mute_stream(self: Pin<&mut Self>, node_id: i32, muted: bool) {
        for sibling in siblings(node_id) {
            send_mute(sibling, muted);
        }
    }

    fn set_output_volume(self: Pin<&mut Self>, node_id: i32, percent: i32) {
        send_volume(node_id, percent);
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

    fn apply(mut self: Pin<&mut Self>, snapshot: MixerSnapshot) {
        let mut names = QStringList::default();
        let mut ids = QList::<i32>::default();
        let mut volumes = QList::<i32>::default();
        let mut muted = QList::<bool>::default();

        for application in &snapshot.applications() {
            names.append(QString::from(&application.name));
            ids.append(application.node_ids[0] as i32);
            volumes.append(application.volume.percent().round() as i32);
            muted.append(application.muted);
        }

        self.as_mut().set_stream_names(names);
        self.as_mut().set_stream_ids(ids);
        self.as_mut().set_stream_volumes(volumes);
        self.as_mut().set_stream_muted(muted);

        let mut names = QStringList::default();
        let mut ids = QList::<i32>::default();
        let mut volumes = QList::<i32>::default();
        let mut muted = QList::<bool>::default();

        for device in &snapshot.outputs {
            names.append(QString::from(&device.description));
            ids.append(device.node_id as i32);
            volumes.append(device.volume.percent().round() as i32);
            muted.append(device.muted);
        }

        self.as_mut().set_output_names(names);
        self.as_mut().set_output_ids(ids);
        self.as_mut().set_output_volumes(volumes);
        self.as_mut().set_output_muted(muted);

        let default_output = snapshot.default_output();
        self.as_mut().set_default_output(QString::from(
            default_output.map_or("No output", |device| device.description.as_str()),
        ));
        self.as_mut()
            .set_default_output_id(default_output.map_or(-1, |device| device.node_id as i32));

        let mut names = QStringList::default();
        let mut ids = QList::<i32>::default();
        for device in &snapshot.inputs {
            names.append(QString::from(&device.description));
            ids.append(device.node_id as i32);
        }

        self.as_mut().set_input_names(names);
        self.as_mut().set_input_ids(ids);
        self.as_mut().set_default_input(QString::from(
            snapshot.default_input().map_or("No input", |device| device.description.as_str()),
        ));
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

fn send_volume(node_id: i32, percent: i32) {
    if let Ok(node_id) = u32::try_from(node_id) {
        mixer_state::send(MixerCommand::SetVolume {
            node_id,
            volume: Volume::from_percent(percent as f32),
        });
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
