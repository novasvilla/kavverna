use crate::app_identity::{Properties, app_key, display_name};
use crate::model::{AudioDevice, AudioStream, DeviceRole, MixerSnapshot};
use crate::volume::Volume;
use libspa::param::ParamType;
use libspa::pod::deserialize::PodDeserializer;
use libspa::pod::serialize::PodSerializer;
use libspa::pod::{Object, Property, Value, ValueArray};
use libspa::utils::dict::DictRef;
use pipewire::types::ObjectType;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};

#[derive(Debug, thiserror::Error)]
pub enum MixerError {
    #[error("could not reach PipeWire: {0}")]
    Connect(String),
}

#[derive(Debug, Clone, Copy)]
pub enum MixerCommand {
    SetVolume { node_id: u32, volume: Volume },
    SetMute { node_id: u32, muted: bool },
    Stop,
}

/// Separate from the snapshot receiver so it can be shared: a receiver is not `Sync`, and
/// the whole point of this half is that any thread can drive the mixer.
pub struct MixerCommands(pipewire::channel::Sender<MixerCommand>);

impl MixerCommands {
    pub fn send(&self, command: MixerCommand) {
        if self.0.send(command).is_err() {
            tracing::warn!("the mixer session has stopped listening");
        }
    }
}

impl Drop for MixerCommands {
    fn drop(&mut self) {
        let _ = self.0.send(MixerCommand::Stop);
    }
}

/// PipeWire's objects are not `Send`, so the whole session lives on one thread and talks to
/// the rest of the app through channels.
pub fn start() -> Result<(MixerCommands, Receiver<MixerSnapshot>), MixerError> {
    let (commands, receiver) = pipewire::channel::channel();
    let (changes_out, changes) = std::sync::mpsc::channel();
    let (ready_out, ready) = std::sync::mpsc::channel::<MixerError>();

    std::thread::Builder::new()
        .name("sound-mixer".into())
        .spawn(move || match run(receiver, changes_out) {
            Ok(()) => tracing::info!("mixer session ended"),
            Err(err) => {
                tracing::error!(%err, "mixer session could not start");
                let _ = ready_out.send(err);
            }
        })
        .map_err(|err| MixerError::Connect(err.to_string()))?;

    // A failure arrives promptly; success simply never reports, so a silent channel means
    // the session is up.
    match ready.recv_timeout(std::time::Duration::from_millis(500)) {
        Ok(err) => Err(err),
        Err(_) => Ok((MixerCommands(commands), changes)),
    }
}

#[derive(Default)]
struct Tracked {
    streams: BTreeMap<u32, AudioStream>,
    stream_props: BTreeMap<u32, Properties>,
    devices: BTreeMap<u32, AudioDevice>,
    clients: BTreeMap<u32, Properties>,
    client_of: BTreeMap<u32, u32>,
    default_sink: Option<String>,
    default_source: Option<String>,
}

impl Tracked {
    /// Identity is worked out again whenever more is known: a node reaches the registry
    /// before its client, and the registry hands out fewer properties than the bound
    /// object's info event does.
    fn reidentify_node(&mut self, node_id: u32) {
        let Some(props) = self.stream_props.get(&node_id).cloned() else {
            return;
        };
        let client =
            self.client_of.get(&node_id).and_then(|owner| self.clients.get(owner)).cloned();

        let key = app_key(&props, client.as_ref());
        let name = display_name(&props, client.as_ref());

        if let Some(stream) = self.streams.get_mut(&node_id) {
            stream.key = key;
            stream.name = name;
        }
    }

    fn reidentify(&mut self, client_id: u32) {
        let affected: Vec<u32> = self
            .client_of
            .iter()
            .filter(|(_, owner)| **owner == client_id)
            .map(|(node, _)| *node)
            .collect();

        for node_id in affected {
            self.reidentify_node(node_id);
        }
    }

    fn snapshot(&self) -> MixerSnapshot {
        let mut outputs: Vec<AudioDevice> = Vec::new();
        let mut inputs: Vec<AudioDevice> = Vec::new();

        for device in self.devices.values() {
            let mut device = device.clone();
            device.is_default = match device.role {
                DeviceRole::Output => self.default_sink.as_deref() == Some(&device.name),
                DeviceRole::Input => self.default_source.as_deref() == Some(&device.name),
            };
            match device.role {
                DeviceRole::Output => outputs.push(device),
                DeviceRole::Input => inputs.push(device),
            }
        }

        outputs.sort_by(|a, b| a.description.cmp(&b.description));
        inputs.sort_by(|a, b| a.description.cmp(&b.description));

        MixerSnapshot { streams: self.streams.values().cloned().collect(), outputs, inputs }
    }
}

fn properties_of(dict: Option<&DictRef>) -> Properties {
    dict.map(|dict| {
        dict.iter().map(|(key, value)| (key.to_owned(), value.to_owned())).collect()
    })
    .unwrap_or_default()
}

fn run(
    commands: pipewire::channel::Receiver<MixerCommand>,
    changes: Sender<MixerSnapshot>,
) -> Result<(), MixerError> {
    pipewire::init();

    let main_loop = pipewire::main_loop::MainLoopRc::new(None)
        .map_err(|e| MixerError::Connect(e.to_string()))?;
    let context = pipewire::context::ContextRc::new(&main_loop, None)
        .map_err(|e| MixerError::Connect(e.to_string()))?;
    let core = context.connect_rc(None).map_err(|e| MixerError::Connect(e.to_string()))?;
    let registry = core.get_registry_rc().map_err(|e| MixerError::Connect(e.to_string()))?;

    let tracked = Rc::new(RefCell::new(Tracked::default()));
    let nodes: Rc<RefCell<BTreeMap<u32, (pipewire::node::Node, pipewire::node::NodeListener)>>> =
        Rc::new(RefCell::new(BTreeMap::new()));
    let extras: Rc<RefCell<Vec<Box<dyn std::any::Any>>>> = Rc::new(RefCell::new(Vec::new()));

    let publish = {
        let tracked = Rc::clone(&tracked);
        let changes = changes.clone();
        move || {
            let _ = changes.send(tracked.borrow().snapshot());
        }
    };

    let _registry_listener = {
        let on_global = {
            let tracked = Rc::clone(&tracked);
            let nodes = Rc::clone(&nodes);
            let extras = Rc::clone(&extras);
            let registry = registry.clone();
            let publish = publish.clone();

            move |global: &pipewire::registry::GlobalObject<&DictRef>| {
                match global.type_ {
                    ObjectType::Node => adopt_node(&registry, global, &tracked, &nodes, &publish),
                    ObjectType::Client => {
                        let props = properties_of(global.props);
                        let mut state = tracked.borrow_mut();
                        state.clients.insert(global.id, props);
                        state.reidentify(global.id);
                    }
                    ObjectType::Metadata => {
                        if let Some(handle) = watch_defaults(&registry, global, &tracked, &publish) {
                            extras.borrow_mut().push(handle);
                        }
                    }
                    _ => return,
                }
                publish();
            }
        };

        let on_remove = {
            let tracked = Rc::clone(&tracked);
            let nodes = Rc::clone(&nodes);
            let publish = publish.clone();

            move |id: u32| {
                nodes.borrow_mut().remove(&id);
                let mut state = tracked.borrow_mut();
                state.streams.remove(&id);
                state.devices.remove(&id);
                state.clients.remove(&id);
                state.client_of.remove(&id);
                state.stream_props.remove(&id);
                drop(state);
                publish();
            }
        };

        registry.add_listener_local().global(on_global).global_remove(on_remove).register()
    };

    let _receiver = {
        let nodes = Rc::clone(&nodes);
        let weak_loop = main_loop.downgrade();

        commands.attach(main_loop.loop_(), move |command| match command {
            MixerCommand::Stop => {
                if let Some(main_loop) = weak_loop.upgrade() {
                    main_loop.quit();
                }
            }
            MixerCommand::SetVolume { node_id, volume } => {
                apply(&nodes, node_id, volume_property(volume));
            }
            MixerCommand::SetMute { node_id, muted } => {
                apply(&nodes, node_id, Property::new(libspa::sys::SPA_PROP_mute, Value::Bool(muted)));
            }
        })
    };

    main_loop.run();
    Ok(())
}

fn volume_property(volume: Volume) -> Property {
    Property::new(
        libspa::sys::SPA_PROP_channelVolumes,
        Value::ValueArray(ValueArray::Float(vec![volume.amplitude(); 2])),
    )
}

fn apply(
    nodes: &Rc<RefCell<BTreeMap<u32, (pipewire::node::Node, pipewire::node::NodeListener)>>>,
    node_id: u32,
    property: Property,
) {
    let nodes = nodes.borrow();
    let Some((node, _)) = nodes.get(&node_id) else {
        tracing::warn!(node_id, "the node went away before the change reached it");
        return;
    };

    let object = Value::Object(Object {
        type_: libspa::sys::SPA_TYPE_OBJECT_Props,
        id: libspa::sys::SPA_PARAM_Props,
        properties: vec![property],
    });

    let mut bytes = std::io::Cursor::new(Vec::new());
    if PodSerializer::serialize(&mut bytes, &object).is_err() {
        tracing::error!(node_id, "could not encode the change");
        return;
    }

    let bytes = bytes.into_inner();
    if let Some(pod) = libspa::pod::Pod::from_bytes(&bytes) {
        node.set_param(ParamType::Props, 0, pod);
    }
}

fn adopt_node(
    registry: &pipewire::registry::RegistryRc,
    global: &pipewire::registry::GlobalObject<&DictRef>,
    tracked: &Rc<RefCell<Tracked>>,
    nodes: &Rc<RefCell<BTreeMap<u32, (pipewire::node::Node, pipewire::node::NodeListener)>>>,
    publish: &(impl Fn() + Clone + 'static),
) {
    let props = properties_of(global.props);
    let class = props.get("media.class").map(String::as_str).unwrap_or_default();

    let role = match class {
        "Stream/Output/Audio" => None,
        "Audio/Sink" => Some(DeviceRole::Output),
        "Audio/Source" => Some(DeviceRole::Input),
        _ => return,
    };

    let Ok(node) = registry.bind::<pipewire::node::Node, _>(global) else {
        return;
    };

    let id = global.id;
    {
        let mut state = tracked.borrow_mut();
        match role {
            None => {
                let client = props
                    .get("client.id")
                    .and_then(|value| value.parse::<u32>().ok())
                    .and_then(|client_id| {
                        state.client_of.insert(id, client_id);
                        state.clients.get(&client_id).cloned()
                    });

                state.stream_props.insert(id, props.clone());
                state.streams.insert(
                    id,
                    AudioStream {
                        node_id: id,
                        key: app_key(&props, client.as_ref()),
                        name: display_name(&props, client.as_ref()),
                        volume: Volume::default(),
                        muted: false,
                        target: props.get("target.object").cloned(),
                    },
                );
            }
            Some(role) => {
                let name = props.get("node.name").cloned().unwrap_or_default();
                state.devices.insert(
                    id,
                    AudioDevice {
                        node_id: id,
                        role,
                        description: props
                            .get("node.description")
                            .cloned()
                            .unwrap_or_else(|| name.clone()),
                        name,
                        volume: Volume::default(),
                        muted: false,
                        is_default: false,
                    },
                );
            }
        }
    }

    let listener = {
        let tracked = Rc::clone(tracked);
        let publish = publish.clone();
        let on_info = {
            let tracked = Rc::clone(&tracked);
            let publish = publish.clone();
            move |info: &pipewire::node::NodeInfoRef| {
                let full = properties_of(info.props());
                if full.is_empty() {
                    return;
                }
                let mut state = tracked.borrow_mut();
                if state.streams.contains_key(&id) {
                    state.stream_props.insert(id, full);
                    state.reidentify_node(id);
                }
                drop(state);
                publish();
            }
        };

        node.add_listener_local()
            .info(on_info)
            .param(move |_, param_type, _, _, pod| {
                if param_type != ParamType::Props {
                    return;
                }
                let Some(pod) = pod else { return };
                let Some((volume, muted)) = read_props(pod.as_bytes()) else { return };

                let mut state = tracked.borrow_mut();
                if let Some(stream) = state.streams.get_mut(&id) {
                    if let Some(volume) = volume {
                        stream.volume = volume;
                    }
                    if let Some(muted) = muted {
                        stream.muted = muted;
                    }
                } else if let Some(device) = state.devices.get_mut(&id) {
                    if let Some(volume) = volume {
                        device.volume = volume;
                    }
                    if let Some(muted) = muted {
                        device.muted = muted;
                    }
                }
                drop(state);
                publish();
            })
            .register()
    };

    node.subscribe_params(&[ParamType::Props]);
    nodes.borrow_mut().insert(id, (node, listener));
}

/// Reads the two fields the mixer cares about, ignoring everything else in the object.
fn read_props(bytes: &[u8]) -> Option<(Option<Volume>, Option<bool>)> {
    let (_, value) = PodDeserializer::deserialize_any_from(bytes).ok()?;
    let Value::Object(object) = value else { return None };

    let mut volume = None;
    let mut muted = None;

    for property in object.properties {
        match (property.key, property.value) {
            (libspa::sys::SPA_PROP_channelVolumes, Value::ValueArray(ValueArray::Float(levels))) => {
                if let Some(level) = levels.first() {
                    volume = Some(Volume::from_amplitude(*level));
                }
            }
            (libspa::sys::SPA_PROP_mute, Value::Bool(value)) => muted = Some(value),
            _ => {}
        }
    }

    Some((volume, muted))
}

/// The default sink and source live in the session's `default` metadata rather than on the
/// devices themselves.
fn watch_defaults(
    registry: &pipewire::registry::RegistryRc,
    global: &pipewire::registry::GlobalObject<&DictRef>,
    tracked: &Rc<RefCell<Tracked>>,
    publish: &(impl Fn() + Clone + 'static),
) -> Option<Box<dyn std::any::Any>> {
    let props = properties_of(global.props);
    if props.get("metadata.name").map(String::as_str) != Some("default") {
        return None;
    }

    let metadata = registry.bind::<pipewire::metadata::Metadata, _>(global).ok()?;
    let tracked = Rc::clone(tracked);
    let publish = publish.clone();

    let listener = metadata
        .add_listener_local()
        .property(move |_, key, _, value| {
            let Some(key) = key else { return 0 };
            let name = value.and_then(parse_default_name);

            let mut state = tracked.borrow_mut();
            match key {
                "default.audio.sink" => state.default_sink = name,
                "default.audio.source" => state.default_source = name,
                _ => return 0,
            }
            drop(state);
            publish();
            0
        })
        .register();

    Some(Box::new((metadata, listener)))
}

/// The metadata value is JSON of the shape `{"name":"alsa_output..."}`.
fn parse_default_name(value: &str) -> Option<String> {
    let start = value.find("\"name\"")?;
    let rest = &value[start + 6..];
    let open = rest.find('"')? + 1;
    let tail = &rest[open..];
    let close = tail.find('"')?;
    Some(tail[..close].to_owned())
}

#[cfg(test)]
mod tests {
    use super::parse_default_name;

    #[test]
    fn the_default_device_name_is_read_out_of_the_metadata_json() {
        assert_eq!(
            parse_default_name(r#"{"name":"alsa_output.pci-0000_0c_00.6.iec958-stereo"}"#),
            Some("alsa_output.pci-0000_0c_00.6.iec958-stereo".to_owned())
        );
        assert_eq!(
            parse_default_name(r#"{ "name" : "headset" }"#),
            Some("headset".to_owned())
        );
    }

    #[test]
    fn anything_that_is_not_a_name_is_ignored() {
        assert_eq!(parse_default_name("null"), None);
        assert_eq!(parse_default_name("{}"), None);
    }
}
