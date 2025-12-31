use anyhow::Result;
use log::{debug, info, warn};
use std::env;

use super::super::state::{WaylandState, WaylandStateInit};
use super::WaylandBackend;
use super::helpers::resume_override_from_env;
use super::setup::WaylandSetup;
use super::tray::process_tray_action;
use crate::{
    RESUME_SESSION_ENV,
    backend::ExitAfterCaptureMode,
    capture::CaptureManager,
    config::{Config, ConfigSource, KeybindingsConfig},
    input::{BoardMode, ClickHighlightSettings, InputState},
    paths, session,
};

#[cfg(tablet)]
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_manager_v2::ZwpTabletManagerV2;

#[cfg(tablet)]
const TABLET_MANAGER_MAX_VERSION: u32 = 2;

pub(super) struct BackendRuntime {
    pub(super) conn: wayland_client::Connection,
    pub(super) event_queue: wayland_client::EventQueue<WaylandState>,
    pub(super) qh: wayland_client::QueueHandle<WaylandState>,
    pub(super) state: WaylandState,
}

pub(super) fn init_state(backend: &WaylandBackend, setup: WaylandSetup) -> Result<BackendRuntime> {
    // Load configuration
    let (config, config_source) = match Config::load() {
        Ok(loaded) => (loaded.config, loaded.source),
        Err(e) => {
            warn!("Failed to load config: {}. Using defaults.", e);
            (Config::default(), ConfigSource::Default)
        }
    };
    let exit_after_capture_mode = match backend.exit_after_capture_mode {
        ExitAfterCaptureMode::Auto if config.capture.exit_after_capture => {
            ExitAfterCaptureMode::Always
        }
        other => other,
    };

    info!("Configuration loaded");
    debug!("  Color: {:?}", config.drawing.default_color);
    debug!("  Thickness: {:.1}px", config.drawing.default_thickness);
    debug!("  Font size: {:.1}px", config.drawing.default_font_size);
    debug!("  Buffer count: {}", config.performance.buffer_count);
    debug!("  VSync: {}", config.performance.enable_vsync);
    debug!(
        "  Status bar: {} @ {:?}",
        config.ui.show_status_bar, config.ui.status_bar_position
    );
    debug!(
        "  Status bar font size: {}",
        config.ui.status_bar_style.font_size
    );
    debug!(
        "  Help overlay font size: {}",
        config.ui.help_overlay_style.font_size
    );
    #[cfg(tablet)]
    info!(
        "Tablet feature: compiled=yes, runtime_enabled={}",
        config.tablet.enabled
    );
    #[cfg(not(tablet))]
    info!("Tablet feature: compiled=no");

    let config_dir = Config::config_directory_from_source(&config_source)?;

    let display_env = env::var("WAYLAND_DISPLAY").ok();
    let resume_override = resume_override_from_env();
    let mut session_options =
        match session::options_from_config(&config.session, &config_dir, display_env.as_deref()) {
            Ok(opts) => Some(opts),
            Err(err) => {
                warn!("Session persistence disabled: {}", err);
                None
            }
        };
    match resume_override {
        Some(true) => {
            if session_options.is_none() {
                let default_base = paths::data_dir()
                    .unwrap_or_else(|| config_dir.clone())
                    .join("wayscriber");
                let display = display_env.clone().unwrap_or_else(|| "default".to_string());
                session_options = Some(session::SessionOptions::new(default_base, display));
            }
            if let Some(options) = session_options.as_mut() {
                options.persist_transparent = true;
                options.persist_whiteboard = true;
                options.persist_blackboard = true;
                options.persist_history = true;
                options.restore_tool_state = true;
                info!(
                    "Session resume forced on via {} (persisting all boards, history, tool state)",
                    RESUME_SESSION_ENV
                );
            }
        }
        Some(false) => {
            if session_options.is_some() {
                info!("Session resume disabled via {}=off", RESUME_SESSION_ENV);
            }
            session_options = None;
        }
        None => {}
    }

    if let Some(ref opts) = session_options {
        info!(
            "Session persistence: base_dir={}, per_output={}, display_id='{}', output_identity={:?}, boards[T/W/B]={}/{}/{}, history={}, max_persisted_history={:?}, restore_tool_state={}, max_file_size={} bytes, compression={:?}",
            opts.base_dir.display(),
            opts.per_output,
            opts.display_id,
            opts.output_identity(),
            opts.persist_transparent,
            opts.persist_whiteboard,
            opts.persist_blackboard,
            opts.persist_history,
            opts.max_persisted_undo_depth,
            opts.restore_tool_state,
            opts.max_file_size_bytes,
            opts.compression
        );
    } else {
        info!("Session persistence disabled (no session options available)");
    }

    #[cfg(tablet)]
    let tablet_manager = if config.tablet.enabled {
        match setup.globals.bind::<ZwpTabletManagerV2, _, _>(
            &setup.qh,
            1..=TABLET_MANAGER_MAX_VERSION,
            (),
        ) {
            Ok(manager) => {
                info!("Bound zwp_tablet_manager_v2");
                Some(manager)
            }
            Err(err) => {
                warn!("Tablet protocol not available: {}", err);
                None
            }
        }
    } else {
        debug!("Tablet input disabled in config");
        None
    };

    let preferred_output_identity = env::var("WAYSCRIBER_XDG_OUTPUT")
        .ok()
        .or_else(|| config.ui.preferred_output.clone());
    if let Some(ref output) = preferred_output_identity {
        info!(
            "Preferring xdg fullscreen on output '{}' (env or config override)",
            output
        );
    }
    let mut xdg_fullscreen = env::var("WAYSCRIBER_XDG_FULLSCREEN")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(config.ui.xdg_fullscreen);
    let desktop_env = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let force_fullscreen = env::var("WAYSCRIBER_XDG_FULLSCREEN_FORCE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if xdg_fullscreen && desktop_env.to_uppercase().contains("GNOME") && !force_fullscreen {
        warn!(
            "GNOME fullscreen xdg fallback is opaque; falling back to maximized. Set WAYSCRIBER_XDG_FULLSCREEN_FORCE=1 to force fullscreen anyway."
        );
        xdg_fullscreen = false;
    }

    // Create font descriptor from config
    let font_descriptor = crate::draw::FontDescriptor::new(
        config.drawing.font_family.clone(),
        config.drawing.font_weight.clone(),
        config.drawing.font_style.clone(),
    );

    // Build keybinding action map
    let action_map = match config.keybindings.build_action_map() {
        Ok(map) => map,
        Err(err) => {
            warn!(
                "Invalid keybindings config: {}. Falling back to defaults.",
                err
            );
            KeybindingsConfig::default()
                .build_action_map()
                .unwrap_or_else(|err| {
                    warn!(
                        "Failed to build default keybindings: {}. Continuing with no bindings.",
                        err
                    );
                    std::collections::HashMap::new()
                })
        }
    };

    // Initialize input state with config defaults
    let mut input_state = InputState::with_defaults(
        config.drawing.default_color.to_color(),
        config.drawing.default_thickness,
        config.drawing.default_eraser_size,
        config.drawing.default_eraser_mode,
        config.drawing.marker_opacity,
        config.drawing.default_fill_enabled,
        config.drawing.default_font_size,
        font_descriptor,
        config.drawing.text_background_enabled,
        config.arrow.length,
        config.arrow.angle_degrees,
        config.arrow.head_at_end,
        config.ui.show_status_bar,
        config.board.clone(),
        action_map,
        config.session.max_shapes_per_frame,
        ClickHighlightSettings::from(&config.ui.click_highlight),
        config.history.undo_all_delay_ms,
        config.history.redo_all_delay_ms,
        config.history.custom_section_enabled,
        config.history.custom_undo_delay_ms,
        config.history.custom_redo_delay_ms,
        config.history.custom_undo_steps,
        config.history.custom_redo_steps,
    );

    input_state.set_hit_test_tolerance(config.drawing.hit_test_tolerance);
    input_state.set_hit_test_threshold(config.drawing.hit_test_linear_threshold);
    input_state.set_undo_stack_limit(config.drawing.undo_stack_limit);
    input_state.set_context_menu_enabled(config.ui.context_menu.enabled);

    // Initialize toolbar visibility from pinned config
    input_state.init_toolbar_from_config(
        config.ui.toolbar.layout_mode,
        config.ui.toolbar.mode_overrides.clone(),
        config.ui.toolbar.top_pinned,
        config.ui.toolbar.side_pinned,
        config.ui.toolbar.use_icons,
        config.ui.toolbar.show_more_colors,
        config.ui.toolbar.show_actions_section,
        config.ui.toolbar.show_actions_advanced,
        config.ui.toolbar.show_presets,
        config.ui.toolbar.show_step_section,
        config.ui.toolbar.show_text_controls,
        config.ui.toolbar.show_settings_section,
        config.ui.toolbar.show_delay_sliders,
        config.ui.toolbar.show_marker_opacity_section,
        config.ui.toolbar.show_preset_toasts,
        config.ui.toolbar.show_tool_preview,
    );
    input_state.init_presets_from_config(&config.presets);

    // Apply initial mode from CLI (if provided) or config default (only if board modes enabled)
    if config.board.enabled {
        let initial_mode_str = backend
            .initial_mode
            .clone()
            .unwrap_or_else(|| config.board.default_mode.clone());

        if let Ok(mode) = initial_mode_str.parse::<BoardMode>() {
            if mode != BoardMode::Transparent {
                info!("Starting in {} mode", initial_mode_str);
                input_state.canvas_set.switch_mode(mode);
                // Apply auto-color adjustment if enabled
                if config.board.auto_adjust_pen
                    && let Some(default_color) = mode.default_pen_color(&config.board)
                {
                    input_state.current_color = default_color;
                }
            }
        } else if !initial_mode_str.is_empty() {
            warn!(
                "Invalid board mode '{}', using transparent",
                initial_mode_str
            );
        }
    } else if backend.initial_mode.is_some() {
        warn!("Board modes disabled in config, ignoring --mode flag");
    }

    // Create capture manager with runtime handle
    let capture_manager = CaptureManager::new(backend.tokio_runtime.handle());
    info!("Capture manager initialized");

    // Clone runtime handle for state
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
        preferred_output_identity,
        xdg_fullscreen,
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
