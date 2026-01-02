use anyhow::Result;
use log::{info, warn};

use super::super::state::{WaylandState, WaylandStateInit};
use super::WaylandBackend;
use super::setup::WaylandSetup;
use super::tray::process_tray_action;
use crate::{capture::CaptureManager, config::Config, input::InputState};

mod config;
mod input_state;
mod output;
mod session;
#[cfg(tablet)]
mod tablet;

pub(super) struct BackendRuntime {
    pub(super) conn: wayland_client::Connection,
    pub(super) event_queue: wayland_client::EventQueue<WaylandState>,
    pub(super) qh: wayland_client::QueueHandle<WaylandState>,
    pub(super) state: WaylandState,
}

pub(super) fn init_state(backend: &WaylandBackend, setup: WaylandSetup) -> Result<BackendRuntime> {
    let config::LoadedConfig {
        config,
        source,
        exit_after_capture_mode,
    } = config::load(backend.exit_after_capture_mode);
    let config_dir = Config::config_directory_from_source(&source)?;
    let session_options = session::build_session_options(&config, &config_dir);
    let output_prefs = output::resolve(&config);

    #[cfg(tablet)]
    let tablet_manager = tablet::bind_tablet_manager(&setup, &config);

    let mut input_state = input_state::build_input_state(&config);
    apply_initial_mode(backend, &mut input_state);

    let capture_manager = CaptureManager::new(backend.tokio_runtime.handle());
    info!("Capture manager initialized");

    let tokio_handle = backend.tokio_runtime.handle().clone();

    let frozen_supported = setup.layer_shell_available;
    let freeze_on_start = if backend.freeze_on_start && !frozen_supported {
        warn!("Frozen mode is not supported on GNOME xdg fallback; ignoring --freeze");
        false
    } else {
        backend.freeze_on_start
    };

    let mut state = WaylandState::new(WaylandStateInit {
        globals: setup.state_globals,
        config,
        input_state,
        capture_manager,
        session_options,
        tokio_handle,
        exit_after_capture_mode,
        frozen_enabled: frozen_supported,
        preferred_output_identity: output_prefs.preferred_output_identity,
        xdg_fullscreen: output_prefs.xdg_fullscreen,
        pending_freeze_on_start: freeze_on_start,
        screencopy_manager: setup.screencopy_manager,
        #[cfg(tablet)]
        tablet_manager,
    });

    // Ensure pinned toolbars are created immediately if visible on startup.
    state.sync_toolbar_visibility(&setup.qh);
    // Process any pending tray action that may have been queued before overlay start.
    process_tray_action(&mut state);

    Ok(BackendRuntime {
        conn: setup.conn,
        event_queue: setup.event_queue,
        qh: setup.qh,
        state,
    })
}

fn apply_initial_mode(backend: &WaylandBackend, input_state: &mut InputState) {
    // Apply initial board from CLI (if provided).
    if let Some(initial_id) = backend.initial_mode.clone() {
        if input_state.boards.has_board(&initial_id) {
            info!("Starting on board '{}'", initial_id);
            input_state.switch_board(&initial_id);
        } else if !initial_id.is_empty() {
            warn!("Requested board '{}' not found; using default", initial_id);
        }
    }
}
