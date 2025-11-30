// Holds the live Wayland protocol state shared by the backend loop and the handler
// submodules; provides rendering, capture routing, and overlay helpers used across them.
use crate::draw::Color;
use anyhow::{Context, Result};
use log::{debug, warn};
use smithay_client_toolkit::{
    activation::{ActivationHandler, ActivationState, RequestData},
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::{
        SeatState,
        pointer::{PointerData, ThemedPointer},
    },
    shell::{
        wlr_layer::{KeyboardInteractivity, LayerShell},
        xdg::XdgShell,
    },
    shm::Shm,
};
use std::collections::HashSet;
use std::time::Instant;
#[cfg(tablet)]
use wayland_client::protocol::wl_surface;
use wayland_client::{
    QueueHandle,
    protocol::{wl_output, wl_seat, wl_shm},
};
#[cfg(tablet)]
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2::ZwpTabletManagerV2, zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2,
    zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2, zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2,
    zwp_tablet_pad_v2::ZwpTabletPadV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
    zwp_tablet_tool_v2::ZwpTabletToolV2, zwp_tablet_v2::ZwpTabletV2,
};

#[cfg(tablet)]
use crate::input::tablet::TabletSettings;
use crate::{
    capture::{
        CaptureDestination, CaptureManager,
        file::{FileSaveConfig, expand_tilde},
        types::CaptureType,
    },
    config::{Action, ColorSpec, Config},
    input::{DrawingState, InputState},
    session::SessionOptions,
    ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot},
    util::Rect,
};

use self::data::StateData;
use super::{
    capture::CaptureState, frozen::FrozenState, session::SessionState, surface::SurfaceState,
    toolbar::ToolbarSurfaceManager,
};

mod data;

/// Internal Wayland state shared across modules.
pub(super) struct WaylandState {
    // Wayland protocol objects
    pub(super) registry_state: RegistryState,
    pub(super) compositor_state: CompositorState,
    pub(super) layer_shell: Option<LayerShell>,
    pub(super) xdg_shell: Option<XdgShell>,
    pub(super) activation: Option<ActivationState>,
    pub(super) shm: Shm,
    pub(super) output_state: OutputState,
    pub(super) seat_state: SeatState,

    // Surface and buffer management
    pub(super) surface: SurfaceState,
    pub(super) toolbar: ToolbarSurfaceManager,
    data: StateData,

    // Configuration
    pub(super) config: Config,

    // Input state
    pub(super) input_state: InputState,

    // Capture manager
    pub(super) capture: CaptureState,
    pub(super) frozen: FrozenState,

    // Pointer cursor
    pub(super) themed_pointer: Option<ThemedPointer<PointerData>>,

    // Tablet / stylus (feature-gated)
    #[cfg(tablet)]
    pub(super) tablet_manager: Option<ZwpTabletManagerV2>,
    #[cfg(tablet)]
    pub(super) tablet_seats: Vec<ZwpTabletSeatV2>,
    #[cfg(tablet)]
    pub(super) tablets: Vec<ZwpTabletV2>,
    #[cfg(tablet)]
    pub(super) tablet_tools: Vec<ZwpTabletToolV2>,
    #[cfg(tablet)]
    pub(super) tablet_pads: Vec<ZwpTabletPadV2>,
    #[cfg(tablet)]
    pub(super) tablet_pad_groups: Vec<ZwpTabletPadGroupV2>,
    #[cfg(tablet)]
    pub(super) tablet_pad_rings: Vec<ZwpTabletPadRingV2>,
    #[cfg(tablet)]
    pub(super) tablet_pad_strips: Vec<ZwpTabletPadStripV2>,
    #[cfg(tablet)]
    pub(super) tablet_settings: TabletSettings,
    #[cfg(tablet)]
    pub(super) tablet_found_logged: bool,
    #[cfg(tablet)]
    pub(super) stylus_tip_down: bool,
    #[cfg(tablet)]
    pub(super) stylus_on_overlay: bool,
    #[cfg(tablet)]
    pub(super) stylus_on_toolbar: bool,
    #[cfg(tablet)]
    pub(super) stylus_base_thickness: Option<f64>,
    #[cfg(tablet)]
    pub(super) stylus_pressure_thickness: Option<f64>,
    #[cfg(tablet)]
    pub(super) stylus_surface: Option<wl_surface::WlSurface>,
    #[cfg(tablet)]
    pub(super) stylus_last_pos: Option<(f64, f64)>,
    #[cfg(tablet)]
    pub(super) stylus_peak_thickness: Option<f64>,

    // Session persistence
    pub(super) session: SessionState,

    // Tokio runtime handle for async operations
    pub(super) tokio_handle: tokio::runtime::Handle,
}

impl WaylandState {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        registry_state: RegistryState,
        compositor_state: CompositorState,
        layer_shell: Option<LayerShell>,
        xdg_shell: Option<XdgShell>,
        activation: Option<ActivationState>,
        shm: Shm,
        output_state: OutputState,
        seat_state: SeatState,
        config: Config,
        input_state: InputState,
        capture_manager: CaptureManager,
        session_options: Option<SessionOptions>,
        tokio_handle: tokio::runtime::Handle,
        frozen_enabled: bool,
        preferred_output_identity: Option<String>,
        xdg_fullscreen: bool,
        pending_freeze_on_start: bool,
        screencopy_manager: Option<wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1>,
        #[cfg(tablet)] tablet_manager: Option<ZwpTabletManagerV2>,
    ) -> Self {
        #[cfg(tablet)]
        let tablet_settings = {
            TabletSettings {
                enabled: config.tablet.enabled,
                pressure_enabled: config.tablet.pressure_enabled,
                min_thickness: config.tablet.min_thickness,
                max_thickness: config.tablet.max_thickness,
            }
        };

        let mut data = StateData::new();
        data.frozen_enabled = frozen_enabled;
        data.pending_freeze_on_start = pending_freeze_on_start;
        data.preferred_output_identity = preferred_output_identity;
        data.xdg_fullscreen = xdg_fullscreen;

        Self {
            registry_state,
            compositor_state,
            layer_shell,
            xdg_shell,
            activation,
            shm,
            output_state,
            seat_state,
            surface: SurfaceState::new(),
            toolbar: ToolbarSurfaceManager::new(),
            data,
            config,
            input_state,
            capture: CaptureState::new(capture_manager),
            frozen: FrozenState::new(screencopy_manager),
            themed_pointer: None,
            #[cfg(tablet)]
            tablet_manager,
            #[cfg(tablet)]
            tablet_seats: Vec::new(),
            #[cfg(tablet)]
            tablets: Vec::new(),
            #[cfg(tablet)]
            tablet_tools: Vec::new(),
            #[cfg(tablet)]
            tablet_pads: Vec::new(),
            #[cfg(tablet)]
            tablet_pad_groups: Vec::new(),
            #[cfg(tablet)]
            tablet_pad_rings: Vec::new(),
            #[cfg(tablet)]
            tablet_pad_strips: Vec::new(),
            #[cfg(tablet)]
            tablet_settings,
            #[cfg(tablet)]
            tablet_found_logged: false,
            #[cfg(tablet)]
            stylus_tip_down: false,
            #[cfg(tablet)]
            stylus_on_overlay: false,
            #[cfg(tablet)]
            stylus_on_toolbar: false,
            #[cfg(tablet)]
            stylus_base_thickness: None,
            #[cfg(tablet)]
            stylus_pressure_thickness: None,
            #[cfg(tablet)]
            stylus_surface: None,
            #[cfg(tablet)]
            stylus_last_pos: None,
            #[cfg(tablet)]
            stylus_peak_thickness: None,
            session: SessionState::new(session_options),
            tokio_handle,
        }
    }

    pub(super) fn pointer_over_toolbar(&self) -> bool {
        self.data.pointer_over_toolbar
    }

    pub(super) fn set_pointer_over_toolbar(&mut self, value: bool) {
        self.data.pointer_over_toolbar = value;
    }

    pub(super) fn toolbar_dragging(&self) -> bool {
        self.data.toolbar_dragging
    }

    pub(super) fn set_toolbar_dragging(&mut self, value: bool) {
        self.data.toolbar_dragging = value;
    }

    pub(super) fn toolbar_needs_recreate(&self) -> bool {
        self.data.toolbar_needs_recreate
    }

    pub(super) fn set_toolbar_needs_recreate(&mut self, value: bool) {
        self.data.toolbar_needs_recreate = value;
    }

    pub(super) fn current_mouse(&self) -> (i32, i32) {
        (self.data.current_mouse_x, self.data.current_mouse_y)
    }

    pub(super) fn set_current_mouse(&mut self, x: i32, y: i32) {
        self.data.current_mouse_x = x;
        self.data.current_mouse_y = y;
    }

    #[allow(dead_code)]
    pub(super) fn has_keyboard_focus(&self) -> bool {
        self.data.has_keyboard_focus
    }

    pub(super) fn set_keyboard_focus(&mut self, value: bool) {
        self.data.has_keyboard_focus = value;
    }

    #[allow(dead_code)]
    pub(super) fn has_pointer_focus(&self) -> bool {
        self.data.has_pointer_focus
    }

    pub(super) fn set_pointer_focus(&mut self, value: bool) {
        self.data.has_pointer_focus = value;
    }

    pub(super) fn current_seat(&self) -> Option<wl_seat::WlSeat> {
        self.data.current_seat.clone()
    }

    pub(super) fn set_current_seat(&mut self, seat: Option<wl_seat::WlSeat>) {
        self.data.current_seat = seat;
    }

    pub(super) fn last_activation_serial(&self) -> Option<u32> {
        self.data.last_activation_serial
    }

    pub(super) fn set_last_activation_serial(&mut self, serial: Option<u32>) {
        self.data.last_activation_serial = serial;
    }

    pub(super) fn current_keyboard_interactivity(&self) -> Option<KeyboardInteractivity> {
        self.data.current_keyboard_interactivity
    }

    pub(super) fn set_current_keyboard_interactivity(
        &mut self,
        interactivity: Option<KeyboardInteractivity>,
    ) {
        self.data.current_keyboard_interactivity = interactivity;
    }

    pub(super) fn frozen_enabled(&self) -> bool {
        self.data.frozen_enabled
    }

    #[allow(dead_code)]
    pub(super) fn set_frozen_enabled(&mut self, value: bool) {
        self.data.frozen_enabled = value;
    }

    pub(super) fn pending_freeze_on_start(&self) -> bool {
        self.data.pending_freeze_on_start
    }

    pub(super) fn set_pending_freeze_on_start(&mut self, value: bool) {
        self.data.pending_freeze_on_start = value;
    }

    pub(super) fn pending_activation_token(&self) -> Option<String> {
        self.data.pending_activation_token.clone()
    }

    pub(super) fn set_pending_activation_token(&mut self, token: Option<String>) {
        self.data.pending_activation_token = token;
    }

    pub(super) fn preferred_output_identity(&self) -> Option<&str> {
        self.data.preferred_output_identity.as_deref()
    }

    #[allow(dead_code)]
    pub(super) fn set_preferred_output_identity(&mut self, value: Option<String>) {
        self.data.preferred_output_identity = value;
    }

    pub(super) fn xdg_fullscreen(&self) -> bool {
        self.data.xdg_fullscreen
    }

    #[allow(dead_code)]
    pub(super) fn set_xdg_fullscreen(&mut self, value: bool) {
        self.data.xdg_fullscreen = value;
    }

    pub(super) fn session_options(&self) -> Option<&SessionOptions> {
        self.session.options()
    }

    pub(super) fn session_options_mut(&mut self) -> Option<&mut SessionOptions> {
        self.session.options_mut()
    }

    pub(super) fn preferred_fullscreen_output(&self) -> Option<wl_output::WlOutput> {
        if let Some(preferred) = self.preferred_output_identity() {
            if let Some(output) = self.output_state.outputs().find(|output| {
                self.output_identity_for(output)
                    .map(|id| id.eq_ignore_ascii_case(preferred))
                    .unwrap_or(false)
            }) {
                return Some(output);
            }
        }

        self.surface
            .current_output()
            .or_else(|| self.output_state.outputs().next())
    }

    /// Determines the desired keyboard interactivity for the main layer surface.
    pub(super) fn desired_keyboard_interactivity(&self) -> KeyboardInteractivity {
        if self.toolbar.is_visible() {
            KeyboardInteractivity::OnDemand
        } else {
            KeyboardInteractivity::Exclusive
        }
    }

    /// Applies keyboard interactivity based on toolbar visibility.
    pub(super) fn refresh_keyboard_interactivity(&mut self) {
        let desired = self.desired_keyboard_interactivity();
        let current = self.current_keyboard_interactivity();

        let updated = if let Some(layer) = self.surface.layer_surface_mut() {
            if current != Some(desired) {
                layer.set_keyboard_interactivity(desired);
                true
            } else {
                false
            }
        } else {
            self.set_current_keyboard_interactivity(None);
            return;
        };

        if updated {
            self.set_current_keyboard_interactivity(Some(desired));
        }
    }

    /// Syncs toolbar visibility from the input state, ensures surfaces exist, and adjusts keyboard interactivity.
    pub(super) fn sync_toolbar_visibility(&mut self, qh: &QueueHandle<Self>) {
        // Sync individual toolbar visibility
        let top_visible = self.input_state.toolbar_top_visible();
        let side_visible = self.input_state.toolbar_side_visible();

        if top_visible != self.toolbar.is_top_visible() {
            self.toolbar.set_top_visible(top_visible);
            self.input_state.needs_redraw = true;
        }

        if side_visible != self.toolbar.is_side_visible() {
            self.toolbar.set_side_visible(side_visible);
            self.input_state.needs_redraw = true;
        }

        let any_visible = self.toolbar.is_visible();
        if !any_visible {
            self.set_pointer_over_toolbar(false);
        }

        if any_visible {
            if self.toolbar_needs_recreate() {
                self.toolbar.destroy_all();
                self.set_toolbar_needs_recreate(false);
            }
            if let Some(layer_shell) = self.layer_shell.as_ref() {
                let scale = self.surface.scale();
                let snapshot = self.toolbar_snapshot();
                self.toolbar.ensure_created(
                    qh,
                    &self.compositor_state,
                    layer_shell,
                    scale,
                    &snapshot,
                );
            }
        }

        self.refresh_keyboard_interactivity();
    }

    pub(super) fn render_toolbars(&mut self, snapshot: &ToolbarSnapshot) {
        if !self.toolbar.is_visible() {
            return;
        }

        // No hover tracking yet; pass None. Can be updated when we record pointer positions per surface.
        self.toolbar.render(&self.shm, snapshot, None);
    }

    pub(super) fn request_xdg_activation(&mut self, qh: &QueueHandle<Self>) {
        if !self.surface.is_xdg_window() {
            return;
        }

        let Some(activation) = self.activation.as_ref() else {
            return;
        };

        let Some(wl_surface) = self.surface.wl_surface().cloned() else {
            return;
        };

        if let Some(seat_serial) = self
            .current_seat()
            .as_ref()
            .cloned()
            .zip(self.last_activation_serial())
        {
            activation.request_token::<Self>(
                qh,
                RequestData {
                    app_id: Some("com.devmobasa.wayscriber".to_string()),
                    seat_and_serial: Some(seat_serial),
                    surface: Some(wl_surface),
                },
            );
        } else {
            // Defer until we have a keyboard enter serial.
            self.set_pending_activation_token(Some(String::new())); // marker
        }
    }

    fn activate_xdg_window_if_possible(&mut self) {
        if !self.surface.is_xdg_window() {
            return;
        }

        let Some(token) = self.pending_activation_token() else {
            return;
        };

        let Some(activation) = self.activation.as_ref() else {
            return;
        };

        let Some(wl_surface) = self.surface.wl_surface().cloned() else {
            return;
        };

        activation.activate::<WaylandState>(&wl_surface, token);
        self.set_pending_activation_token(None);
    }

    pub(super) fn maybe_retry_activation(&mut self, qh: &QueueHandle<Self>) {
        if self.pending_activation_token().is_some() && self.last_activation_serial().is_some() {
            // Drop the placeholder and re-request with the new serial.
            self.set_pending_activation_token(None);
            self.request_xdg_activation(qh);
        }
    }

    pub(super) fn output_identity_for(&self, output: &wl_output::WlOutput) -> Option<String> {
        let info = self.output_state.info(output)?;

        let mut components: Vec<String> = Vec::new();

        if let Some(name) = info.name.filter(|s| !s.is_empty()) {
            components.push(name);
        }

        if !info.make.is_empty() {
            components.push(info.make);
        }

        if !info.model.is_empty() {
            components.push(info.model);
        }

        if components.is_empty() {
            components.push(format!("id{}", info.id));
        }

        Some(components.join("-"))
    }

    pub(super) fn render(&mut self, qh: &QueueHandle<Self>) -> Result<bool> {
        debug!("=== RENDER START ===");
        if self.capture.is_overlay_hidden() {
            debug!("Skipping render while overlay is hidden for capture");
            self.input_state.needs_redraw = false;
            return Ok(false);
        }

        // Create pool if needed
        let buffer_count = self.config.performance.buffer_count as usize;
        let scale = self.surface.scale().max(1);
        let width = self.surface.width();
        let height = self.surface.height();
        let phys_width = width.saturating_mul(scale as u32);
        let phys_height = height.saturating_mul(scale as u32);
        let now = Instant::now();
        let highlight_active = self.input_state.advance_click_highlights(now);
        let mut eraser_pattern: Option<cairo::SurfacePattern> = None;
        let mut eraser_bg_color: Option<Color> = None;

        // Get a buffer from the pool
        let (buffer, canvas) = {
            let pool = self.surface.ensure_pool(&self.shm, buffer_count)?;
            debug!("Requesting buffer from pool");
            let result = pool
                .create_buffer(
                    phys_width as i32,
                    phys_height as i32,
                    (phys_width * 4) as i32,
                    wl_shm::Format::Argb8888,
                )
                .context("Failed to create buffer")?;
            debug!("Buffer acquired from pool");
            result
        };

        // SAFETY: This unsafe block creates a Cairo surface from raw memory buffer.
        // Safety invariants that must be maintained:
        // 1. `canvas` is a valid mutable slice from SlotPool with exactly (width * height * 4) bytes
        // 2. The buffer format ARgb32 matches the allocation (4 bytes per pixel: alpha, red, green, blue)
        // 3. The stride (width * 4) correctly represents the number of bytes per row
        // 4. `cairo_surface` and `ctx` are explicitly dropped before the buffer is committed to Wayland,
        //    ensuring Cairo doesn't access memory after ownership transfers
        // 5. No other references to this memory exist during Cairo's usage
        // 6. The buffer remains valid throughout Cairo's usage (enforced by Rust's borrow checker)
        let cairo_surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                phys_width as i32,
                phys_height as i32,
                (phys_width * 4) as i32,
            )
            .context("Failed to create Cairo surface")?
        };

        // Render using Cairo
        let ctx = cairo::Context::new(&cairo_surface).context("Failed to create Cairo context")?;

        // Clear with fully transparent background
        debug!("Clearing background");
        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().context("Failed to clear background")?;
        ctx.set_operator(cairo::Operator::Over);

        if let Some(image) = self.frozen.image() {
            // SAFETY: we create a Cairo surface borrowing our owned buffer; it is dropped
            // before commit, and we hold the buffer alive via `image.data`.
            let surface = unsafe {
                cairo::ImageSurface::create_for_data_unsafe(
                    image.data.as_ptr() as *mut u8,
                    cairo::Format::ARgb32,
                    image.width as i32,
                    image.height as i32,
                    image.stride,
                )
            }
            .context("Failed to create frozen image surface")?;

            let scale_x = if image.width > 0 {
                phys_width as f64 / image.width as f64
            } else {
                1.0
            };
            let scale_y = if image.height > 0 {
                phys_height as f64 / image.height as f64
            } else {
                1.0
            };
            let _ = ctx.save();
            if (scale_x - 1.0).abs() > f64::EPSILON || (scale_y - 1.0).abs() > f64::EPSILON {
                ctx.scale(scale_x, scale_y);
            }

            if let Err(err) = ctx.set_source_surface(&surface, 0.0, 0.0) {
                warn!("Failed to set frozen background surface: {}", err);
            } else if let Err(err) = ctx.paint() {
                warn!("Failed to paint frozen background: {}", err);
            }
            let _ = ctx.restore();

            let pattern = cairo::SurfacePattern::create(&surface);
            pattern.set_extend(cairo::Extend::Pad);
            let mut matrix = cairo::Matrix::identity();
            let scale_x_inv = 1.0 / (scale as f64 * scale_x.max(f64::MIN_POSITIVE));
            let scale_y_inv = 1.0 / (scale as f64 * scale_y.max(f64::MIN_POSITIVE));
            matrix.scale(scale_x_inv, scale_y_inv);
            pattern.set_matrix(matrix);
            eraser_pattern = Some(pattern);
        } else {
            // Render board background if in board mode (whiteboard/blackboard)
            crate::draw::render_board_background(
                &ctx,
                self.input_state.board_mode(),
                &self.input_state.board_config,
            );
            eraser_bg_color = self
                .input_state
                .board_mode()
                .background_color(&self.input_state.board_config);
        }

        // Scale subsequent drawing to logical coordinates
        let _ = ctx.save();
        if scale > 1 {
            ctx.scale(scale as f64, scale as f64);
        }
        // Render all completed shapes from active frame
        debug!(
            "Rendering {} completed shapes",
            self.input_state.canvas_set.active_frame().shapes.len()
        );
        let eraser_ctx = crate::draw::EraserReplayContext {
            pattern: eraser_pattern.as_ref().map(|p| p as &cairo::Pattern),
            bg_color: eraser_bg_color,
        };
        crate::draw::render_shapes(
            &ctx,
            &self.input_state.canvas_set.active_frame().shapes,
            Some(&eraser_ctx),
        );

        // Render selection halo overlays
        if self.input_state.has_selection() {
            let selected: HashSet<_> = self
                .input_state
                .selected_shape_ids()
                .iter()
                .copied()
                .collect();
            let frame = self.input_state.canvas_set.active_frame();
            for drawn in &frame.shapes {
                if selected.contains(&drawn.id) {
                    crate::draw::render_selection_halo(&ctx, drawn);
                }
            }
        }

        // Render provisional shape if actively drawing
        // Use optimized method that avoids cloning for freehand
        let (mx, my) = self.current_mouse();
        if self.input_state.render_provisional_shape(&ctx, mx, my) {
            debug!("Rendered provisional shape");
        }

        // Render text cursor/buffer if in text mode
        if let DrawingState::TextInput { x, y, buffer } = &self.input_state.state {
            let preview_text = if buffer.is_empty() {
                "_".to_string() // Show cursor when buffer is empty
            } else {
                format!("{}_", buffer)
            };
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

        // Render click highlight overlays before UI so status/help remain legible
        self.input_state.render_click_highlights(&ctx, now);

        // Render frozen badge even if status bar is hidden
        if self.input_state.frozen_active() && self.config.ui.show_frozen_badge {
            crate::ui::render_frozen_badge(&ctx, width, height);
        }

        // Render status bar if enabled
        if self.input_state.show_status_bar {
            crate::ui::render_status_bar(
                &ctx,
                &self.input_state,
                self.config.ui.status_bar_position,
                &self.config.ui.status_bar_style,
                width,
                height,
            );
        }

        // Render help overlay if toggled
        if self.input_state.show_help {
            crate::ui::render_help_overlay(
                &ctx,
                &self.config.ui.help_overlay_style,
                width,
                height,
                self.frozen_enabled(),
            );
        }

        crate::ui::render_properties_panel(&ctx, &self.input_state, width, height);

        if self.input_state.is_context_menu_open() {
            self.input_state
                .update_context_menu_layout(&ctx, width, height);
        } else {
            self.input_state.clear_context_menu_layout();
        }

        // Render context menu if open
        crate::ui::render_context_menu(&ctx, &self.input_state, width, height);

        let _ = ctx.restore();

        // Flush Cairo
        debug!("Flushing Cairo surface");
        cairo_surface.flush();
        drop(ctx);
        drop(cairo_surface);

        // Attach buffer and commit
        debug!("Attaching buffer and committing surface");
        let wl_surface = self
            .surface
            .wl_surface()
            .cloned()
            .context("Surface not created")?;
        wl_surface.set_buffer_scale(scale);
        wl_surface.attach(Some(buffer.wl_buffer()), 0, 0);

        let dirty_regions = scale_damage_regions(
            resolve_damage_regions(
                self.surface.width().min(i32::MAX as u32) as i32,
                self.surface.height().min(i32::MAX as u32) as i32,
                self.input_state.take_dirty_regions(),
            ),
            scale,
        );

        if dirty_regions.is_empty() {
            debug!("No valid dirty regions; skipping damage request");
        } else {
            for rect in &dirty_regions {
                debug!(
                    "Damaging buffer region x={} y={} w={} h={}",
                    rect.x, rect.y, rect.width, rect.height
                );
                wl_surface.damage_buffer(rect.x, rect.y, rect.width, rect.height);
            }
        }

        if self.config.performance.enable_vsync {
            debug!("Requesting frame callback (vsync enabled)");
            wl_surface.frame(qh, wl_surface.clone());
        } else {
            debug!("Skipping frame callback (vsync disabled - allows back-to-back renders)");
        }

        wl_surface.commit();
        debug!("=== RENDER COMPLETE ===");

        // Render toolbar overlays if visible, only when state/hover changed.
        if self.toolbar.is_visible() {
            let snapshot = self.toolbar_snapshot();
            if self.toolbar.update_snapshot(&snapshot) {
                self.toolbar.mark_dirty();
            }
            self.render_toolbars(&snapshot);
        }

        Ok(highlight_active)
    }

    /// Returns a snapshot of the current input state for toolbar UI consumption.
    pub(super) fn toolbar_snapshot(&self) -> ToolbarSnapshot {
        let hints = ToolbarBindingHints::from_keybindings(&self.config.keybindings);
        ToolbarSnapshot::from_input_with_bindings(&self.input_state, hints)
    }

    /// Applies an incoming toolbar event and schedules redraws as needed.
    pub(super) fn handle_toolbar_event(&mut self, event: ToolbarEvent) {
        #[cfg(tablet)]
        let prev_thickness = self.input_state.current_thickness;
        #[cfg(tablet)]
        let thickness_event = matches!(
            event,
            ToolbarEvent::SetThickness(_) | ToolbarEvent::NudgeThickness(_)
        );

        // Check if this is a toolbar config event that needs saving
        let needs_config_save = matches!(
            event,
            ToolbarEvent::PinTopToolbar(_)
                | ToolbarEvent::PinSideToolbar(_)
                | ToolbarEvent::ToggleIconMode(_)
                | ToolbarEvent::ToggleMoreColors(_)
                | ToolbarEvent::ToggleActionsSection(_)
                | ToolbarEvent::ToggleDelaySliders(_)
                | ToolbarEvent::ToggleCustomSection(_)
        );

        let persist_drawing = matches!(
            event,
            ToolbarEvent::SetColor(_)
                | ToolbarEvent::SetThickness(_)
                | ToolbarEvent::SetFont(_)
                | ToolbarEvent::SetFontSize(_)
                | ToolbarEvent::ToggleFill(_)
        );

        if self.input_state.apply_toolbar_event(event) {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;

            #[cfg(tablet)]
            if thickness_event {
                self.sync_stylus_thickness_cache(prev_thickness);
                if self.stylus_tip_down {
                    self.record_stylus_peak(self.input_state.current_thickness);
                } else {
                    self.stylus_peak_thickness = None;
                }
            }

            // Save config when pin state changes
            if needs_config_save {
                self.save_toolbar_pin_config();
            }

            if persist_drawing {
                self.save_drawing_preferences();
            }
        }
        self.refresh_keyboard_interactivity();
    }

    #[cfg(tablet)]
    fn sync_stylus_thickness_cache(&mut self, prev: f64) {
        let cur = self.input_state.current_thickness;
        if (cur - prev).abs() > f64::EPSILON {
            self.stylus_base_thickness = Some(cur);
            if self.stylus_tip_down {
                self.stylus_pressure_thickness = Some(cur);
            } else {
                self.stylus_pressure_thickness = None;
            }
        }
    }

    /// Records the maximum stylus thickness seen during the current stroke.
    #[cfg(tablet)]
    pub(super) fn record_stylus_peak(&mut self, thickness: f64) {
        self.stylus_peak_thickness = Some(
            self.stylus_peak_thickness
                .map_or(thickness, |p| p.max(thickness)),
        );
    }

    /// Saves the current toolbar configuration to disk (pinned state, icon mode, section visibility).
    fn save_toolbar_pin_config(&mut self) {
        self.config.ui.toolbar.top_pinned = self.input_state.toolbar_top_pinned;
        self.config.ui.toolbar.side_pinned = self.input_state.toolbar_side_pinned;
        self.config.ui.toolbar.use_icons = self.input_state.toolbar_use_icons;
        self.config.ui.toolbar.show_more_colors = self.input_state.show_more_colors;
        self.config.ui.toolbar.show_actions_section = self.input_state.show_actions_section;
        self.config.ui.toolbar.show_delay_sliders = self.input_state.show_delay_sliders;
        self.config.ui.toolbar.show_marker_opacity_section =
            self.input_state.show_marker_opacity_section;
        // Step controls toggle is in history config
        self.config.history.custom_section_enabled = self.input_state.custom_section_enabled;

        if let Err(err) = self.config.save() {
            log::warn!("Failed to save toolbar config: {}", err);
        } else {
            log::debug!("Saved toolbar config");
        }
    }

    fn save_drawing_preferences(&mut self) {
        self.config.drawing.default_color = ColorSpec::from(self.input_state.current_color);
        self.config.drawing.default_thickness = self.input_state.current_thickness;
        self.config.drawing.default_fill_enabled = self.input_state.fill_enabled;
        self.config.drawing.default_font_size = self.input_state.current_font_size;
        self.config.drawing.font_family = self.input_state.font_descriptor.family.clone();
        self.config.drawing.font_weight = self.input_state.font_descriptor.weight.clone();
        self.config.drawing.font_style = self.input_state.font_descriptor.style.clone();
        self.config.drawing.marker_opacity = self.input_state.marker_opacity;

        if let Err(err) = self.config.save() {
            log::warn!("Failed to persist drawing preferences: {}", err);
        }
    }

    /// Restore the overlay after screenshot capture completes.
    ///
    /// Re-maps the layer surface to its original size and forces a redraw.
    pub(super) fn show_overlay(&mut self) {
        self.input_state.clear_click_highlights();
        if self.capture.show_overlay(&mut self.surface) {
            // Force a redraw to show the overlay again
            self.input_state.needs_redraw = true;
        }
    }

    /// Handles capture actions by delegating to the CaptureManager.
    pub(super) fn handle_capture_action(&mut self, action: Action) {
        if !self.config.capture.enabled {
            log::warn!("Capture action triggered but capture is disabled in config");
            return;
        }

        if self.capture.is_in_progress() {
            log::warn!(
                "Capture action {:?} requested while another capture is running; ignoring",
                action
            );
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
        self.capture.hide_overlay(&mut self.surface);
        self.capture.mark_in_progress();

        // Request capture
        log::info!("Requesting {:?} capture", capture_type);
        if let Err(e) =
            self.capture
                .manager_mut()
                .request_capture(capture_type, destination, save_config)
        {
            log::error!("Failed to request capture: {}", e);

            // Restore overlay on error
            self.show_overlay();
            self.capture.clear_in_progress();
        }
    }
}

fn resolve_damage_regions(width: i32, height: i32, mut regions: Vec<Rect>) -> Vec<Rect> {
    regions.retain(Rect::is_valid);

    if regions.is_empty() && width > 0 && height > 0 {
        if let Some(full) = Rect::new(0, 0, width, height) {
            regions.push(full);
        }
    }

    regions
}

fn scale_damage_regions(regions: Vec<Rect>, scale: i32) -> Vec<Rect> {
    if scale <= 1 {
        return regions;
    }

    regions
        .into_iter()
        .filter_map(|r| {
            let x = r.x.saturating_mul(scale);
            let y = r.y.saturating_mul(scale);
            let w = r.width.saturating_mul(scale);
            let h = r.height.saturating_mul(scale);

            Rect::new(x, y, w, h)
        })
        .collect()
}

impl ActivationHandler for WaylandState {
    type RequestData = RequestData;

    fn new_token(&mut self, token: String, _data: &Self::RequestData) {
        self.set_pending_activation_token(Some(token));
        self.activate_xdg_window_if_possible();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_damage_returns_full_when_empty() {
        let regions = resolve_damage_regions(1920, 1080, Vec::new());
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Rect::new(0, 0, 1920, 1080).unwrap());
    }

    #[test]
    fn resolve_damage_filters_invalid_rects() {
        let regions = resolve_damage_regions(
            800,
            600,
            vec![
                Rect {
                    x: 10,
                    y: 10,
                    width: 50,
                    height: 40,
                },
                Rect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 10,
                },
            ],
        );

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Rect::new(10, 10, 50, 40).unwrap());
    }

    #[test]
    fn resolve_damage_preserves_existing_regions() {
        let regions = resolve_damage_regions(
            800,
            600,
            vec![Rect {
                x: 5,
                y: 5,
                width: 20,
                height: 30,
            }],
        );

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Rect::new(5, 5, 20, 30).unwrap());
    }
}
