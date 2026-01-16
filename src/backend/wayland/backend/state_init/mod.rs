use anyhow::Result;
use log::{info, warn};
use smithay_client_toolkit::globals::ProvidesBoundGlobal;

use super::super::state::{WaylandState, WaylandStateInit};
use super::WaylandBackend;
use super::setup::WaylandSetup;
use super::tray::process_tray_action;
use crate::{
    capture::CaptureManager,
    config::Config,
    input::{InputState, state::CompositorCapabilities},
    onboarding::OnboardingStore,
};

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

    // Set compositor capabilities based on detected Wayland protocols
    input_state.compositor_capabilities = CompositorCapabilities {
        layer_shell: setup.layer_shell_available,
        screencopy: setup.screencopy_manager.is_some(),
        pointer_constraints: setup
            .state_globals
            .pointer_constraints_state
            .bound_global()
            .is_ok(),
    };

    let mut onboarding = OnboardingStore::load();
    if !onboarding.state().welcome_shown {
        // Start the guided tour for new users
        input_state.start_tour();
        onboarding.state_mut().welcome_shown = true;
        onboarding.state_mut().tour_shown = true;
        onboarding.save();
    }
    apply_initial_mode(backend, &config, &mut input_state);

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
        onboarding,
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

fn apply_initial_mode(backend: &WaylandBackend, _config: &Config, input_state: &mut InputState) {
    // Apply initial board from CLI (if provided).
    if let Some(initial_id) = backend.initial_mode.clone() {
        if input_state.boards.has_board(&initial_id) {
            info!("Starting on board '{}'", initial_id);
            input_state.switch_board_force(&initial_id);
        } else if !initial_id.is_empty() {
            warn!("Requested board '{}' not found; using default", initial_id);
        }
    }
}
