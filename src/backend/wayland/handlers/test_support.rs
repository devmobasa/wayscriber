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
    shell::xdg::{XdgShell, window::WindowDecorations},
    shm::Shm,
};
use wayland_client::{Connection, EventQueue, globals::registry_queue_init};

use crate::backend::wayland::{
    backend::runtime_wake::RuntimeWakeSource,
    session::{HomeSession, PersistenceController, SessionHome, SessionLaunch, session_target},
    state::{ProtocolGlobals, ProtocolGlobalsSeed, WaylandState, WaylandStateInit},
};

pub(in crate::backend::wayland) struct HandlerFixture {
    pub state: WaylandState,
    pub conn: Connection,
    pub queue: EventQueue<WaylandState>,
    _peer: UnixStream,
    _wake: RuntimeWakeSource,
    _runtime: tokio::runtime::Runtime,
    _data: crate::test_temp::TempDir,
}

impl HandlerFixture {
    pub fn new(config: crate::config::Config) -> Self {
        let (client, mut peer) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let bootstrap = std::thread::spawn(move || {
            loop {
                let mut header = [0u8; 8];
                peer.read_exact(&mut header).unwrap();
                let object = u32::from_ne_bytes(header[..4].try_into().unwrap());
                let size_opcode = u32::from_ne_bytes(header[4..].try_into().unwrap());
                let mut payload = vec![0; (size_opcode >> 16) as usize - 8];
                peer.read_exact(&mut payload).unwrap();
                assert_eq!(object, 1);
                let id = u32::from_ne_bytes(payload[..4].try_into().unwrap());
                match size_opcode & 0xffff {
                    1 => {
                        for (name, interface, version) in [
                            (1, "wl_compositor", 6),
                            (2, "wl_shm", 1),
                            (3, "xdg_wm_base", 1),
                        ] {
                            let mut body = Vec::new();
                            body.extend_from_slice(&u32::to_ne_bytes(name));
                            body.extend_from_slice(&((interface.len() + 1) as u32).to_ne_bytes());
                            body.extend_from_slice(interface.as_bytes());
                            body.push(0);
                            while body.len() % 4 != 0 {
                                body.push(0);
                            }
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
            frozen_enabled: false,
            preferred_output_identity: None,
            xdg_fullscreen: false,
            main_surface_uses_overlay_layer: false,
            pending_freeze_on_start: false,
            screencopy_manager: None,
            ext_image_copy_managers: None,
            portal_freeze_supported: false,
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
            _peer: peer,
            _wake: wake,
            _runtime: runtime,
            _data: data,
        }
    }
}

fn send(peer: &mut UnixStream, object: u32, opcode: u32, payload: &[u8]) {
    peer.write_all(&object.to_ne_bytes()).unwrap();
    peer.write_all(&(((payload.len() + 8) as u32) << 16 | opcode).to_ne_bytes())
        .unwrap();
    peer.write_all(payload).unwrap();
}
