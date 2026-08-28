//! Watching the Wayland selection, and owning it.
//!
//! `ext_data_control_manager_v1` is what a clipboard manager without a window gets on KWin. It
//! reports every selection change and lets us take the selection back. Watching and owning share
//! one connection on one thread on purpose: holding the selection means answering `send` for as
//! long as we hold it, and being the owner is the only reliable way to recognise a selection
//! change we caused ourselves.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, TryRecvError, channel, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nix::fcntl::OFlag;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use wayland_client::globals::{BindError, GlobalError, GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::{Connection, Dispatch, DispatchError, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_device_v1::{
    self, EVT_DATA_OFFER_OPCODE, ExtDataControlDeviceV1,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::ExtDataControlManagerV1;
use wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::{
    self, ExtDataControlOfferV1,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_source_v1::{
    self, ExtDataControlSourceV1,
};

/// Password managers on KDE advertise this type beside the secret. Its presence is the whole
/// signal: the content is never read, so nothing sensitive reaches this process at all.
pub const CONCEALED_HINT: &str = "x-kde-passwordManagerHint";

/// Carried on every selection we set, so the change it causes is recognisable as ours. Taking
/// the selection destroys the previous owner's offer, so a manager that re-read its own writes
/// would rewrite history on every paste.
const OWN_MARKER: &str = "application/x-kavverna-internal";

const URI_LIST: &str = "text/uri-list";
const PNG: &str = "image/png";
const UTF8_TEXT: &str = "text/plain;charset=utf-8";

/// In preference order. The first one an offer advertises is the one asked for.
const TEXT_TYPES: [&str; 5] = [UTF8_TEXT, "UTF8_STRING", "text/plain", "STRING", "TEXT"];

/// A source that never answers leaves the read blocked forever, and the thread with it.
const TRANSFER_DEADLINE: Duration = Duration::from_secs(5);

const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_URI_LIST_BYTES: usize = 1024 * 1024;

/// Wayland keeps two independent selections. The second one has no macOS counterpart: it is
/// filled by selecting text and pasted with the middle mouse button.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Selection {
    Clipboard,
    Primary,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Payload {
    Text(String),
    Image(Vec<u8>),
    Files(Vec<PathBuf>),
}

#[derive(Debug)]
pub enum SelectionEvent {
    Copied { selection: Selection, payload: Payload },
    Emptied(Selection),
}

/// Read at every selection change rather than captured once, so a setting change takes effect
/// on the next copy instead of needing the watcher restarted.
#[derive(Debug)]
pub struct CapturePolicy {
    pub images_and_files: AtomicBool,
    pub primary_selection: AtomicBool,
}

impl Default for CapturePolicy {
    fn default() -> Self {
        Self {
            images_and_files: AtomicBool::new(true),
            primary_selection: AtomicBool::new(false),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("no Wayland display: {0}")]
    NoDisplay(#[from] wayland_client::ConnectError),
    #[error("the compositor announced no globals: {0}")]
    NoGlobals(#[from] GlobalError),
    #[error("the compositor offers no clipboard access for windowless applications: {0}")]
    NoManager(#[from] BindError),
    #[error("the Wayland connection failed: {0}")]
    Connection(#[from] DispatchError),
    #[error("could not reach the watcher thread")]
    ThreadGone,
    #[error("{0}")]
    Pipe(#[from] nix::Error),
}

enum Request {
    Offer { selection: Selection, payload: Payload },
    Clear(Selection),
    Stop,
}

/// Drops the connection when it goes out of scope, which is also how the feature is switched
/// off: nothing is observed while no device is bound.
pub struct SelectionWatcher {
    requests: Sender<Request>,
    wake: OwnedFd,
    thread: Option<JoinHandle<()>>,
}

/// Where a selection change is reported. A closure rather than a channel so the caller can fold
/// these into whatever else it is already waiting on, instead of needing a thread to forward them.
pub type Report = Box<dyn Fn(SelectionEvent) + Send>;

impl SelectionWatcher {
    pub fn start(policy: Arc<CapturePolicy>, report: Report) -> Result<Self, WatchError> {
        let (requests_out, requests_in) = channel();
        let (ready_out, ready_in) = sync_channel(1);
        let (wake_read, wake_write) = nix::unistd::pipe2(OFlag::O_CLOEXEC | OFlag::O_NONBLOCK)?;

        let thread = std::thread::Builder::new()
            .name("kavverna-clipboard".into())
            .spawn(move || run(policy, report, requests_in, wake_read, ready_out))
            .map_err(|_| WatchError::ThreadGone)?;

        match ready_in.recv() {
            Ok(Ok(())) => {
                Ok(Self { requests: requests_out, wake: wake_write, thread: Some(thread) })
            }
            Ok(Err(err)) => Err(err),
            Err(_) => Err(WatchError::ThreadGone),
        }
    }

    pub fn offer(&self, selection: Selection, payload: Payload) {
        self.send(Request::Offer { selection, payload });
    }

    pub fn clear(&self, selection: Selection) {
        self.send(Request::Clear(selection));
    }

    fn send(&self, request: Request) {
        if self.requests.send(request).is_ok() {
            let _ = nix::unistd::write(self.wake.as_fd(), &[1]);
        }
    }
}

impl Drop for SelectionWatcher {
    fn drop(&mut self) {
        self.send(Request::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Watcher {
    policy: Arc<CapturePolicy>,
    report: Report,
    /// Mime types arrive one event at a time before the offer becomes the selection.
    offered: HashMap<u32, Vec<String>>,
    clipboard_source: Option<ExtDataControlSourceV1>,
    primary_source: Option<ExtDataControlSourceV1>,
    served: HashMap<u32, Vec<u8>>,
    /// The compositor replays whatever is already on a selection the moment the device is
    /// bound. Switching history on is not a copy, so the first report of each is dropped.
    replayed: Vec<Selection>,
    stopped: bool,
}

fn run(
    policy: Arc<CapturePolicy>,
    report: Report,
    requests: Receiver<Request>,
    wake: OwnedFd,
    ready: SyncSender<Result<(), WatchError>>,
) {
    let (conn, mut queue, manager, device) = match connect(&policy) {
        Ok(parts) => {
            let _ = ready.send(Ok(()));
            parts
        }
        Err(err) => {
            let _ = ready.send(Err(err));
            return;
        }
    };

    let mut watcher = Watcher {
        policy,
        report,
        offered: HashMap::new(),
        clipboard_source: None,
        primary_source: None,
        served: HashMap::new(),
        replayed: Vec::new(),
        stopped: false,
    };
    let qh = queue.handle();

    while !watcher.stopped {
        if queue.flush().is_err() || queue.dispatch_pending(&mut watcher).is_err() {
            break;
        }

        let Some(guard) = conn.prepare_read() else {
            continue;
        };

        let display = conn.as_fd();
        let mut fds = [
            PollFd::new(display, PollFlags::POLLIN),
            PollFd::new(wake.as_fd(), PollFlags::POLLIN),
        ];
        if poll(&mut fds, PollTimeout::NONE).is_err() {
            break;
        }

        if fds[0].any().unwrap_or(false) {
            if guard.read().is_err() {
                break;
            }
        } else {
            drop(guard);
        }

        if fds[1].any().unwrap_or(false) {
            let mut sink = [0u8; 64];
            while nix::unistd::read(wake.as_fd(), &mut sink).unwrap_or(0) > 0 {}
        }

        loop {
            match requests.try_recv() {
                Ok(request) => watcher.handle(request, &manager, &device, &qh),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    watcher.stopped = true;
                    break;
                }
            }
        }
    }

    let _ = queue.flush();
}

type Parts = (
    Connection,
    wayland_client::EventQueue<Watcher>,
    ExtDataControlManagerV1,
    ExtDataControlDeviceV1,
);

fn connect(policy: &CapturePolicy) -> Result<Parts, WatchError> {
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<Watcher>(&conn)?;
    let qh = queue.handle();

    let manager: ExtDataControlManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let seat: WlSeat = globals.bind(&qh, 1..=1, ())?;
    let device = manager.get_data_device(&seat, &qh, ());

    tracing::info!(
        primary = policy.primary_selection.load(Ordering::Relaxed),
        "watching the Wayland selection"
    );
    Ok((conn, queue, manager, device))
}

impl Watcher {
    fn handle(
        &mut self,
        request: Request,
        manager: &ExtDataControlManagerV1,
        device: &ExtDataControlDeviceV1,
        qh: &QueueHandle<Self>,
    ) {
        match request {
            Request::Stop => self.stopped = true,
            Request::Clear(selection) => {
                self.forget_source(selection);
                match selection {
                    Selection::Clipboard => device.set_selection(None),
                    Selection::Primary => device.set_primary_selection(None),
                }
            }
            Request::Offer { selection, payload } => {
                self.forget_source(selection);
                let source = manager.create_data_source(qh, ());
                for mime in offered_types(&payload) {
                    source.offer(mime.to_string());
                }
                source.offer(OWN_MARKER.to_string());
                self.served.insert(source.id().protocol_id(), encode(&payload));

                match selection {
                    Selection::Clipboard => {
                        device.set_selection(Some(&source));
                        self.clipboard_source = Some(source);
                    }
                    Selection::Primary => {
                        device.set_primary_selection(Some(&source));
                        self.primary_source = Some(source);
                    }
                }
            }
        }
    }

    fn forget_source(&mut self, selection: Selection) {
        let held = match selection {
            Selection::Clipboard => self.clipboard_source.take(),
            Selection::Primary => self.primary_source.take(),
        };
        if let Some(source) = held {
            self.served.remove(&source.id().protocol_id());
            source.destroy();
        }
    }

    fn capture(&mut self, selection: Selection, offer: Option<ExtDataControlOfferV1>, conn: &Connection) {
        let replay = !self.replayed.contains(&selection);
        if replay {
            self.replayed.push(selection);
        }

        let Some(offer) = offer else {
            if !replay {
                (self.report)(SelectionEvent::Emptied(selection));
            }
            return;
        };

        if replay {
            self.offered.remove(&offer.id().protocol_id());
            offer.destroy();
            return;
        }

        let types = self.offered.remove(&offer.id().protocol_id()).unwrap_or_default();

        if types.iter().any(|mime| mime == OWN_MARKER) {
            offer.destroy();
            return;
        }
        if types.iter().any(|mime| mime == CONCEALED_HINT) {
            offer.destroy();
            tracing::debug!("a copy marked as a secret was left unread");
            return;
        }

        match self.read(&offer, &types, conn) {
            Some(payload) => (self.report)(SelectionEvent::Copied { selection, payload }),
            None => tracing::debug!(?types, "nothing worth keeping in this selection"),
        }
        offer.destroy();
    }

    /// Files win over text because a file manager also offers the names as a string, and an
    /// image wins over the text fallback for the same reason.
    fn read(
        &self,
        offer: &ExtDataControlOfferV1,
        types: &[String],
        conn: &Connection,
    ) -> Option<Payload> {
        let has = |wanted: &str| types.iter().any(|mime| mime == wanted);

        if self.policy.images_and_files.load(Ordering::Relaxed) {
            if has(URI_LIST) {
                let raw = receive(offer, URI_LIST, MAX_URI_LIST_BYTES, conn)?;
                let paths = parse_uri_list(&String::from_utf8_lossy(&raw));
                if !paths.is_empty() {
                    return Some(Payload::Files(paths));
                }
            }
            if has(PNG) {
                let raw = receive(offer, PNG, MAX_IMAGE_BYTES, conn)?;
                if !raw.is_empty() {
                    return Some(Payload::Image(raw));
                }
            }
        }

        let mime = TEXT_TYPES.iter().find(|wanted| has(wanted))?;
        let raw = receive(offer, mime, MAX_TEXT_BYTES, conn)?;
        let text = String::from_utf8_lossy(&raw).trim().to_string();
        (!text.is_empty()).then_some(Payload::Text(text))
    }
}

fn receive(
    offer: &ExtDataControlOfferV1,
    mime: &str,
    limit: usize,
    conn: &Connection,
) -> Option<Vec<u8>> {
    let (reader, writer) = nix::unistd::pipe2(OFlag::O_CLOEXEC).ok()?;
    offer.receive(mime.to_string(), writer.as_fd());

    // The request has to reach the compositor before anything can arrive down the pipe, and
    // our own copy of the write end has to go or the read never sees end of file.
    conn.flush().ok()?;
    drop(writer);

    read_until_eof(reader, limit)
}

fn read_until_eof(reader: OwnedFd, limit: usize) -> Option<Vec<u8>> {
    let deadline = Instant::now() + TRANSFER_DEADLINE;
    let mut file = std::fs::File::from(reader);
    let mut collected = Vec::new();
    let mut chunk = [0u8; 64 * 1024];

    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            tracing::warn!(mime_limit = limit, "gave up waiting for the clipboard owner");
            return None;
        }
        if !wait_readable(file.as_fd(), left) {
            continue;
        }
        match file.read(&mut chunk) {
            Ok(0) => return Some(collected),
            Ok(read) => {
                if collected.len() + read > limit {
                    tracing::debug!(limit, "clipboard content too large to keep");
                    return None;
                }
                collected.extend_from_slice(&chunk[..read]);
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
}

fn wait_readable(fd: BorrowedFd<'_>, left: Duration) -> bool {
    let millis = u16::try_from(left.as_millis().min(u16::MAX as u128)).unwrap_or(u16::MAX);
    let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];
    matches!(poll(&mut fds, PollTimeout::from(millis)), Ok(count) if count > 0)
}

fn offered_types(payload: &Payload) -> Vec<&'static str> {
    match payload {
        Payload::Text(_) => TEXT_TYPES.to_vec(),
        Payload::Image(_) => vec![PNG],
        Payload::Files(_) => vec![URI_LIST, UTF8_TEXT, "text/plain"],
    }
}

fn encode(payload: &Payload) -> Vec<u8> {
    match payload {
        Payload::Text(text) => text.as_bytes().to_vec(),
        Payload::Image(png) => png.clone(),
        Payload::Files(paths) => uri_list(paths).into_bytes(),
    }
}

/// RFC 2483 asks for CRLF between entries, and Dolphin reads the list either way.
fn uri_list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .filter_map(|path| url::Url::from_file_path(path).ok())
        .map(|url| url.to_string())
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn parse_uri_list(raw: &str) -> Vec<PathBuf> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| url::Url::parse(line).ok())
        .filter_map(|url| url.to_file_path().ok())
        .collect()
}

fn write_all_before(fd: OwnedFd, bytes: &[u8]) {
    let mut file = std::fs::File::from(fd);
    // A consumer that stops reading closes the pipe, which is an ordinary outcome here.
    if let Err(err) = file.write_all(bytes) {
        tracing::debug!(%err, "the paste target stopped reading");
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for Watcher {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for Watcher {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlManagerV1, ()> for Watcher {
    fn event(
        _: &mut Self,
        _: &ExtDataControlManagerV1,
        _: <ExtDataControlManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for Watcher {
    fn event(
        state: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        conn: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::DataOffer { id } => {
                state.offered.insert(id.id().protocol_id(), Vec::new());
            }
            ext_data_control_device_v1::Event::Selection { id } => {
                state.capture(Selection::Clipboard, id, conn);
            }
            ext_data_control_device_v1::Event::PrimarySelection { id } => {
                if state.policy.primary_selection.load(Ordering::Relaxed) {
                    state.capture(Selection::Primary, id, conn);
                } else if let Some(offer) = id {
                    state.offered.remove(&offer.id().protocol_id());
                    offer.destroy();
                }
            }
            ext_data_control_device_v1::Event::Finished => state.stopped = true,
            _ => {}
        }
    }

    event_created_child!(Watcher, ExtDataControlDeviceV1, [
        EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for Watcher {
    fn event(
        state: &mut Self,
        offer: &ExtDataControlOfferV1,
        event: ext_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
            state.offered.entry(offer.id().protocol_id()).or_default().push(mime_type);
        }
    }
}

impl Dispatch<ExtDataControlSourceV1, ()> for Watcher {
    fn event(
        state: &mut Self,
        source: &ExtDataControlSourceV1,
        event: ext_data_control_source_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = source.id().protocol_id();
        match event {
            ext_data_control_source_v1::Event::Send { mime_type, fd } => {
                let Some(bytes) = state.served.get(&id) else {
                    return;
                };
                if mime_type == OWN_MARKER {
                    write_all_before(fd, b"");
                } else {
                    write_all_before(fd, bytes);
                }
            }
            // Another application took the selection. Ours is dead and must not be destroyed
            // twice, so it is forgotten here rather than in the next offer.
            ext_data_control_source_v1::Event::Cancelled => {
                state.served.remove(&id);
                if state.clipboard_source.as_ref().map(|held| held.id().protocol_id()) == Some(id) {
                    state.clipboard_source = None;
                }
                if state.primary_source.as_ref().map(|held| held.id().protocol_id()) == Some(id) {
                    state.primary_source = None;
                }
                source.destroy();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uri_list_survives_a_round_trip() {
        let paths = vec![PathBuf::from("/home/novas/a file.txt"), PathBuf::from("/tmp/b.png")];
        assert_eq!(parse_uri_list(&uri_list(&paths)), paths);
    }

    #[test]
    fn comments_and_blank_lines_are_not_paths() {
        let raw = "# comment\r\n\r\nfile:///tmp/one.txt\r\n";
        assert_eq!(parse_uri_list(raw), vec![PathBuf::from("/tmp/one.txt")]);
    }

    #[test]
    fn a_text_payload_is_offered_as_every_text_type() {
        let offered = offered_types(&Payload::Text("hello".into()));
        assert!(offered.contains(&UTF8_TEXT));
        assert!(offered.contains(&"STRING"));
    }

    #[test]
    fn files_are_offered_as_a_uri_list_first() {
        assert_eq!(offered_types(&Payload::Files(vec![])).first(), Some(&URI_LIST));
    }
}
