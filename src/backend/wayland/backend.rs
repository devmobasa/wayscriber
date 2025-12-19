// Coordinates backend startup/shutdown and drives the event loop while delegating
// rendering & protocol state to `WaylandState` and its handler modules.
use anyhow::{Context, Result};
use log::{debug, info, warn};
#[cfg(unix)]
use signal_hook::{
    consts::signal::{SIGINT, SIGTERM, SIGUSR1, SIGUSR2},
    iterator::Signals,
};
use smithay_client_toolkit::globals::ProvidesBoundGlobal;
use smithay_client_toolkit::{
    activation::ActivationState,
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::{
        SeatState, pointer_constraints::PointerConstraintsState,
        relative_pointer::RelativePointerState,
    },
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, Layer, LayerShell},
        xdg::{XdgShell, window::WindowDecorations},
    },
    shm::Shm,
};
#[cfg(unix)]
use std::thread;
use std::{
    env, fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[cfg(tablet)]
const TABLET_MANAGER_MAX_VERSION: u32 = 2;
use wayland_client::{Connection, globals::registry_queue_init};
#[cfg(tablet)]
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_manager_v2::ZwpTabletManagerV2;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use super::state::WaylandState;
use crate::{
    RESUME_SESSION_ENV,
    capture::{CaptureManager, CaptureOutcome},
    config::{Action, Config, ConfigSource},
    input::{BoardMode, ClickHighlightSettings, InputState},
    notification, paths, runtime_session_override, session,
};

fn friendly_capture_error(error: &str) -> String {
    let lower = error.to_lowercase();

    if lower.contains("requestcancelled") || lower.contains("cancelled") {
        "Screen capture cancelled by user".to_string()
    } else if lower.contains("permission") {
        "Permission denied. Enable screen sharing in system settings.".to_string()
    } else if lower.contains("busy") {
        "Screen capture in progress. Try again in a moment.".to_string()
    } else {
        "Screen capture failed. Please try again.".to_string()
    }
}

fn process_tray_action(state: &mut WaylandState) {
    let action_path = crate::paths::tray_action_file();
    // Read-and-delete best effort; if a new action is written between read and delete
    // it will be picked up on the next signal/start.
    let action_str = match fs::read_to_string(&action_path) {
        Ok(content) => content.lines().next().unwrap_or("").trim().to_string(),
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    "Tray action signal received but failed to read {}: {}",
                    action_path.display(),
                    err
                );
            }
            return;
        }
    };

    if action_str.is_empty() {
        let _ = fs::remove_file(&action_path);
        return;
    }

    match action_str.as_str() {
        "toggle_freeze" => {
            state.input_state.request_frozen_toggle();
            state.input_state.needs_redraw = true;
        }
        "capture_full" => state.handle_capture_action(Action::CaptureFullScreen),
        "capture_window" => state.handle_capture_action(Action::CaptureActiveWindow),
        "capture_region" => {
            // Honor clipboard preference for region captures
            if state.config.capture.copy_to_clipboard {
                state.handle_capture_action(Action::CaptureClipboardRegion);
            } else {
                state.handle_capture_action(Action::CaptureFileRegion);
            }
        }
        "toggle_help" => {
            state.input_state.show_help = !state.input_state.show_help;
            state.input_state.dirty_tracker.mark_full();
            state.input_state.needs_redraw = true;
        }
        other => warn!("Unknown tray action '{}'", other),
    }

    let _ = fs::remove_file(&action_path);
}

fn resume_override_from_env() -> Option<bool> {
    if let Some(runtime) = runtime_session_override() {
        return Some(runtime);
    }
    match env::var(RESUME_SESSION_ENV) {
        Ok(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" | "resume" | "enable" | "enabled" => Some(true),
                "0" | "false" | "no" | "off" | "disable" | "disabled" => Some(false),
                _ => {
                    warn!(
                        "Ignoring invalid {} value '{}'; expected on/off/true/false",
                        RESUME_SESSION_ENV, raw
                    );
                    None
                }
            }
        }
        Err(_) => None,
    }
}

/// Wayland backend state
pub struct WaylandBackend {
    initial_mode: Option<String>,
    freeze_on_start: bool,
    /// Tokio runtime for async capture operations
    tokio_runtime: tokio::runtime::Runtime,
}

impl WaylandBackend {
    pub fn new(initial_mode: Option<String>, freeze_on_start: bool) -> Result<Self> {
        let tokio_runtime = tokio::runtime::Runtime::new()
            .context("Failed to create Tokio runtime for capture operations")?;
        Ok(Self {
            initial_mode,
            freeze_on_start,
            tokio_runtime,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        info!("Starting Wayland backend");

        // Connect to Wayland compositor
        let conn =
            Connection::connect_to_env().context("Failed to connect to Wayland compositor")?;
        debug!("Connected to Wayland display");

        // Initialize registry and event queue
        let (globals, mut event_queue) =
            registry_queue_init(&conn).context("Failed to initialize Wayland registry")?;
        let qh = event_queue.handle();

        // Bind global interfaces
        let compositor_state =
            CompositorState::bind(&globals, &qh).context("wl_compositor not available")?;
        debug!("Bound compositor");

        let layer_shell = match LayerShell::bind(&globals, &qh) {
            Ok(shell) => {
                debug!("Bound layer shell");
                Some(shell)
            }
            Err(err) => {
                let desktop_env =
                    env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into());
                let session_env =
                    env::var("XDG_SESSION_DESKTOP").unwrap_or_else(|_| "unknown".into());
                warn!(
                    "Layer shell not available: {} (desktop='{}', session='{}'); toolbars will be disabled and xdg fallback may not cover docks/panels.",
                    err, desktop_env, session_env
                );
                None
            }
        };

        let xdg_shell = match XdgShell::bind(&globals, &qh) {
            Ok(shell) => {
                debug!("Bound xdg-shell");
                Some(shell)
            }
            Err(err) => {
                warn!("xdg-shell not available: {}", err);
                None
            }
        };

        let activation = match ActivationState::bind(&globals, &qh) {
            Ok(state) => {
                debug!("Bound xdg-activation");
                Some(state)
            }
            Err(err) => {
                debug!("xdg-activation not available: {}", err);
                None
            }
        };

        if layer_shell.is_none() && xdg_shell.is_none() {
            return Err(anyhow::anyhow!(
                "Wayland compositor does not expose layer-shell or xdg-shell protocols"
            ));
        }

        let shm = Shm::bind(&globals, &qh).context("wl_shm not available")?;
        debug!("Bound shared memory");

        let output_state = OutputState::new(&globals, &qh);
        debug!("Initialized output state");

        let seat_state = SeatState::new(&globals, &qh);
        debug!("Initialized seat state");

        let registry_state = RegistryState::new(&globals);
        let pointer_constraints_state = PointerConstraintsState::bind(&globals, &qh);
        let relative_pointer_state = RelativePointerState::bind(&globals, &qh);
        if pointer_constraints_state.bound_global().is_ok() {
            debug!("Pointer constraints global available");
        } else {
            debug!("Pointer constraints global not available");
        }

        let screencopy_manager = match globals.bind::<ZwlrScreencopyManagerV1, _, _>(&qh, 1..=3, ())
        {
            Ok(manager) => {
                debug!("Bound zwlr_screencopy_manager_v1");
                Some(manager)
            }
            Err(err) => {
                warn!(
                    "zwlr_screencopy_manager_v1 not available (frozen mode disabled): {}",
                    err
                );
                None
            }
        };

        // Load configuration
        let (config, config_source) = match Config::load() {
            Ok(loaded) => (loaded.config, loaded.source),
            Err(e) => {
                warn!("Failed to load config: {}. Using defaults.", e);
                (Config::default(), ConfigSource::Default)
            }
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
        let mut session_options = match session::options_from_config(
            &config.session,
            &config_dir,
            display_env.as_deref(),
        ) {
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
            match globals.bind::<ZwpTabletManagerV2, _, _>(&qh, 1..=TABLET_MANAGER_MAX_VERSION, ())
            {
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
        let action_map = config
            .keybindings
            .build_action_map()
            .expect("Failed to build keybinding action map");

        // Initialize input state with config defaults
        let mut input_state = InputState::with_defaults(
            config.drawing.default_color.to_color(),
            config.drawing.default_thickness,
            config.drawing.default_eraser_size,
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
            config.ui.toolbar.top_pinned,
            config.ui.toolbar.side_pinned,
            config.ui.toolbar.use_icons,
            config.ui.toolbar.show_more_colors,
            config.ui.toolbar.show_actions_section,
            config.ui.toolbar.show_delay_sliders,
            config.ui.toolbar.show_marker_opacity_section,
        );

        // Apply initial mode from CLI (if provided) or config default (only if board modes enabled)
        if config.board.enabled {
            let initial_mode_str = self
                .initial_mode
                .clone()
                .unwrap_or_else(|| config.board.default_mode.clone());

            if let Ok(mode) = initial_mode_str.parse::<BoardMode>() {
                if mode != BoardMode::Transparent {
                    info!("Starting in {} mode", initial_mode_str);
                    input_state.canvas_set.switch_mode(mode);
                    // Apply auto-color adjustment if enabled
                    if config.board.auto_adjust_pen {
                        if let Some(default_color) = mode.default_pen_color(&config.board) {
                            input_state.current_color = default_color;
                        }
                    }
                }
            } else if !initial_mode_str.is_empty() {
                warn!(
                    "Invalid board mode '{}', using transparent",
                    initial_mode_str
                );
            }
        } else if self.initial_mode.is_some() {
            warn!("Board modes disabled in config, ignoring --mode flag");
        }

        // Create capture manager with runtime handle
        let capture_manager = CaptureManager::new(self.tokio_runtime.handle());
        info!("Capture manager initialized");

        // Clone runtime handle for state
        let tokio_handle = self.tokio_runtime.handle().clone();

        let frozen_supported = layer_shell.is_some();

        let freeze_on_start = if self.freeze_on_start && !frozen_supported {
            warn!("Frozen mode is not supported on GNOME xdg fallback; ignoring --freeze");
            false
        } else {
            self.freeze_on_start
        };

        // Create application state
        let mut state = WaylandState::new(
            registry_state,
            compositor_state,
            layer_shell,
            xdg_shell,
            activation,
            shm,
            pointer_constraints_state,
            relative_pointer_state,
            output_state,
            seat_state,
            config,
            input_state,
            capture_manager,
            session_options,
            tokio_handle,
            frozen_supported,
            preferred_output_identity,
            xdg_fullscreen,
            freeze_on_start,
            screencopy_manager,
            #[cfg(tablet)]
            tablet_manager,
        );

        // Ensure pinned toolbars are created immediately if visible on startup.
        state.sync_toolbar_visibility(&qh);
        // Process any pending tray action that may have been queued before overlay start.
        process_tray_action(&mut state);

        // Gracefully exit the overlay when external signals request termination
        #[cfg(unix)]
        let (exit_flag, tray_action_flag): (
            Option<Arc<AtomicBool>>,
            Option<Arc<AtomicBool>>,
        ) = {
            let exit_flag = Arc::new(AtomicBool::new(false));
            let tray_action_flag = Arc::new(AtomicBool::new(false));
            match Signals::new([SIGTERM, SIGINT, SIGUSR1, SIGUSR2]) {
                Ok(mut signals) => {
                    let exit_flag_clone = Arc::clone(&exit_flag);
                    let tray_action_flag_clone = Arc::clone(&tray_action_flag);
                    thread::spawn(move || {
                        for sig in signals.forever() {
                            match sig {
                                SIGUSR2 => {
                                    debug!("Overlay received SIGUSR2 for tray action");
                                    tray_action_flag_clone.store(true, Ordering::Release);
                                }
                                _ => {
                                    debug!(
                                        "Overlay received signal {}; scheduling graceful shutdown",
                                        sig
                                    );
                                    exit_flag_clone.store(true, Ordering::Release);
                                }
                            }
                        }
                    });
                    (Some(exit_flag), Some(tray_action_flag))
                }
                Err(err) => {
                    warn!("Failed to register overlay signal handlers: {}", err);
                    (Some(exit_flag), Some(tray_action_flag))
                }
            }
        };

        #[cfg(not(unix))]
        let (exit_flag, tray_action_flag): (
            Option<Arc<AtomicBool>>,
            Option<Arc<AtomicBool>>,
        ) = (None, None);

        // Create surface using layer-shell when available, otherwise fall back to xdg-shell
        let wl_surface = state.compositor_state.create_surface(&qh);
        if let Some(layer_shell) = state.layer_shell.as_ref() {
            info!("Creating layer shell surface");
            let layer_surface = layer_shell.create_layer_surface(
                &qh,
                wl_surface,
                Layer::Top,
                Some("wayscriber"),
                None, // Default output
            );

            // Configure the layer surface for fullscreen overlay
            layer_surface.set_anchor(Anchor::all());
            let desired_keyboard_mode = state.desired_keyboard_interactivity();
            layer_surface.set_keyboard_interactivity(desired_keyboard_mode);
            layer_surface.set_size(0, 0); // Use full screen size
            layer_surface.set_exclusive_zone(-1);

            // Commit the surface
            layer_surface.commit();

            state.surface.set_layer_surface(layer_surface);
            state.set_current_keyboard_interactivity(Some(desired_keyboard_mode));
            info!("Layer shell surface created");
        } else if let Some(xdg_shell) = state.xdg_shell.as_ref() {
            info!("Layer shell missing; creating xdg-shell window");
            let window = xdg_shell.create_window(wl_surface, WindowDecorations::None, &qh);
            window.set_title("wayscriber overlay");
            window.set_app_id("com.devmobasa.wayscriber");
            if state.xdg_fullscreen() {
                if let Some(output) = state.preferred_fullscreen_output() {
                    info!("Requesting fullscreen on preferred output");
                    window.set_fullscreen(Some(&output));
                } else {
                    info!("Preferred output unknown; requesting compositor-chosen fullscreen");
                    window.set_fullscreen(None);
                }
            } else {
                window.set_maximized();
            }
            window.commit();
            state.surface.set_xdg_window(window);
            state.request_xdg_activation(&qh);
            info!("xdg-shell window created");
        } else {
            return Err(anyhow::anyhow!(
                "No supported Wayland shell protocol available"
            ));
        }

        // Track consecutive render failures for error recovery
        let mut consecutive_render_failures = 0u32;
        const MAX_RENDER_FAILURES: u32 = 10;

        // Main event loop
        let mut loop_error: Option<anyhow::Error> = None;
        loop {
            if exit_flag
                .as_ref()
                .map(|flag| flag.load(Ordering::Acquire))
                .unwrap_or(false)
            {
                state.input_state.should_exit = true;
            }

            // Check if we should exit before blocking
            if state.input_state.should_exit {
                info!("Exit requested, breaking event loop");
                break;
            }

            // Apply any completed portal fallback captures without blocking.
            state
                .frozen
                .poll_portal_capture(&mut state.surface, &mut state.input_state);
            state
                .zoom
                .poll_portal_capture(&mut state.surface, &mut state.input_state);

            if tray_action_flag
                .as_ref()
                .map(|flag| flag.swap(false, Ordering::AcqRel))
                .unwrap_or(false)
            {
                process_tray_action(&mut state);
            }

            // Dispatch all pending events (blocking) but check should_exit after each batch
            match event_queue.blocking_dispatch(&mut state) {
                Ok(_) => {
                    // Check immediately after dispatch returns
                    if state.input_state.should_exit {
                        info!("Exit requested after dispatch, breaking event loop");
                        break;
                    }
                    // Adjust keyboard interactivity if toolbar visibility changed.
                    state.sync_toolbar_visibility(&qh);

                    // Advance any delayed history playback (undo/redo with delay).
                    if state
                        .input_state
                        .tick_delayed_history(std::time::Instant::now())
                    {
                        state.toolbar.mark_dirty();
                        state.input_state.needs_redraw = true;
                    }
                    if state.input_state.has_pending_history() {
                        state.input_state.needs_redraw = true;
                    }
                }
                Err(e) => {
                    warn!("Event queue error: {}", e);
                    loop_error = Some(anyhow::anyhow!("Wayland event queue error: {}", e));
                    break;
                }
            }

            if state.input_state.take_pending_frozen_toggle() {
                if !state.frozen_enabled() {
                    warn!(
                        "Frozen mode disabled on this compositor (xdg fallback); ignoring toggle"
                    );
                } else if state.frozen.is_in_progress() {
                    warn!("Frozen capture already in progress; ignoring toggle");
                } else if state.input_state.frozen_active() {
                    state.frozen.unfreeze(&mut state.input_state);
                } else {
                    let use_fallback = !state.frozen.manager_available();
                    if use_fallback {
                        warn!("Frozen mode: screencopy unavailable, using portal fallback");
                    } else {
                        log::info!("Frozen mode: using screencopy fast path");
                    }
                    if let Err(err) = state.frozen.start_capture(
                        &state.shm,
                        &mut state.surface,
                        &qh,
                        use_fallback,
                        &mut state.input_state,
                        &state.tokio_handle,
                    ) {
                        warn!("Frozen capture failed to start: {}", err);
                        state
                            .frozen
                            .cancel(&mut state.surface, &mut state.input_state);
                    }
                }
            }

            if let Some(action) = state.input_state.take_pending_zoom_action() {
                state.handle_zoom_action(action, &qh);
            }

            // Check for completed capture operations
            if state.capture.is_in_progress() {
                if let Some(outcome) = state.capture.manager_mut().try_take_result() {
                    log::info!("Capture completed");

                    // Restore overlay
                    state.show_overlay();
                    state.capture.clear_in_progress();

                    match outcome {
                        CaptureOutcome::Success(result) => {
                            // Build notification message
                            let mut message_parts = Vec::new();

                            if let Some(ref path) = result.saved_path {
                                log::info!("Screenshot saved to: {}", path.display());
                                if let Some(filename) = path.file_name() {
                                    message_parts
                                        .push(format!("Saved as {}", filename.to_string_lossy()));
                                }
                            }

                            if result.copied_to_clipboard {
                                log::info!("Screenshot copied to clipboard");
                                message_parts.push("Copied to clipboard".to_string());
                            }

                            // Send notification
                            let notification_body = if message_parts.is_empty() {
                                "Screenshot captured".to_string()
                            } else {
                                message_parts.join(" • ")
                            };

                            notification::send_notification_async(
                                &state.tokio_handle,
                                "Screenshot Captured".to_string(),
                                notification_body,
                                Some("camera-photo".to_string()),
                            );
                        }
                        CaptureOutcome::Failed(error) => {
                            let friendly_error = friendly_capture_error(&error);

                            log::warn!("Screenshot capture failed: {}", error);

                            notification::send_notification_async(
                                &state.tokio_handle,
                                "Screenshot Failed".to_string(),
                                friendly_error,
                                Some("dialog-error".to_string()),
                            );
                        }
                        CaptureOutcome::Cancelled(reason) => {
                            log::info!("Capture cancelled: {}", reason);
                        }
                    }
                }
            }

            // Render if configured and needs redraw, but only if no frame callback pending
            // This throttles rendering to display refresh rate (when vsync is enabled)
            let can_render = state.surface.is_configured()
                && state.input_state.needs_redraw
                && (!state.surface.frame_callback_pending()
                    || !state.config.performance.enable_vsync);

            if can_render {
                debug!(
                    "Main loop: needs_redraw=true, frame_callback_pending={}, triggering render",
                    state.surface.frame_callback_pending()
                );
                match state.render(&qh) {
                    Ok(keep_rendering) => {
                        // Reset failure counter on successful render
                        consecutive_render_failures = 0;
                        state.input_state.needs_redraw =
                            keep_rendering || state.input_state.has_pending_history();
                        // Only set frame_callback_pending if vsync is enabled
                        if state.config.performance.enable_vsync {
                            state.surface.set_frame_callback_pending(true);
                            debug!(
                                "Main loop: render complete, frame_callback_pending set to true (vsync enabled)"
                            );
                        } else {
                            debug!(
                                "Main loop: render complete, frame_callback_pending unchanged (vsync disabled)"
                            );
                        }
                    }
                    Err(e) => {
                        consecutive_render_failures += 1;
                        warn!(
                            "Rendering error (attempt {}/{}): {}",
                            consecutive_render_failures, MAX_RENDER_FAILURES, e
                        );

                        if consecutive_render_failures >= MAX_RENDER_FAILURES {
                            return Err(anyhow::anyhow!(
                                "Too many consecutive render failures ({}), exiting: {}",
                                consecutive_render_failures,
                                e
                            ));
                        }

                        // Clear redraw flag to avoid infinite error loop
                        state.input_state.needs_redraw = false;
                    }
                }
            } else if state.input_state.needs_redraw && state.surface.frame_callback_pending() {
                debug!("Main loop: Skipping render - frame callback already pending");
            }
        }

        info!("Wayland backend exiting");

        if let Some(options) = state.session_options() {
            if let Some(snapshot) = session::snapshot_from_input(&state.input_state, options) {
                if let Err(err) = session::save_snapshot(&snapshot, options) {
                    warn!("Failed to save session state: {}", err);
                    notification::send_notification_async(
                        &state.tokio_handle,
                        "Failed to Save Session".to_string(),
                        format!("Your drawings may not persist: {}", err),
                        Some("dialog-error".to_string()),
                    );
                }
            }
        }

        // Return error if loop exited due to error, otherwise success
        match loop_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    pub fn init(&mut self) -> Result<()> {
        info!("Initializing Wayland backend");
        Ok(())
    }

    pub fn show(&mut self) -> Result<()> {
        info!("Showing Wayland overlay");
        self.run()
    }

    pub fn hide(&mut self) -> Result<()> {
        info!("Hiding Wayland overlay");
        Ok(())
    }
}
