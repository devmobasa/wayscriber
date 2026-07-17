use anyhow::Result;
use log::{info, warn};
use smithay_client_toolkit::globals::ProvidesBoundGlobal;
use std::env;

use super::super::state::{WaylandState, WaylandStateInit};
use super::WaylandBackend;
use super::runtime_wake::RuntimeWakeSource;
use super::setup::WaylandSetup;
use crate::backend::wayland::portal_capture::screenshot_portal_available;
use crate::env_vars::{DESKTOP_SESSION_ENV, XDG_CURRENT_DESKTOP_ENV, XDG_SESSION_DESKTOP_ENV};
use crate::{
    capture::CaptureManager,
    config::Config,
    input::InputState,
    input::state::{CompositorCapabilities, DesktopEnvironment, ShellMode},
    onboarding::{DEFERRED_HINT_REPEAT_MAX, OnboardingStore},
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
    // Declared after state so the persistence controller and worker are dropped
    // before the last runtime-owned wake source during startup-error unwinding.
    pub(super) runtime_wake: RuntimeWakeSource,
}

pub(super) fn init_state(backend: &WaylandBackend, setup: WaylandSetup) -> Result<BackendRuntime> {
    let config::LoadedConfig {
        config,
        source,
        exit_after_capture_mode,
    } = config::load(backend.exit_after_capture_mode);
    let config_dir = Config::config_directory_from_source(&source)?;
    let session_options =
        session::build_session_options(&config, &config_dir, backend.named_session_file.clone());
    let runtime_wake = RuntimeWakeSource::new()
        .map_err(|err| anyhow::anyhow!("failed to create runtime wake descriptor: {err}"))?;
    let persistence =
        crate::backend::wayland::session::PersistenceController::start(runtime_wake.handle())?;
    let output_prefs = output::resolve(&config);

    #[cfg(tablet)]
    let tablet_manager = tablet::bind_tablet_manager(&setup, &config);

    let mut input_state = input_state::build_input_state(&config);
    input_state.set_session_preflight_options(session_options.clone());
    let screencopy_supported = setup.screencopy_manager.is_some();
    let portal_freeze_supported = screenshot_portal_available(&backend.tokio_runtime);
    let frozen_supported = screencopy_supported || portal_freeze_supported;
    let tokio_handle = backend.tokio_runtime.handle().clone();

    // Set compositor capabilities based on detected Wayland protocols
    input_state.compositor_capabilities = CompositorCapabilities {
        layer_shell: setup.layer_shell_available,
        screencopy: screencopy_supported,
        freeze_capture: frozen_supported,
        pointer_constraints: setup
            .state_globals
            .pointer_constraints_state
            .bound_global()
            .is_ok(),
        desktop_environment: desktop_environment_from_env(),
        shell_mode: if setup.layer_shell_available {
            ShellMode::LayerShell
        } else if setup.state_globals.xdg_shell.is_some() {
            ShellMode::XdgFallback
        } else {
            ShellMode::Unknown
        },
    };

    let mut onboarding = OnboardingStore::load();
    {
        let state = onboarding.state_mut();
        state.sessions_seen = state.sessions_seen.saturating_add(1);
        // Re-arm deferred hints per session until each feature is actually used.
        if !state.used_help_overlay && state.hint_help_count < DEFERRED_HINT_REPEAT_MAX {
            state.hint_help_shown = false;
        }
        if !state.used_command_palette && state.hint_palette_count < DEFERRED_HINT_REPEAT_MAX {
            state.hint_palette_shown = false;
        }
        if !state.used_radial_menu
            && !state.used_context_menu_right_click
            && !state.used_context_menu_keyboard
            && state.hint_quick_access_count < DEFERRED_HINT_REPEAT_MAX
        {
            state.hint_quick_access_shown = false;
        }
        if !state.first_run_completed && !state.first_run_skipped {
            state
                .active_step
                .get_or_insert(crate::onboarding::FirstRunStep::BackgroundModeSetup);
        } else {
            state.active_step = None;
            state.quick_access_requires_toolbar = false;
        }
        // Keep legacy flags marked so older checks never re-trigger.
        state.welcome_shown = true;
        state.tour_shown = true;
    }
    onboarding.save();
    apply_initial_mode(backend, &config, &mut input_state);

    let capture_manager = CaptureManager::new(backend.tokio_runtime.handle());
    info!("Capture manager initialized");

    let freeze_on_start = if backend.freeze_on_start && !frozen_supported {
        warn!(
            "Frozen mode unavailable: no screencopy backend and no screenshot portal backend; ignoring --freeze"
        );
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
        persistence,
        tokio_handle,
        exit_after_capture_mode,
        frozen_enabled: frozen_supported,
        preferred_output_identity: output_prefs.preferred_output_identity,
        xdg_fullscreen: output_prefs.xdg_fullscreen,
        main_surface_uses_overlay_layer: output_prefs.main_surface_uses_overlay_layer,
        pending_freeze_on_start: freeze_on_start,
        screencopy_manager: setup.screencopy_manager,
        #[cfg(tablet)]
        tablet_manager,
    });

    // Decide the toolbar frontend before the first visibility sync so the
    // built-in surfaces are never created just to be torn down when the
    // GTK bars take over.
    state.spawn_gtk_toolbar_if_selected(runtime_wake.handle());
    // Ensure pinned toolbars are created immediately if visible on startup.
    state.sync_toolbar_visibility(&setup.qh);
    Ok(BackendRuntime {
        conn: setup.conn,
        event_queue: setup.event_queue,
        qh: setup.qh,
        state,
        runtime_wake,
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

fn desktop_environment_from_env() -> DesktopEnvironment {
    let values = [
        env::var(XDG_CURRENT_DESKTOP_ENV).unwrap_or_default(),
        env::var(XDG_SESSION_DESKTOP_ENV).unwrap_or_default(),
        env::var(DESKTOP_SESSION_ENV).unwrap_or_default(),
    ];
    if values
        .iter()
        .any(|value| value.to_ascii_uppercase().contains("GNOME"))
    {
        DesktopEnvironment::Gnome
    } else {
        DesktopEnvironment::Unknown
    }
}
