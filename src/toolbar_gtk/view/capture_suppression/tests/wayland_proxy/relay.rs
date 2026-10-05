//! Per-client relays between GTK and Weston, and the control socket that arms,
//! inspects, and releases the withheld popup frame callback.

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use anyhow::{Context, Result};

use super::protocol::{self, Arguments, Direction, Schema};
use super::wire::{self, Packet};
use super::{CONTROL_SOCKET, PROXY_SOCKET, ProxyResponse, ProxyStatus};

/// Accepts GTK clients on `proxy` and commands on `control` until dropped.
pub(super) struct Proxy {
    shared: Arc<Shared>,
    listeners: Vec<(PathBuf, JoinHandle<()>)>,
}

struct Shared {
    upstream: PathBuf,
    schema: Arc<Schema>,
    connections: Mutex<Vec<Arc<Connection>>>,
    forwarders: Mutex<Vec<JoinHandle<()>>>,
    stopping: AtomicBool,
}

impl Proxy {
    pub(super) fn start(directory: &Path, upstream: PathBuf, schema: Schema) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            upstream,
            schema: Arc::new(schema),
            connections: Mutex::new(Vec::new()),
            forwarders: Mutex::new(Vec::new()),
            stopping: AtomicBool::new(false),
        });

        let proxy_path = directory.join(PROXY_SOCKET);
        let clients = UnixListener::bind(&proxy_path)?;
        let control_path = directory.join(CONTROL_SOCKET);
        let commands = UnixListener::bind(&control_path)?;

        let accept_shared = Arc::clone(&shared);
        let accept = std::thread::spawn(move || accept_shared.accept_clients(clients));
        let control_shared = Arc::clone(&shared);
        let control = std::thread::spawn(move || control_shared.answer_commands(commands));

        Ok(Self {
            shared,
            listeners: vec![(proxy_path, accept), (control_path, control)],
        })
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        // Close the connections before joining the listeners: a forwarder
        // blocked on a hung peer holds its connection's state lock, and a
        // control command waiting for that lock would never let its listener
        // finish. Closing the socket wakes the forwarder.
        self.shared.shutdown_connections();
        for (path, listener) in self.listeners.drain(..) {
            // A connection wakes the blocking accept so the thread sees `stopping`.
            let wake = UnixStream::connect(&path);
            let _ = listener.join();
            drop(wake);
        }

        // No listener runs now, so the connection list is final; close any
        // accepted while the listeners were stopping.
        self.shared.shutdown_connections();
        let forwarders = std::mem::take(&mut *lock(&self.shared.forwarders));
        for forwarder in forwarders {
            let _ = forwarder.join();
        }
    }
}

impl Shared {
    fn shutdown_connections(&self) {
        for connection in lock(&self.connections).iter() {
            connection.shutdown();
        }
    }

    fn accept_clients(&self, listener: UnixListener) {
        for client in listener.incoming() {
            if self.stopping.load(Ordering::SeqCst) {
                return;
            }

            let client = client.expect("accept a proxied Wayland client");
            let server =
                UnixStream::connect(&self.upstream).expect("connect to the private Weston");
            let connection = Arc::new(Connection::new(client, server, Arc::clone(&self.schema)));
            lock(&self.connections).push(Arc::clone(&connection));

            for direction in [Direction::Request, Direction::Event] {
                let connection = Arc::clone(&connection);
                let forwarder = std::thread::spawn(move || connection.forward(direction));
                lock(&self.forwarders).push(forwarder);
            }
        }
    }

    /// Answers one line command per control connection with one JSON line.
    fn answer_commands(&self, listener: UnixListener) {
        for client in listener.incoming() {
            if self.stopping.load(Ordering::SeqCst) {
                return;
            }

            let client = client.expect("accept a proxy control client");
            let mut command = String::new();
            if BufReader::new(&client).read_line(&mut command).is_err() {
                continue;
            }

            let response = self.control(command.trim());
            let mut line = serde_json::to_string(&response).expect("serialize proxy response");
            line.push('\n');
            // A client that left without reading its response needs nothing more.
            let _ = (&client).write_all(line.as_bytes());
        }
    }

    /// Applies a command to the latest connection that has created a popup.
    fn control(&self, command: &str) -> ProxyResponse {
        let connections = lock(&self.connections).clone();
        let latest = connections
            .iter()
            .rev()
            .find(|connection| connection.has_popups());

        match latest {
            Some(connection) => connection.control(command),
            None => ProxyResponse::error("no GTK popup connection"),
        }
    }
}

struct Connection {
    client: UnixStream,
    server: UnixStream,
    schema: Arc<Schema>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    objects: HashMap<u32, String>,
    xdg_surfaces: HashMap<u32, u32>,
    popups: HashSet<u32>,
    /// Pending frame callbacks and the surfaces that requested them.
    callbacks: HashMap<u32, u32>,
    armed: bool,
    selected: Option<u32>,
    held: Vec<Packet>,
    commits: u64,
    delivered: u64,
    error: Option<String>,
}

impl Connection {
    fn new(client: UnixStream, server: UnixStream, schema: Arc<Schema>) -> Self {
        let mut state = State::default();
        state.objects.insert(1, "wl_display".to_owned());

        Self {
            client,
            server,
            schema,
            state: Mutex::new(state),
        }
    }

    fn has_popups(&self) -> bool {
        !lock(&self.state).popups.is_empty()
    }

    fn shutdown(&self) {
        let _ = self.client.shutdown(Shutdown::Both);
        let _ = self.server.shutdown(Shutdown::Both);
    }

    /// The socket a direction reads from, then the one it writes to.
    fn endpoints(&self, direction: Direction) -> (&UnixStream, &UnixStream) {
        match direction {
            Direction::Request => (&self.client, &self.server),
            Direction::Event => (&self.server, &self.client),
        }
    }

    /// Relays one direction until it ends. A source that closes, or a peer that
    /// went away, passes the end on by half-closing the destination, so the
    /// other direction still drains what is queued, such as the compositor's
    /// protocol error before it disconnects, and GTK learns when Weston resets.
    /// The half-close is harmless when the destination is the peer that left, as
    /// when the test session is killed after a passing test. Any other failure
    /// is a fixture or protocol error: the first one is printed and kept for the
    /// control socket, and the whole connection closes, so GTK loses its display
    /// and the test fails at once instead of stalling.
    fn forward(&self, direction: Direction) {
        let (_, destination) = self.endpoints(direction);

        match self.relay(direction) {
            Ok(()) => {
                let _ = destination.shutdown(Shutdown::Write);
            }
            Err(error) if is_disconnect(&error) => {
                let _ = destination.shutdown(Shutdown::Write);
            }
            Err(error) => {
                let mut state = lock(&self.state);
                if state.error.is_none() {
                    let error = format!("{error:#}");
                    eprintln!("Wayland proxy stopped relaying {direction:?}s: {error}");
                    state.error = Some(error);
                }
                drop(state);
                self.shutdown();
            }
        }
    }

    /// Passes every message on in order, except the ones `inspect` withholds.
    /// The state lock is held while sending so a release cannot interleave.
    fn relay(&self, direction: Direction) -> Result<()> {
        let (source, destination) = self.endpoints(direction);

        while let Some(packet) = wire::receive(source)? {
            let mut state = lock(&self.state);
            if state.inspect(&self.schema, &packet, direction)? {
                state.held.push(packet);
            } else {
                wire::send(destination, packet)?;
            }
        }

        Ok(())
    }

    fn control(&self, command: &str) -> ProxyResponse {
        let mut state = lock(&self.state);
        if let Some(error) = &state.error {
            return ProxyResponse::error(error);
        }

        match command {
            "arm" => {
                state.armed = true;
                state.commits = 0;
                state.delivered = 0;
            }
            "release" if state.held.is_empty() => {
                return ProxyResponse::error("no popup callback held");
            }
            "release" => {
                if let Err(error) = self.release(&mut state) {
                    let error = format!("{error:#}");
                    state.error = Some(error.clone());
                    return ProxyResponse::error(error);
                }
            }
            "status" => {}
            _ => return ProxyResponse::error("unknown control command"),
        }

        ProxyResponse::Status(ProxyStatus {
            held: !state.held.is_empty(),
            popup_commits: state.commits,
            popup_callbacks_delivered: state.delivered,
        })
    }

    /// Delivers the withheld events in their original order.
    fn release(&self, state: &mut State) -> Result<()> {
        state.selected = None;
        for packet in std::mem::take(&mut state.held) {
            state.inspect(&self.schema, &packet, Direction::Event)?;
            wire::send(&self.client, packet)?;
        }

        Ok(())
    }
}

impl State {
    /// Tracks the message and returns whether it must be withheld.
    fn inspect(&mut self, schema: &Schema, packet: &Packet, direction: Direction) -> Result<bool> {
        let object_id = packet.object_id();
        let Some(interface) = self
            .objects
            .get(&object_id)
            .and_then(|name| schema.get(name.as_str()).copied())
        else {
            return Ok(false);
        };

        let message = direction
            .messages(interface)
            .get(packet.opcode())
            .with_context(|| {
                format!(
                    "{} has no {direction:?} opcode {}",
                    interface.name,
                    packet.opcode()
                )
            })?;
        let arguments = protocol::decode(message, packet.payload())?;
        for (id, created) in &arguments.new_objects {
            self.objects.insert(*id, created.clone());
        }

        match direction {
            Direction::Request => {
                self.track_request(interface.name, message.name, object_id, &arguments)?;
                Ok(false)
            }
            Direction::Event => {
                self.track_event(interface.name, message.name, object_id, &arguments)
            }
        }
    }

    fn track_request(
        &mut self,
        interface: &str,
        name: &str,
        object_id: u32,
        arguments: &Arguments,
    ) -> Result<()> {
        match (interface, name) {
            // get_xdg_surface(id: new_id, surface: object)
            ("xdg_wm_base", "get_xdg_surface") => {
                self.xdg_surfaces
                    .insert(arguments.word(0)?, arguments.word(1)?);
            }
            ("xdg_surface", "get_popup") => {
                let surface = self
                    .xdg_surfaces
                    .get(&object_id)
                    .with_context(|| format!("get_popup on unknown xdg_surface {object_id}"))?;
                self.popups.insert(*surface);
            }
            // frame(callback: new_id)
            ("wl_surface", "frame") => {
                let callback = arguments.word(0)?;
                self.callbacks.insert(callback, object_id);
                if self.armed && self.popups.contains(&object_id) {
                    self.selected = Some(callback);
                    self.armed = false;
                }
            }
            ("wl_surface", "commit") if self.popups.contains(&object_id) => {
                self.commits += 1;
            }
            _ => {}
        }

        Ok(())
    }

    fn track_event(
        &mut self,
        interface: &str,
        name: &str,
        object_id: u32,
        arguments: &Arguments,
    ) -> Result<bool> {
        match (interface, name) {
            ("wl_callback", "done") => {
                if self.selected == Some(object_id) {
                    return Ok(true);
                }

                let surface = self.callbacks.remove(&object_id);
                if surface.is_some_and(|surface| self.popups.contains(&surface)) {
                    self.delivered += 1;
                }
            }
            // delete_id(id: uint)
            ("wl_display", "delete_id") => {
                let id = arguments.word(0)?;
                if self.selected == Some(id) {
                    return Ok(true);
                }

                self.objects.remove(&id);
            }
            _ => {}
        }

        Ok(false)
    }
}

/// Whether a relay stopped because a peer closed its end of the socket.
fn is_disconnect(error: &anyhow::Error) -> bool {
    error.downcast_ref::<io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::NotConnected
        )
    })
}

/// Relay threads report failures through `State::error`, so a poisoned lock
/// carries no extra information and cleanup must still proceed.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
