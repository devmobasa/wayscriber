//! Private protocol peer for exercising real handlers without a desktop connection.
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::{
        SeatState, pointer_constraints::PointerConstraintsState,
        relative_pointer::RelativePointerState,
    },
    shell::xdg::{XdgShell, XdgSurface, window::WindowDecorations},
    shm::Shm,
};
use wayland_client::{
    Connection, EventQueue, Proxy, globals::registry_queue_init, protocol::wl_output,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::{
    backend::runtime_wake::RuntimeWakeSource,
    session::{HomeSession, PersistenceController, SessionHome, SessionLaunch, session_target},
    state::{ProtocolGlobals, ProtocolGlobalsSeed, WaylandState, WaylandStateInit},
};

pub(in crate::backend::wayland) struct HandlerFixture {
    pub state: WaylandState,
    pub conn: Connection,
    pub queue: EventQueue<WaylandState>,
    peer: UnixStream,
    direct_capture_manager: Option<u32>,
    _wake: RuntimeWakeSource,
    _runtime: tokio::runtime::Runtime,
    _data: crate::test_temp::TempDir,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum CaptureFixtureBackend {
    Unavailable,
    Portal,
    ExtImageCopy,
    WlrScreencopy,
}

impl HandlerFixture {
    pub fn new(config: crate::config::Config) -> Self {
        Self::new_inner(config, 0, CaptureFixtureBackend::Unavailable)
    }

    pub fn with_outputs(config: crate::config::Config, output_count: u32) -> Self {
        Self::new_inner(config, output_count, CaptureFixtureBackend::Unavailable)
    }

    pub fn with_capture_output(
        config: crate::config::Config,
        backend: CaptureFixtureBackend,
    ) -> Self {
        Self::new_inner(config, 1, backend)
    }

    fn new_inner(
        config: crate::config::Config,
        output_count: u32,
        backend: CaptureFixtureBackend,
    ) -> Self {
        let (client, mut peer) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let bootstrap = std::thread::spawn(move || {
            loop {
                let request = read_request(&mut peer);
                assert_eq!(request.object, 1);
                let id = request.first_word();
                match request.opcode {
                    1 => {
                        let mut advertised = vec![
                            (1, "wl_compositor", 6),
                            (2, "wl_shm", 1),
                            (3, "xdg_wm_base", 1),
                        ];
                        for index in 0..output_count {
                            advertised.push((4 + index, "wl_output", 4));
                        }

                        if backend == CaptureFixtureBackend::ExtImageCopy {
                            advertised.push((20, "ext_image_copy_capture_manager_v1", 1));
                            advertised.push((21, "ext_output_image_capture_source_manager_v1", 1));
                        }
                        if backend == CaptureFixtureBackend::WlrScreencopy {
                            advertised.push((22, "zwlr_screencopy_manager_v1", 3));
                        }

                        for (name, interface, version) in advertised {
                            let mut body = Vec::new();
                            body.extend_from_slice(&u32::to_ne_bytes(name));
                            append_string(&mut body, interface);
                            body.extend_from_slice(&u32::to_ne_bytes(version));
                            send(&mut peer, id, 0, &body);
                        }
                    }
                    0 => {
                        send(&mut peer, id, 0, &0u32.to_ne_bytes());
                        send(&mut peer, 1, 1, &id.to_ne_bytes());
                        return peer;
                    }
                    opcode => panic!("unexpected display request {opcode}"),
                }
            }
        });
        let conn = Connection::from_socket(client).unwrap();
        let (globals, queue) = registry_queue_init::<WaylandState>(&conn).unwrap();
        let peer = bootstrap.join().unwrap();
        let qh = queue.handle();
        let protocol = ProtocolGlobals::from_seed(ProtocolGlobalsSeed {
            registry: RegistryState::new(&globals),
            compositor: CompositorState::bind(&globals, &qh).unwrap(),
            shm: Shm::bind(&globals, &qh).unwrap(),
            xdg_shell: Some(XdgShell::bind(&globals, &qh).unwrap()),
            layer_shell: None,
            activation: None,
            pointer_constraints: PointerConstraintsState::bind(&globals, &qh),
            relative_pointer: RelativePointerState::bind(&globals, &qh),
            output: OutputState::new(&globals, &qh),
            seat: SeatState::new(&globals, &qh),
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let wake = RuntimeWakeSource::new().unwrap();
        // Private store files keep tests away from the developer's own data
        // folder without changing the environment other tests read.
        let data = crate::test_temp::tempdir().unwrap();
        let onboarding =
            crate::onboarding::OnboardingStore::load_from_path(data.path().join("onboarding.toml"));
        let palette_recents = crate::palette_recents::PaletteRecentsStore::load_from_path(
            data.path().join("palette_recents.toml"),
        );

        let screencopy_manager: Option<ZwlrScreencopyManagerV1> = (backend
            == CaptureFixtureBackend::WlrScreencopy)
            .then(|| globals.bind(&qh, 1..=3, ()).unwrap());
        let ext_capture_manager: Option<ExtImageCopyCaptureManagerV1> = (backend
            == CaptureFixtureBackend::ExtImageCopy)
            .then(|| globals.bind(&qh, 1..=1, ()).unwrap());
        let direct_capture_manager = screencopy_manager
            .as_ref()
            .map(|manager| manager.id().protocol_id())
            .or_else(|| {
                ext_capture_manager
                    .as_ref()
                    .map(|manager| manager.id().protocol_id())
            });
        let ext_image_copy_managers = ext_capture_manager.map(|capture| {
            let source = globals.bind(&qh, 1..=1, ()).unwrap();

            crate::backend::wayland::frozen::ExtImageCopyManagers::new(capture, source)
        });

        // Placement follows the configured preference, as at startup without env overrides.
        let xdg_fullscreen = config.ui.xdg_fullscreen;
        let mut state = WaylandState::new(WaylandStateInit {
            ui_text: crate::ui_text::UiTextEngine::default(),
            text_measurer: crate::draw::TextMeasurer::default(),
            globals: protocol,
            input_state: crate::input::InputState::from_config(&config),
            config,
            startup_activation_token: None,
            onboarding,
            palette_recents: crate::palette_recents::PaletteRecentsWriter::new(palette_recents),
            capture_manager: crate::capture::CaptureManager::new(runtime.handle()),
            session_options: None,
            session_home: SessionHome::new(
                SessionLaunch {
                    home: HomeSession::Default,
                    preferred: None,
                    from_daemon: false,
                },
                None,
                session_target(None),
            ),
            session_config_failed: false,
            persistence: PersistenceController::start(wake.handle()).unwrap(),
            runtime_ui: None,
            runtime_ui_unavailable: None,
            runtime_wake: wake.handle(),
            tokio_handle: runtime.handle().clone(),
            exit_after_capture_mode: crate::backend::ExitAfterCaptureMode::Never,
            frozen_enabled: backend != CaptureFixtureBackend::Unavailable,
            preferred_output_identity: None,
            xdg_fullscreen,
            main_surface_uses_overlay_layer: false,
            pending_freeze_on_start: false,
            screencopy_manager,
            ext_image_copy_managers,
            portal_freeze_supported: backend == CaptureFixtureBackend::Portal,
            text_input_manager: None,
            #[cfg(feature = "tablet-input")]
            tablet_manager: None,
        });
        let surface = state.protocol.compositor().create_surface(&qh);
        let window = state.protocol.xdg_shell().unwrap().create_window(
            surface,
            WindowDecorations::None,
            &qh,
        );
        state.surface.set_xdg_window(window);
        state.surface.update_dimensions(1000, 800);
        state.surface.set_configured(true);
        state.input_state.update_screen_dimensions(1000, 800);
        Self {
            state,
            conn,
            queue,
            peer,
            direct_capture_manager,
            _wake: wake,
            _runtime: runtime,
            _data: data,
        }
    }

    /// A direct fixture must receive a real request on its advertised manager.
    /// No portal is available in these cases; a portal task cannot satisfy this proof.
    pub fn assert_direct_capture_started(&mut self) {
        let Some(manager) = self.direct_capture_manager else {
            return;
        };

        self.conn.flush().unwrap();
        while read_request(&mut self.peer).object != manager {}
    }

    /// Deliver a compositor configure for the fixture's xdg window.
    pub fn configure_xdg_window(&mut self) {
        let window = self.state.surface.xdg_window().unwrap().clone();
        let mut toplevel = Vec::new();
        for value in [0i32, 0] {
            toplevel.extend_from_slice(&value.to_ne_bytes());
        }
        // An empty states array.
        toplevel.extend_from_slice(&0u32.to_ne_bytes());

        send(
            &mut self.peer,
            window.xdg_toplevel().id().protocol_id(),
            0,
            &toplevel,
        );
        send(
            &mut self.peer,
            window.xdg_surface().id().protocol_id(),
            0,
            &1u32.to_ne_bytes(),
        );
        self.dispatch_peer_events();
    }

    fn dispatch_peer_events(&mut self) {
        self.queue.prepare_read().unwrap().read().unwrap();
        self.queue.dispatch_pending(&mut self.state).unwrap();
    }

    /// Deliver real output metadata before its surface-enter notification.
    pub fn complete_output_metadata(&mut self, name: &str) -> wl_output::WlOutput {
        let output = self.state.protocol.output().outputs().next().unwrap();
        self.complete_output_metadata_for(output, name)
    }

    pub fn complete_output_metadata_for(
        &mut self,
        output: wl_output::WlOutput,
        name: &str,
    ) -> wl_output::WlOutput {
        let object = output.id().protocol_id();
        let mut geometry = Vec::new();
        for value in [0i32, 0, 300, 200, 0] {
            geometry.extend_from_slice(&value.to_ne_bytes());
        }
        append_string(&mut geometry, "Test");
        append_string(&mut geometry, "Output");
        geometry.extend_from_slice(&0i32.to_ne_bytes());
        send(&mut self.peer, object, 0, &geometry);

        let mut mode = Vec::new();
        for value in [3u32, 1000, 800, 60_000] {
            mode.extend_from_slice(&value.to_ne_bytes());
        }
        send(&mut self.peer, object, 1, &mode);
        send(&mut self.peer, object, 3, &1i32.to_ne_bytes());
        let mut output_name = Vec::new();
        append_string(&mut output_name, name);
        send(&mut self.peer, object, 4, &output_name);
        send(&mut self.peer, object, 2, &[]);

        self.dispatch_peer_events();
        assert_eq!(
            self.state
                .protocol
                .output()
                .info(&output)
                .unwrap()
                .name
                .as_deref(),
            Some(name)
        );

        output
    }

    /// Complete the callback requested by the real hidden-frame render path.
    /// This peer never connects to the user's compositor.
    pub fn complete_main_frame(&mut self) {
        self.conn.flush().unwrap();
        let surface = self.state.surface.wl_surface().unwrap().id().protocol_id();
        let callback = loop {
            let request = read_request(&mut self.peer);
            if request.object == surface && request.opcode == 3 {
                break request.first_word();
            }
        };

        send(&mut self.peer, callback, 0, &0u32.to_ne_bytes());
        self.dispatch_peer_events();
    }
}

struct PeerRequest {
    object: u32,
    opcode: u32,
    payload: Vec<u8>,
}

impl PeerRequest {
    fn first_word(&self) -> u32 {
        u32::from_ne_bytes(self.payload[..4].try_into().unwrap())
    }
}

fn read_request(peer: &mut UnixStream) -> PeerRequest {
    let mut header = [0u8; 8];
    peer.read_exact(&mut header)
        .expect("the client did not send the expected request");
    let object = u32::from_ne_bytes(header[..4].try_into().unwrap());
    let size_opcode = u32::from_ne_bytes(header[4..].try_into().unwrap());
    let mut payload = vec![0; (size_opcode >> 16) as usize - 8];
    peer.read_exact(&mut payload).unwrap();

    PeerRequest {
        object,
        opcode: size_opcode & 0xffff,
        payload,
    }
}

fn append_string(body: &mut Vec<u8>, value: &str) {
    body.extend_from_slice(&((value.len() + 1) as u32).to_ne_bytes());
    body.extend_from_slice(value.as_bytes());
    body.push(0);
    while !body.len().is_multiple_of(4) {
        body.push(0);
    }
}

fn send(peer: &mut UnixStream, object: u32, opcode: u32, payload: &[u8]) {
    peer.write_all(&object.to_ne_bytes()).unwrap();
    peer.write_all(&(((payload.len() + 8) as u32) << 16 | opcode).to_ne_bytes())
        .unwrap();
    peer.write_all(payload).unwrap();
}
