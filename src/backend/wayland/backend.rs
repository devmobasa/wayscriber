// Coordinates backend startup/shutdown and drives the event loop while delegating
// rendering & protocol state to `WaylandState` and its handler modules.
use anyhow::{Context, Result};
use log::{debug, info, warn};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    },
    shm::Shm,
};
use std::env;
use wayland_client::{Connection, globals::registry_queue_init};

use super::state::WaylandState;
use crate::{
    capture::{CaptureManager, CaptureOutcome},
    config::{Config, ConfigSource},
    input::{BoardMode, InputState},
    legacy, notification, session,
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

/// Wayland backend state
pub struct WaylandBackend {
    initial_mode: Option<String>,
    /// Tokio runtime for async capture operations
    tokio_runtime: tokio::runtime::Runtime,
}

impl WaylandBackend {
    pub fn new(initial_mode: Option<String>) -> Result<Self> {
        let tokio_runtime = tokio::runtime::Runtime::new()
            .context("Failed to create Tokio runtime for capture operations")?;
        Ok(Self {
            initial_mode,
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

        let layer_shell =
            LayerShell::bind(&globals, &qh).context("zwlr_layer_shell_v1 not available")?;
        debug!("Bound layer shell");

        let shm = Shm::bind(&globals, &qh).context("wl_shm not available")?;
        debug!("Bound shared memory");

        let output_state = OutputState::new(&globals, &qh);
        debug!("Initialized output state");

        let seat_state = SeatState::new(&globals, &qh);
        debug!("Initialized seat state");

        let registry_state = RegistryState::new(&globals);

        // Load configuration
        let (config, config_source) = match Config::load() {
            Ok(loaded) => (loaded.config, loaded.source),
            Err(e) => {
                warn!("Failed to load config: {}. Using defaults.", e);
                (Config::default(), ConfigSource::Default)
            }
        };

        if matches!(config_source, ConfigSource::Legacy(_)) && !legacy::warnings_suppressed() {
            warn!(
                "Continuing with settings from legacy hyprmarker config. Run `wayscriber --migrate-config` when convenient."
            );
        }
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

        let config_dir = Config::config_directory_from_source(&config_source)?;

        let display_env = env::var("WAYLAND_DISPLAY").ok();
        let session_options = match session::options_from_config(
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
            config.drawing.default_font_size,
            font_descriptor,
            config.drawing.text_background_enabled,
            config.arrow.length,
            config.arrow.angle_degrees,
            config.ui.show_status_bar,
            config.board.clone(),
            action_map,
            config.session.max_shapes_per_frame,
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

        // Create application state
        let mut state = WaylandState::new(
            registry_state,
            compositor_state,
            layer_shell,
            shm,
            output_state,
            seat_state,
            config,
            input_state,
            capture_manager,
            session_options,
            tokio_handle,
        );

        // Create layer shell surface
        info!("Creating layer shell surface");
        let wl_surface = state.compositor_state.create_surface(&qh);
        let layer_surface = state.layer_shell.create_layer_surface(
            &qh,
            wl_surface,
            Layer::Overlay,
            Some("wayscriber"),
            None, // Default output
        );

        // Configure the layer surface for fullscreen overlay
        layer_surface.set_anchor(Anchor::all());
        // NOTE: Using Exclusive keyboard interactivity for complete input capture
        // If clipboard operations are interrupted during overlay toggle, consider switching
        // to KeyboardInteractivity::OnDemand which cooperates better with other applications
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer_surface.set_size(0, 0); // Use full screen size
        layer_surface.set_exclusive_zone(-1);

        // Commit the surface
        layer_surface.commit();

        state.surface.set_layer_surface(layer_surface);
        info!("Layer shell surface created");

        // Track consecutive render failures for error recovery
        let mut consecutive_render_failures = 0u32;
        const MAX_RENDER_FAILURES: u32 = 10;

        // Main event loop
        let mut loop_error: Option<anyhow::Error> = None;
        loop {
            // Check if we should exit before blocking
            if state.input_state.should_exit {
                info!("Exit requested, breaking event loop");
                break;
            }

            // Dispatch all pending events (blocking) but check should_exit after each batch
            match event_queue.blocking_dispatch(&mut state) {
                Ok(_) => {
                    // Check immediately after dispatch returns
                    if state.input_state.should_exit {
                        info!("Exit requested after dispatch, breaking event loop");
                        break;
                    }
                }
                Err(e) => {
                    warn!("Event queue error: {}", e);
                    loop_error = Some(anyhow::anyhow!("Wayland event queue error: {}", e));
                    break;
                }
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
                    Ok(()) => {
                        // Reset failure counter on successful render
                        consecutive_render_failures = 0;
                        state.input_state.needs_redraw = false;
                        // Only set frame_callback_pending if vsync is enabled
                        if state.config.performance.enable_vsync {
                            state.surface.set_frame_callback_pending(true);
                            debug!(
                                "Main loop: needs_redraw set to false, frame_callback_pending set to true (vsync enabled)"
                            );
                        } else {
                            debug!(
                                "Main loop: needs_redraw set to false, frame_callback_pending unchanged (vsync disabled)"
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
