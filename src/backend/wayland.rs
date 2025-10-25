// Wayland backend using wlr-layer-shell for overlay
use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::collections::HashMap;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_buffer, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};
// Removed: Arc, Mutex - not needed after removing WaylandBackend.inner

use crate::capture::{CaptureDestination, CaptureManager, CaptureOutcome};
use crate::config::{Action, Config, ConfigSource};
use crate::input::{InputState, Key, MouseButton, SystemCommand};
use crate::legacy;

/// Wayland backend state
pub struct WaylandBackend {
    // Removed: inner Arc<Mutex> was unused - WaylandState is created and used directly in run()
    initial_mode: Option<String>,
    /// Tokio runtime for async capture operations
    tokio_runtime: tokio::runtime::Runtime,
}

/// Internal Wayland state
struct WaylandState {
    // Wayland protocol objects
    registry_state: RegistryState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    output_state: OutputState,
    seat_state: SeatState,

    // Surface management
    surfaces: HashMap<u32, OutputSurface>,
    surface_map: HashMap<u32, u32>,
    workspace_origin: (i32, i32),
    workspace_size: (u32, u32),

    // Configuration
    config: Config,

    // Input state
    input_state: InputState,
    current_mouse_x: i32,
    current_mouse_y: i32,

    // Capture manager
    capture_manager: CaptureManager,

    // Capture state tracking
    capture_in_progress: bool,
    overlay_hidden_for_capture: bool,

    // Tokio runtime handle for async operations
    tokio_handle: tokio::runtime::Handle,
}

struct OutputSurface {
    id: u32,
    output: wl_output::WlOutput,
    wl_surface: wl_surface::WlSurface,
    layer_surface: LayerSurface,
    pool: Option<SlotPool>,
    width: u32,
    height: u32,
    configured: bool,
    frame_callback_pending: bool,
    logical_position: (i32, i32),
    scale_factor: i32,
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

    pub fn run(&mut self) -> Result<Option<SystemCommand>> {
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
            config.board.clone(),
            action_map,
        );

        // Apply initial mode from CLI (if provided) or config default (only if board modes enabled)
        if config.board.enabled {
            let initial_mode_str = self
                .initial_mode
                .clone()
                .unwrap_or_else(|| config.board.default_mode.clone());

            if let Ok(mode) = initial_mode_str.parse::<crate::input::BoardMode>() {
                if mode != crate::input::BoardMode::Transparent {
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
        } else if self.initial_mode.is_some() {
            warn!("Board modes disabled in config, ignoring --mode flag");
        }

        // Create capture manager with runtime handle
        let capture_manager = CaptureManager::new(self.tokio_runtime.handle());
        info!("Capture manager initialized");

        // Clone runtime handle for state
        let tokio_handle = self.tokio_runtime.handle().clone();

        // Create application state
        let mut state = WaylandState {
            registry_state,
            compositor_state,
            layer_shell,
            shm,
            output_state,
            seat_state,
            surfaces: HashMap::new(),
            surface_map: HashMap::new(),
            workspace_origin: (0, 0),
            workspace_size: (0, 0),
            config,
            input_state,
            current_mouse_x: 0,
            current_mouse_y: 0,
            capture_manager,
            capture_in_progress: false,
            overlay_hidden_for_capture: false,
            tokio_handle,
        };

        state.initialize_output_surfaces(&qh);

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
            if state.capture_in_progress
                && let Some(outcome) = state.capture_manager.try_take_result()
            {
                log::info!("Capture completed");

                // Restore overlay
                state.show_overlay();
                state.capture_in_progress = false;
                state.input_state.end_capture_guard();

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

                        crate::notification::send_notification_async(
                            &state.tokio_handle,
                            "Screenshot Captured".to_string(),
                            notification_body,
                            Some("camera-photo".to_string()),
                        );
                    }
                    CaptureOutcome::Failed(error) => {
                        log::warn!("Screenshot capture failed: {}", error);

                        crate::notification::send_notification_async(
                            &state.tokio_handle,
                            "Screenshot Failed".to_string(),
                            error,
                            Some("dialog-error".to_string()),
                        );
                    }
                }
            }

            // Render if configured and needs redraw, but only if no frame callback pending
            // This throttles rendering to display refresh rate (when vsync is enabled)
            let can_render = state.input_state.needs_redraw && state.surfaces_ready_for_render();

            if can_render {
                debug!("Main loop: triggering render across {} surfaces", state.surfaces.len());
                match state.render(&qh) {
                    Ok(()) => {
                        // Reset failure counter on successful render
                        consecutive_render_failures = 0;
                        state.input_state.needs_redraw = false;
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
            } else if state.input_state.needs_redraw && !state.surfaces_ready_for_render() {
                debug!("Main loop: Skipping render - waiting for surfaces to configure or frame callbacks");
            }
        }

        info!("Wayland backend exiting");

        // Capture any requested system command before tearing down state
        let system_command = state.input_state.take_pending_system_command();

        // Return error if loop exited due to error, otherwise success
        match loop_error {
            Some(e) => Err(e),
            None => Ok(system_command),
        }
    }
}

impl WaylandState {
    fn initialize_output_surfaces(&mut self, qh: &QueueHandle<Self>) {
        let outputs: Vec<_> = self.output_state.outputs().collect();
        if outputs.is_empty() {
            warn!("No outputs reported by compositor; overlay will activate once an output appears");
        }

        for output in outputs {
            self.create_surface_for_output(output, qh);
        }

        self.recompute_workspace_bounds();
    }

    fn create_surface_for_output(
        &mut self,
        output: wl_output::WlOutput,
        qh: &QueueHandle<Self>,
    ) {
        let id = output.id().protocol_id();
        if self.surfaces.contains_key(&id) {
            return;
        }

        let wl_surface = self.compositor_state.create_surface(qh);
        let layer_surface = self.layer_shell.create_layer_surface(
            qh,
            wl_surface.clone(),
            Layer::Overlay,
            Some("wayscriber"),
            Some(output.clone()),
        );

        layer_surface.set_anchor(Anchor::all());
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer_surface.set_size(0, 0);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.commit();

        let (logical_position, scale_factor) = self.output_state
            .info(&output)
            .map(|info| {
                let pos = info
                    .logical_position
                    .or(Some(info.location))
                    .unwrap_or((0, 0));
                (pos, info.scale_factor.max(1))
            })
            .unwrap_or(((0, 0), 1));

        wl_surface.set_buffer_scale(scale_factor);

        info!(
            "Created overlay surface for output {} at logical position ({}, {})",
            id, logical_position.0, logical_position.1
        );

        self.surface_map
            .insert(wl_surface.id().protocol_id(), id);
        self.surfaces.insert(
            id,
            OutputSurface {
                id,
                output,
                wl_surface,
                layer_surface,
                pool: None,
                width: 0,
                height: 0,
                configured: false,
                frame_callback_pending: false,
                logical_position,
                scale_factor,
            },
        );

        self.input_state.needs_redraw = true;
    }

    fn remove_surface_for_output(&mut self, output: &wl_output::WlOutput) {
        let id = output.id().protocol_id();
        if let Some(surface) = self.surfaces.remove(&id) {
            self.surface_map
                .retain(|_, surface_id| surface_id != &id);
            debug!("Removed overlay surface for output {}", id);
        }
        self.recompute_workspace_bounds();
    }

    fn recompute_workspace_bounds(&mut self) {
        if self.surfaces.is_empty() {
            self.workspace_origin = (0, 0);
            self.workspace_size = (0, 0);
            self.input_state.update_screen_dimensions(0, 0);
            return;
        }

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for surface in self.surfaces.values() {
            let info = self.output_state.info(&surface.output);
            let logical_position = info
                .as_ref()
                .and_then(|i| i.logical_position)
                .or_else(|| info.as_ref().map(|i| i.location))
                .unwrap_or(surface.logical_position);

            let logical_size = info
                .as_ref()
                .and_then(|i| i.logical_size)
                .unwrap_or((surface.width as i32, surface.height as i32));

            min_x = min_x.min(logical_position.0);
            min_y = min_y.min(logical_position.1);
            max_x = max_x.max(logical_position.0 + logical_size.0.max(1));
            max_y = max_y.max(logical_position.1 + logical_size.1.max(1));
        }

        if min_x == i32::MAX || min_y == i32::MAX {
            min_x = 0;
            min_y = 0;
        }
        if max_x <= min_x {
            max_x = min_x + 1;
        }
        if max_y <= min_y {
            max_y = min_y + 1;
        }

        self.workspace_origin = (min_x, min_y);
        self.workspace_size = ((max_x - min_x) as u32, (max_y - min_y) as u32);
        self.input_state
            .update_screen_dimensions(self.workspace_size.0, self.workspace_size.1);
    }

    fn surfaces_ready_for_render(&self) -> bool {
        if self.surfaces.is_empty() {
            return false;
        }

        if self.config.performance.enable_vsync {
            self.surfaces
                .values()
                .all(|surface| surface.configured && !surface.frame_callback_pending)
        } else {
            self.surfaces.values().all(|surface| surface.configured)
        }
    }

    fn surface_offset(&self, surface_id: u32) -> Option<(i32, i32)> {
        let output_id = self.surface_map.get(&surface_id)?;
        self.surfaces.get(output_id).map(|surface| {
            (
                surface.logical_position.0 - self.workspace_origin.0,
                surface.logical_position.1 - self.workspace_origin.1,
            )
        })
    }

    fn surface_mut_by_layer(
        &mut self,
        layer: &LayerSurface,
    ) -> Option<&mut OutputSurface> {
        let target = layer.wl_surface().id().protocol_id();
        let output_id = *self.surface_map.get(&target)?;
        self.surfaces.get_mut(&output_id)
    }

    fn surface_mut_by_wl_surface(
        &mut self,
        surface: &wl_surface::WlSurface,
    ) -> Option<&mut OutputSurface> {
        let target = surface.id().protocol_id();
        let output_id = *self.surface_map.get(&target)?;
        self.surfaces.get_mut(&output_id)
    }

    fn render(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        debug!("=== RENDER START ({} surfaces) ===", self.surfaces.len());
        for surface in self.surfaces.values_mut() {
            if surface.configured && surface.width > 0 && surface.height > 0 {
                self.render_surface(surface, qh)?;
            }
        }
        Ok(())
    }

    fn render_surface(
        &mut self,
        surface: &mut OutputSurface,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let buffer_extent = (surface.width * surface.height * 4) as usize;
        let buffer_count = self.config.performance.buffer_count as usize;

        if surface.pool.is_none() {
            let pool_size = buffer_extent * buffer_count.max(1);
            info!(
                "Creating new SlotPool for output {} ({}x{}, {} bytes, {} buffers)",
                surface.id, surface.width, surface.height, pool_size, buffer_count
            );
            surface.pool = Some(
                SlotPool::new(pool_size, &self.shm).context("Failed to create slot pool")?,
            );
        }

        let pool = surface
            .pool
            .as_mut()
            .context("Output surface pool missing despite initialization")?;

        let (buffer, canvas) = pool
            .create_buffer(
                surface.width as i32,
                surface.height as i32,
                (surface.width * 4) as i32,
                wl_shm::Format::Argb8888,
            )
            .context("Failed to create buffer")?;

        let cairo_surface = unsafe {
            cairo::ImageSurface::create_for_data(
                canvas,
                cairo::Format::ARgb32,
                surface.width as i32,
                surface.height as i32,
                (surface.width * 4) as i32,
            )
            .context("Failed to create Cairo surface")?
        };
        let ctx = cairo::Context::new(&cairo_surface).context("Failed to create Cairo context")?;

        let offset_x = (surface.logical_position.0 - self.workspace_origin.0) as f64;
        let offset_y = (surface.logical_position.1 - self.workspace_origin.1) as f64;
        ctx.translate(-offset_x, -offset_y);

        ctx.set_operator(cairo::Operator::Source);
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        ctx.paint()?;
        ctx.set_operator(cairo::Operator::Over);

        crate::draw::render_canvas(&ctx, &self.input_state);

        if let DrawingState::TextInput { x, y, ref buffer } = self.input_state.state {
            let preview_text = if buffer.is_empty() { "|" } else { buffer };
            crate::draw::render_text(
                &ctx,
                *x,
                *y,
                &preview_text,
                self.input_state.current_color,
                self.input_state.current_font_size,
                &self.input_state.font_descriptor,
                self.input_state.text_background_enabled,
            );
        }

        if self.config.ui.show_status_bar {
            crate::ui::render_status_bar(
                &ctx,
                &self.input_state,
                self.config.ui.status_bar_position,
                &self.config.ui.status_bar_style,
                self.workspace_size.0,
                self.workspace_size.1,
            );
        }

        if self.input_state.show_help {
            crate::ui::render_help_overlay(
                &ctx,
                &self.config.ui.help_overlay_style,
                self.workspace_size.0,
                self.workspace_size.1,
            );
        }

        cairo_surface.flush();
        drop(ctx);
        drop(cairo_surface);

        surface
            .wl_surface
            .attach(Some(buffer.wl_buffer()), 0, 0);
        surface.wl_surface.damage_buffer(0, 0, surface.width as i32, surface.height as i32);

        if self.config.performance.enable_vsync {
            surface
                .wl_surface
                .frame(qh, surface.wl_surface.clone());
            surface.frame_callback_pending = true;
        }

        surface.wl_surface.commit();
        Ok(())
    }
}
    /// Temporarily hide the overlay for screenshot capture.
    ///
    /// This unmaps the layer surface so the compositor doesn't render it.
    /// The overlay state (drawings, mode, etc.) is preserved.
    fn hide_overlay(&mut self) {
        if self.overlay_hidden_for_capture {
            log::warn!("Overlay already hidden for capture");
            return;
        }

        log::info!("Hiding overlay for screenshot capture");

        for surface in self.surfaces.values() {
            surface.layer_surface.set_size(0, 0);
            surface.wl_surface.commit();
        }

        self.overlay_hidden_for_capture = true;

        // Give compositor time to process the unmap
        // (the async capture will start shortly after)
    }

    /// Restore the overlay after screenshot capture completes.
    ///
    /// Re-maps the layer surface to its original size and forces a redraw.
    fn show_overlay(&mut self) {
        if !self.overlay_hidden_for_capture {
            log::warn!("Overlay was not hidden, nothing to restore");
            return;
        }

        log::info!("Restoring overlay after screenshot capture");

        for surface in self.surfaces.values() {
            surface
                .layer_surface
                .set_size(surface.width as i32, surface.height as i32);
            surface.wl_surface.commit();
        }

        self.overlay_hidden_for_capture = false;

        // Force a redraw to show the overlay again
        self.input_state.needs_redraw = true;
    }

    /// Handles capture actions by delegating to the CaptureManager.
    fn handle_capture_action(&mut self, action: Action) {
        use crate::capture::file::{FileSaveConfig, expand_tilde};
        use crate::capture::types::CaptureType;

        if !self.config.capture.enabled {
            log::warn!("Capture action triggered but capture is disabled in config");
            return;
        }

        let default_destination = if self.config.capture.copy_to_clipboard {
            CaptureDestination::ClipboardAndFile
        } else {
            CaptureDestination::FileOnly
        };

        let (capture_type, destination) = match action {
            Action::CaptureFullScreen => (CaptureType::FullScreen, default_destination),
            Action::CaptureActiveWindow => (CaptureType::ActiveWindow, default_destination),
            Action::CaptureSelection => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                default_destination,
            ),
            Action::CaptureClipboardFull => {
                (CaptureType::FullScreen, CaptureDestination::ClipboardOnly)
            }
            Action::CaptureFileFull => (CaptureType::FullScreen, CaptureDestination::FileOnly),
            Action::CaptureClipboardSelection => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                CaptureDestination::ClipboardOnly,
            ),
            Action::CaptureFileSelection => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                CaptureDestination::FileOnly,
            ),
            Action::CaptureClipboardRegion => {
                log::info!("Region clipboard capture requested");
                // TODO: implement persistent region geometry; fall back to selection for now
                (
                    CaptureType::Selection {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    },
                    CaptureDestination::ClipboardOnly,
                )
            }
            Action::CaptureFileRegion => {
                log::info!("Region file capture requested");
                // TODO: implement persistent region geometry; fall back to selection for now
                (
                    CaptureType::Selection {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    },
                    CaptureDestination::FileOnly,
                )
            }
            _ => {
                log::error!(
                    "Non-capture action passed to handle_capture_action: {:?}",
                    action
                );
                return;
            }
        };

        // Build file save config from user config when needed
        let save_config = if matches!(destination, CaptureDestination::ClipboardOnly) {
            None
        } else {
            Some(FileSaveConfig {
                save_directory: expand_tilde(&self.config.capture.save_directory),
                filename_template: self.config.capture.filename_template.clone(),
                format: self.config.capture.format.clone(),
            })
        };

        // Hide overlay before capture to prevent capturing the overlay itself
        self.hide_overlay();
        self.capture_in_progress = true;
        self.input_state.begin_capture_guard();

        // Request capture
        log::info!("Requesting {:?} capture", capture_type);
        if let Err(e) = self
            .capture_manager
            .request_capture(capture_type, destination, save_config)
        {
            log::error!("Failed to request capture: {}", e);

            // Restore overlay on error
            self.show_overlay();
            self.capture_in_progress = false;
            self.input_state.end_capture_guard();
        }
    }
}

impl WaylandBackend {
    pub fn init(&mut self) -> Result<()> {
        info!("Initializing Wayland backend");
        Ok(())
    }

    pub fn show(&mut self) -> Result<Option<SystemCommand>> {
        info!("Showing Wayland overlay");
        self.run()
    }

    pub fn hide(&mut self) -> Result<()> {
        info!("Hiding Wayland overlay");
        Ok(())
    }
}

// Implement required trait delegates
delegate_compositor!(WaylandState);
delegate_output!(WaylandState);
delegate_shm!(WaylandState);
delegate_layer!(WaylandState);
delegate_seat!(WaylandState);
delegate_keyboard!(WaylandState);
delegate_pointer!(WaylandState);
delegate_registry!(WaylandState);

// Implement CompositorHandler
impl CompositorHandler for WaylandState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        debug!("Scale factor changed");
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        debug!("Transform changed");
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // Frame callback - compositor is ready for next frame
        debug!(
            "Frame callback received (time: {}ms), clearing frame_callback_pending",
            _time
        );
        self.frame_callback_pending = false;

        // If we're actively drawing, request another render
        // (input events may have set needs_redraw while we were waiting)
        if self.input_state.needs_redraw {
            debug!(
                "Frame callback: needs_redraw is still true, will render on next loop iteration"
            );
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        debug!("Surface entered output");
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        debug!("Surface left output");
    }
}

// Implement OutputHandler
impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        debug!("New output detected");
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        debug!("Output updated");
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        debug!("Output destroyed");
    }
}

// Implement ShmHandler
impl ShmHandler for WaylandState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

// Implement LayerShellHandler
impl LayerShellHandler for WaylandState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        info!("Layer surface closed by compositor");
        self.input_state.should_exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        info!(
            "Layer surface configured: {}x{}",
            configure.new_size.0, configure.new_size.1
        );

        // Update dimensions
        if configure.new_size.0 > 0 && configure.new_size.1 > 0 {
            let size_changed =
                self.width != configure.new_size.0 || self.height != configure.new_size.1;

            self.width = configure.new_size.0;
            self.height = configure.new_size.1;

            // Recreate pool if dimensions changed
            if size_changed && self.pool.is_some() {
                info!("Surface size changed - recreating SlotPool");
                self.pool = None;
            }

            // Update input state with actual screen dimensions
            self.input_state
                .update_screen_dimensions(self.width, self.height);
        }

        // Mark as configured and request first draw
        self.configured = true;
        self.input_state.needs_redraw = true;
    }
}

// Implement SeatHandler
impl SeatHandler for WaylandState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
        debug!("New seat available");
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            info!("Keyboard capability available");
            if self.seat_state.get_keyboard(qh, &seat, None).is_ok() {
                debug!("Keyboard initialized");
            }
        }

        if capability == Capability::Pointer {
            info!("Pointer capability available");
            if self.seat_state.get_pointer(qh, &seat).is_ok() {
                debug!("Pointer initialized");
            }
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            info!("Keyboard capability removed");
        }
        if capability == Capability::Pointer {
            info!("Pointer capability removed");
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
        debug!("Seat removed");
    }
}

// Implement KeyboardHandler
impl KeyboardHandler for WaylandState {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
        debug!("Keyboard focus entered");
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        debug!("Keyboard focus left");
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        let key = keysym_to_key(event.keysym);
        debug!("Key pressed: {:?}", key);
        self.input_state.on_key_press(key);
        self.input_state.needs_redraw = true;

        // Check for pending capture actions
        if let Some(action) = self.input_state.take_pending_capture_action() {
            self.handle_capture_action(action);
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        let key = keysym_to_key(event.keysym);
        debug!("Key released: {:?}", key);
        self.input_state.on_key_release(key);
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _layout: RawModifiers,
        _group: u32,
    ) {
        debug!(
            "Modifiers: ctrl={} alt={} shift={}",
            modifiers.ctrl, modifiers.alt, modifiers.shift
        );
    }

    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // Handle key repeat - treat like a regular key press
        let key = keysym_to_key(event.keysym);
        debug!("Key repeated: {:?}", key);
        self.input_state.on_key_press(key);
        self.input_state.needs_redraw = true;
    }
}

// Implement PointerHandler
impl PointerHandler for WaylandState {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use smithay_client_toolkit::seat::pointer::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT};

        for event in events {
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    debug!(
                        "Pointer entered at ({}, {})",
                        event.position.0, event.position.1
                    );
                    self.current_mouse_x = event.position.0 as i32;
                    self.current_mouse_y = event.position.1 as i32;
                }
                PointerEventKind::Leave { .. } => {
                    debug!("Pointer left surface");
                }
                PointerEventKind::Motion { .. } => {
                    self.current_mouse_x = event.position.0 as i32;
                    self.current_mouse_y = event.position.1 as i32;
                    self.input_state
                        .on_mouse_motion(self.current_mouse_x, self.current_mouse_y);
                    // Note: needs_redraw is set inside on_mouse_motion if actively drawing
                    // Don't set it here unconditionally to avoid rendering on every mouse move
                }
                PointerEventKind::Press { button, .. } => {
                    debug!(
                        "Button {} pressed at ({}, {})",
                        button, event.position.0, event.position.1
                    );

                    let mb = match button {
                        BTN_LEFT => MouseButton::Left,
                        BTN_MIDDLE => MouseButton::Middle,
                        BTN_RIGHT => MouseButton::Right,
                        _ => continue,
                    };

                    self.input_state.on_mouse_press(
                        mb,
                        event.position.0 as i32,
                        event.position.1 as i32,
                    );
                    self.input_state.needs_redraw = true;
                }
                PointerEventKind::Release { button, .. } => {
                    debug!("Button {} released", button);

                    let mb = match button {
                        BTN_LEFT => MouseButton::Left,
                        BTN_MIDDLE => MouseButton::Middle,
                        BTN_RIGHT => MouseButton::Right,
                        _ => continue,
                    };

                    self.input_state.on_mouse_release(
                        mb,
                        event.position.0 as i32,
                        event.position.1 as i32,
                    );
                    self.input_state.needs_redraw = true;
                }
                PointerEventKind::Axis { vertical, .. } => {
                    // Use discrete steps if available, otherwise fall back to absolute with threshold
                    let scroll_direction = if vertical.discrete != 0 {
                        vertical.discrete
                    } else if vertical.absolute.abs() > 0.1 {
                        // Threshold to ignore tiny movements
                        if vertical.absolute > 0.0 { 1 } else { -1 }
                    } else {
                        0
                    };

                    if scroll_direction != 0 {
                        let shift_mode = self.input_state.modifiers.shift;
                        self.input_state.on_scroll(scroll_direction);

                        if shift_mode {
                            debug!(
                                "Font size {}: {:.1}px",
                                if scroll_direction > 0 { "decreased" } else { "increased" },
                                self.input_state.current_font_size
                            );
                        } else {
                            debug!(
                                "Thickness {}: {:.0}px",
                                if scroll_direction > 0 { "decreased" } else { "increased" },
                                self.input_state.current_thickness
                            );
                        }
                    }
                }
            }
        }
    }
}

// Implement ProvidesRegistryState
impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

// Implement Dispatch for wl_buffer (required for buffer lifecycle)
impl Dispatch<wl_buffer::WlBuffer, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            debug!("Buffer released by compositor");
        }
    }
}

// Convert Wayland keysym to our Key enum
fn keysym_to_key(keysym: Keysym) -> Key {
    match keysym {
        Keysym::Escape => Key::Escape,
        Keysym::Return => Key::Return,
        Keysym::BackSpace => Key::Backspace,
        Keysym::Tab => Key::Tab,
        Keysym::space => Key::Space,
        Keysym::Shift_L | Keysym::Shift_R => Key::Shift,
        Keysym::Control_L | Keysym::Control_R => Key::Ctrl,
        Keysym::Alt_L | Keysym::Alt_R => Key::Alt,
        Keysym::plus | Keysym::equal => Key::Plus,
        Keysym::minus | Keysym::underscore => Key::Minus,
        Keysym::t => Key::Char('t'),
        Keysym::T => Key::Char('T'),
        Keysym::e => Key::Char('e'),
        Keysym::E => Key::Char('E'),
        Keysym::r => Key::Char('r'),
        Keysym::R => Key::Char('R'),
        Keysym::g => Key::Char('g'),
        Keysym::G => Key::Char('G'),
        Keysym::b => Key::Char('b'),
        Keysym::B => Key::Char('B'),
        Keysym::y => Key::Char('y'),
        Keysym::Y => Key::Char('Y'),
        Keysym::o => Key::Char('o'),
        Keysym::O => Key::Char('O'),
        Keysym::p => Key::Char('p'),
        Keysym::P => Key::Char('P'),
        Keysym::w => Key::Char('w'),
        Keysym::W => Key::Char('W'),
        Keysym::k => Key::Char('k'),
        Keysym::K => Key::Char('K'),
        Keysym::z => Key::Char('z'),
        Keysym::Z => Key::Char('Z'),
        Keysym::F10 => Key::F10,
        Keysym::F11 => Key::F11,
        _ => {
            // For other printable characters, try to map them
            // Use the raw value to determine if it's ASCII printable
            let raw = keysym.raw();
            if (0x20..=0x7E).contains(&raw) {
                Key::Char(raw as u8 as char)
            } else {
                Key::Unknown
            }
        }
    }
}
