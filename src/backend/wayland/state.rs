// Holds the live Wayland protocol state shared by the backend loop and the handler
// submodules; provides rendering, capture routing, and overlay helpers used across them.
use crate::draw::Color;
use anyhow::{Context, Result};
use log::{debug, info, warn};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::{
    activation::{ActivationHandler, ActivationState, RequestData},
    compositor::CompositorState,
    globals::ProvidesBoundGlobal,
    output::OutputState,
    registry::RegistryState,
    seat::{
        SeatState,
        pointer::{PointerData, ThemedPointer},
        pointer_constraints::PointerConstraintsState,
        relative_pointer::RelativePointerState,
    },
    shell::{
        wlr_layer::{KeyboardInteractivity, LayerShell},
        xdg::XdgShell,
    },
    shm::Shm,
};
use std::time::Instant;
use std::{collections::HashSet, sync::OnceLock};
use wayland_client::{
    Proxy, QueueHandle,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};
#[cfg(tablet)]
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2::ZwpTabletManagerV2, zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2,
    zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2, zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2,
    zwp_tablet_pad_v2::ZwpTabletPadV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
    zwp_tablet_tool_v2::ZwpTabletToolV2, zwp_tablet_v2::ZwpTabletV2,
};
use wayland_protocols::wp::{
    pointer_constraints::zv1::client::{
        zwp_locked_pointer_v1::ZwpLockedPointerV1, zwp_pointer_constraints_v1,
    },
    relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1,
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
    input::{BoardMode, DrawingState, EraserMode, InputState, Tool, ZoomAction},
    notification,
    session::SessionOptions,
    ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot},
    util::Rect,
};

use self::data::{MoveDrag, StateData};
pub use self::data::{MoveDragKind, OverlaySuppression};
use super::{
    capture::CaptureState,
    frozen::FrozenState,
    overlay_passthrough::set_surface_clickthrough,
    session::SessionState,
    surface::SurfaceState,
    toolbar::{
        ToolbarSurfaceManager,
        hit::{drag_intent_for_hit, intent_for_hit},
        layout::{side_size, top_size},
        render::{render_side_palette, render_top_strip},
    },
    toolbar_intent::intent_to_event,
    zoom::ZoomState,
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
    #[allow(dead_code)] // Kept for potential future pointer lock support
    pub(super) pointer_constraints_state: PointerConstraintsState,
    #[allow(dead_code)] // Kept for potential future pointer lock support
    pub(super) relative_pointer_state: RelativePointerState,
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
    pub(super) zoom: ZoomState,

    // Pointer cursor
    pub(super) themed_pointer: Option<ThemedPointer<PointerData>>,
    pub(super) locked_pointer: Option<ZwpLockedPointerV1>,
    pub(super) relative_pointer: Option<ZwpRelativePointerV1>,

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
    const TOP_MARGIN_RIGHT: f64 = 12.0;
    const TOP_BASE_MARGIN_TOP: f64 = 12.0;
    const TOP_MARGIN_BOTTOM: f64 = 0.0;
    const SIDE_BASE_MARGIN_TOP: f64 = 24.0;
    const SIDE_MARGIN_BOTTOM: f64 = 24.0;
    const SIDE_BASE_MARGIN_LEFT: f64 = 24.0;
    const SIDE_MARGIN_RIGHT: f64 = 0.0;
    const INLINE_TOP_Y: f64 = 16.0;
    const INLINE_SIDE_X: f64 = 24.0;
    const TOOLBAR_CONFIGURE_FAIL_THRESHOLD: u32 = 180;
    const INLINE_TOP_PUSH: f64 = 16.0;
    const ZOOM_STEP_KEY: f64 = 1.2;
    const ZOOM_STEP_SCROLL: f64 = 1.1;
    pub(super) const ZOOM_PAN_STEP: f64 = 32.0;
    pub(super) const ZOOM_PAN_STEP_LARGE: f64 = 96.0;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        registry_state: RegistryState,
        compositor_state: CompositorState,
        layer_shell: Option<LayerShell>,
        xdg_shell: Option<XdgShell>,
        activation: Option<ActivationState>,
        shm: Shm,
        pointer_constraints_state: PointerConstraintsState,
        relative_pointer_state: RelativePointerState,
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
        let force_inline_toolbars = force_inline_toolbars_requested(&config);
        data.inline_toolbars = layer_shell.is_none() || force_inline_toolbars;
        if force_inline_toolbars {
            info!(
                "Forcing inline toolbars (config/ui.toolbar.force_inline or WAYSCRIBER_FORCE_INLINE_TOOLBARS)"
            );
        }
        data.toolbar_top_offset = config.ui.toolbar.top_offset;
        data.toolbar_top_offset_y = config.ui.toolbar.top_offset_y;
        data.toolbar_side_offset = config.ui.toolbar.side_offset;
        data.toolbar_side_offset_x = config.ui.toolbar.side_offset_x;
        drag_log(format!(
            "load offsets from config: top_offset=({}, {}), side_offset=({}, {})",
            data.toolbar_top_offset,
            data.toolbar_top_offset_y,
            data.toolbar_side_offset,
            data.toolbar_side_offset_x
        ));
        let zoom_manager = screencopy_manager.clone();

        Self {
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
            surface: SurfaceState::new(),
            toolbar: ToolbarSurfaceManager::new(),
            data,
            config,
            input_state,
            capture: CaptureState::new(capture_manager),
            frozen: FrozenState::new(screencopy_manager),
            zoom: ZoomState::new(zoom_manager),
            themed_pointer: None,
            locked_pointer: None,
            relative_pointer: None,
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

    pub(super) fn overlay_suppressed(&self) -> bool {
        self.data.overlay_suppression != OverlaySuppression::None
    }

    fn apply_overlay_clickthrough(&mut self, clickthrough: bool) {
        if let Some(wl_surface) = self.surface.wl_surface().cloned() {
            set_surface_clickthrough(&self.compositor_state, &wl_surface, clickthrough);
        }
        self.toolbar
            .set_suppressed(&self.compositor_state, clickthrough);
    }

    pub(super) fn enter_overlay_suppression(&mut self, reason: OverlaySuppression) {
        if self.data.overlay_suppression != OverlaySuppression::None {
            return;
        }
        self.data.overlay_suppression = reason;
        self.apply_overlay_clickthrough(true);
        self.input_state.needs_redraw = true;
        self.toolbar.mark_dirty();
    }

    pub(super) fn exit_overlay_suppression(&mut self, reason: OverlaySuppression) {
        if self.data.overlay_suppression != reason {
            return;
        }
        self.data.overlay_suppression = OverlaySuppression::None;
        self.apply_overlay_clickthrough(false);
        self.input_state.needs_redraw = true;
        self.toolbar.mark_dirty();
    }

    pub(super) fn apply_capture_completion(&mut self) {
        if self.frozen.take_capture_done() {
            self.exit_overlay_suppression(OverlaySuppression::Frozen);
        }
        if self.zoom.take_capture_done() {
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
        }
    }

    pub(super) fn sync_zoom_board_mode(&mut self) {
        let board_mode = self.input_state.board_mode();
        if board_mode != BoardMode::Transparent {
            if self.data.overlay_suppression == OverlaySuppression::Zoom {
                self.exit_overlay_suppression(OverlaySuppression::Zoom);
            }
            if self.zoom.abort_capture() {
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            if self.zoom.is_engaged() && !self.zoom.active {
                self.zoom.activate_without_capture();
                self.input_state
                    .set_zoom_status(true, self.zoom.locked, self.zoom.scale);
            }
            if self.zoom.clear_image() {
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            return;
        }

        if self.zoom.is_engaged()
            && self.zoom.image().is_none()
            && !self.zoom.is_in_progress()
            && let Err(err) = self.start_zoom_capture(false)
        {
            warn!("Zoom capture failed to start: {}", err);
            self.zoom.deactivate(&mut self.input_state);
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
        }
    }

    pub(super) fn zoomed_world_coords(&self, screen_x: f64, screen_y: f64) -> (i32, i32) {
        if self.zoom.active {
            let (wx, wy) = self.zoom.screen_to_world(screen_x, screen_y);
            (wx.round() as i32, wy.round() as i32)
        } else {
            (screen_x.round() as i32, screen_y.round() as i32)
        }
    }

    pub(super) fn handle_zoom_action(&mut self, action: ZoomAction) {
        let (sx, sy) = self.zoom_keyboard_anchor();
        match action {
            ZoomAction::In => {
                self.apply_zoom_factor(Self::ZOOM_STEP_KEY, sx, sy, true);
            }
            ZoomAction::Out => {
                self.apply_zoom_factor(1.0 / Self::ZOOM_STEP_KEY, sx, sy, false);
            }
            ZoomAction::Reset => {
                if self.zoom.is_engaged() {
                    self.exit_zoom();
                }
            }
            ZoomAction::ToggleLock => {
                if self.zoom.active {
                    self.zoom.locked = !self.zoom.locked;
                    if self.zoom.locked && self.zoom.panning {
                        self.zoom.stop_pan();
                    }
                    self.input_state.set_zoom_status(
                        self.zoom.active,
                        self.zoom.locked,
                        self.zoom.scale,
                    );
                }
            }
            ZoomAction::RefreshCapture => {
                if self.input_state.board_mode() != BoardMode::Transparent {
                    info!("Zoom capture refresh ignored in board mode");
                } else if self.zoom.active
                    && let Err(err) = self.start_zoom_capture(true)
                {
                    warn!("Zoom capture refresh failed: {}", err);
                }
            }
        }
    }

    fn zoom_keyboard_anchor(&self) -> (f64, f64) {
        if self.has_pointer_focus() {
            let (sx, sy) = self.current_mouse();
            (sx as f64, sy as f64)
        } else {
            let cx = (self.surface.width() as f64) * 0.5;
            let cy = (self.surface.height() as f64) * 0.5;
            (cx, cy)
        }
    }

    pub(super) fn handle_zoom_scroll(&mut self, zoom_in: bool, screen_x: f64, screen_y: f64) {
        let factor = if zoom_in {
            Self::ZOOM_STEP_SCROLL
        } else {
            1.0 / Self::ZOOM_STEP_SCROLL
        };
        self.apply_zoom_factor(factor, screen_x, screen_y, zoom_in);
    }

    pub(super) fn exit_zoom(&mut self) {
        if self.zoom.is_engaged() {
            self.zoom.deactivate(&mut self.input_state);
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
        }
    }

    fn apply_zoom_factor(
        &mut self,
        factor: f64,
        screen_x: f64,
        screen_y: f64,
        allow_activate: bool,
    ) {
        let screen_w = self.surface.width();
        let screen_h = self.surface.height();
        let board_zoom = self.input_state.board_mode() != BoardMode::Transparent;
        if board_zoom {
            let mut cleared = false;
            if self.zoom.abort_capture() {
                cleared = true;
                self.exit_overlay_suppression(OverlaySuppression::Zoom);
            }
            if self.zoom.clear_image() {
                cleared = true;
            }
            if cleared {
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
        }

        if !self.zoom.is_engaged() {
            if !allow_activate {
                return;
            }
            self.zoom.locked = false;
            self.zoom.reset_view();
            self.input_state.close_context_menu();
            self.input_state.close_properties_panel();
            if board_zoom {
                self.zoom.activate_without_capture();
                self.input_state
                    .set_zoom_status(true, self.zoom.locked, self.zoom.scale);
            } else {
                self.zoom.request_activation();
            }
        } else if board_zoom && !self.zoom.active {
            self.zoom.activate_without_capture();
            self.input_state
                .set_zoom_status(true, self.zoom.locked, self.zoom.scale);
        }

        let changed = self
            .zoom
            .zoom_at_screen_point(factor, screen_x, screen_y, screen_w, screen_h);
        if self.zoom.active && changed {
            self.input_state
                .set_zoom_status(true, self.zoom.locked, self.zoom.scale);
        }

        if self.zoom.is_engaged()
            && !board_zoom
            && let Err(err) = self.start_zoom_capture(false)
        {
            warn!("Zoom capture failed to start: {}", err);
            self.zoom.deactivate(&mut self.input_state);
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
        }
    }

    fn start_zoom_capture(&mut self, force: bool) -> Result<()> {
        if self.zoom.is_in_progress() {
            return Ok(());
        }
        if !force && self.zoom.image().is_some() {
            return Ok(());
        }
        if self.input_state.board_mode() != BoardMode::Transparent {
            debug!("Zoom capture skipped in board mode");
            return Ok(());
        }
        if self.frozen.is_in_progress() {
            warn!("Zoom capture requested while frozen capture is in progress; ignoring");
            return Ok(());
        }
        let use_fallback = !self.zoom.manager_available();
        if use_fallback {
            warn!("Zoom: screencopy unavailable, using portal fallback");
        } else {
            log::info!("Zoom: using screencopy fast path");
        }
        self.enter_overlay_suppression(OverlaySuppression::Zoom);
        match self.zoom.start_capture(use_fallback, &self.tokio_handle) {
            Ok(()) => Ok(()),
            Err(err) => {
                self.exit_overlay_suppression(OverlaySuppression::Zoom);
                Err(err)
            }
        }
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

    #[allow(dead_code)] // Kept for potential future pointer lock support
    pub(super) fn current_pointer(&self) -> Option<wl_pointer::WlPointer> {
        self.themed_pointer.as_ref().map(|p| p.pointer().clone())
    }

    pub(super) fn toolbar_top_offset(&self) -> f64 {
        self.data.toolbar_top_offset
    }

    pub(super) fn toolbar_top_offset_y(&self) -> f64 {
        self.data.toolbar_top_offset_y
    }

    pub(super) fn toolbar_side_offset(&self) -> f64 {
        self.data.toolbar_side_offset
    }

    pub(super) fn toolbar_side_offset_x(&self) -> f64 {
        self.data.toolbar_side_offset_x
    }

    pub(super) fn pointer_lock_active(&self) -> bool {
        self.locked_pointer.is_some()
    }

    #[allow(dead_code)] // Kept for potential future pointer lock support
    pub(super) fn lock_pointer_for_drag(
        &mut self,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
    ) {
        if self.inline_toolbars_active() || self.pointer_lock_active() {
            log::info!(
                "skip pointer lock: inline_active={}, already_locked={}",
                self.inline_toolbars_active(),
                self.pointer_lock_active()
            );
            return;
        }
        if self.pointer_constraints_state.bound_global().is_err() {
            log::info!("pointer lock unavailable: constraints global missing");
            return;
        }
        let Some(pointer) = self.current_pointer() else {
            log::info!("pointer lock unavailable: no current pointer");
            return;
        };

        match self.pointer_constraints_state.lock_pointer(
            surface,
            &pointer,
            None,
            zwp_pointer_constraints_v1::Lifetime::Oneshot,
            qh,
        ) {
            Ok(lp) => {
                self.locked_pointer = Some(lp);
                log::info!(
                    "pointer lock requested: seat={:?}, surface={}, pointer_id={}",
                    self.current_seat_id(),
                    surface_id(surface),
                    pointer.id().protocol_id()
                );
            }
            Err(err) => {
                warn!("Failed to lock pointer for toolbar drag: {}", err);
                return;
            }
        }

        match self
            .relative_pointer_state
            .get_relative_pointer(&pointer, qh)
        {
            Ok(rp) => {
                self.relative_pointer = Some(rp);
                log::info!("relative pointer bound for drag");
            }
            Err(err) => {
                warn!("Failed to obtain relative pointer for drag: {}", err);
                // Abort lock if relative pointer is unavailable; fall back to absolute path.
                self.unlock_pointer();
            }
        }
    }

    pub(super) fn unlock_pointer(&mut self) {
        if let Some(lp) = self.locked_pointer.take() {
            lp.destroy();
        }
        if let Some(rp) = self.relative_pointer.take() {
            rp.destroy();
        }
    }

    pub(super) fn current_seat_id(&self) -> Option<u32> {
        self.data
            .current_seat
            .as_ref()
            .map(|seat| seat.id().protocol_id())
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
        if let Some(preferred) = self.preferred_output_identity()
            && let Some(output) = self.output_state.outputs().find(|output| {
                self.output_identity_for(output)
                    .map(|id| id.eq_ignore_ascii_case(preferred))
                    .unwrap_or(false)
            })
        {
            return Some(output);
        }

        self.surface
            .current_output()
            .or_else(|| self.output_state.outputs().next())
    }

    /// Determines the desired keyboard interactivity for the main layer surface.
    pub(super) fn desired_keyboard_interactivity(&self) -> KeyboardInteractivity {
        if self.layer_shell.is_some() && self.toolbar.is_visible() {
            KeyboardInteractivity::OnDemand
        } else {
            KeyboardInteractivity::Exclusive
        }
    }

    fn log_toolbar_layer_shell_missing_once(&mut self) {
        if self.data.toolbar_layer_shell_missing_logged {
            return;
        }

        let desktop_env = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into());
        let session_env = std::env::var("XDG_SESSION_DESKTOP").unwrap_or_else(|_| "unknown".into());
        log::warn!(
            "Layer-shell protocol unavailable; toolbar surfaces will not appear (desktop='{}', session='{}'). Overlay may be limited to the work area on compositors like GNOME.",
            desktop_env,
            session_env
        );
        self.data.toolbar_layer_shell_missing_logged = true;
    }

    fn notify_toolbar_layer_shell_missing_once(&mut self) {
        if self.data.toolbar_layer_shell_notice_sent {
            return;
        }

        self.data.toolbar_layer_shell_notice_sent = true;
        let summary = "Toolbars unavailable on this desktop";
        let body = "This compositor does not expose the layer-shell protocol, so the toolbar surfaces cannot be created. Try a compositor with layer-shell support or an X11 session.";
        notification::send_notification_async(
            &self.tokio_handle,
            summary.to_string(),
            body.to_string(),
            Some("dialog-warning".to_string()),
        );
        log::warn!("{}", summary);
        log::warn!("{}", body);
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
        let inline_active = self.inline_toolbars_active();

        if top_visible != self.toolbar.is_top_visible() {
            self.toolbar.set_top_visible(top_visible);
            self.input_state.needs_redraw = true;
        }

        if side_visible != self.toolbar.is_side_visible() {
            self.toolbar.set_side_visible(side_visible);
            self.input_state.needs_redraw = true;
            drag_log(format!(
                "toolbar visibility change: side -> {}",
                side_visible
            ));
        }

        let any_visible = self.toolbar.is_visible();
        if !any_visible {
            self.set_pointer_over_toolbar(false);
            self.data.toolbar_configure_miss_count = 0;
        }

        if any_visible {
            log::debug!(
                "Toolbar visibility sync: top_visible={}, side_visible={}, layer_shell_available={}, inline_active={}, top_created={}, side_created={}, needs_recreate={}, scale={}",
                top_visible,
                side_visible,
                self.layer_shell.is_some(),
                inline_active,
                self.toolbar.top_created(),
                self.toolbar.side_created(),
                self.toolbar_needs_recreate(),
                self.surface.scale()
            );
            drag_log(format!(
                "toolbar sync: top_offset=({}, {}), side_offset=({}, {}), inline_active={}, layer_shell={}, needs_recreate={}",
                self.data.toolbar_top_offset,
                self.data.toolbar_top_offset_y,
                self.data.toolbar_side_offset,
                self.data.toolbar_side_offset_x,
                inline_active,
                self.layer_shell.is_some(),
                self.toolbar_needs_recreate()
            ));
        }

        // Warn the user when layer-shell is unavailable and we're forced to inline fallback.
        if any_visible && self.layer_shell.is_none() {
            self.log_toolbar_layer_shell_missing_once();
            self.notify_toolbar_layer_shell_missing_once();
        }

        if any_visible && inline_active {
            // If we forced inline while layer surfaces already existed, tear them down to avoid
            // focus/input conflicts on compositors that support layer-shell.
            if self.toolbar.top_created() || self.toolbar.side_created() {
                self.toolbar.destroy_all();
                self.set_toolbar_needs_recreate(true);
            }
            self.data.toolbar_configure_miss_count = 0;
        }

        if any_visible && self.layer_shell.is_some() && !inline_active {
            // Detect compositors ignoring or failing to configure toolbar layer surfaces; if they
            // never configure after repeated attempts, fall back to inline toolbars automatically.
            let (top_configured, side_configured) = self.toolbar.configured_states();
            let expected_top = self.toolbar.is_top_visible();
            let expected_side = self.toolbar.is_side_visible();
            if (expected_top && !top_configured) || (expected_side && !side_configured) {
                self.data.toolbar_configure_miss_count =
                    self.data.toolbar_configure_miss_count.saturating_add(1);
                if debug_toolbar_drag_logging_enabled()
                    && self.data.toolbar_configure_miss_count.is_multiple_of(60)
                {
                    debug!(
                        "Toolbar configure pending: count={}, expected_top={}, configured_top={}, expected_side={}, configured_side={}",
                        self.data.toolbar_configure_miss_count,
                        expected_top,
                        top_configured,
                        expected_side,
                        side_configured
                    );
                }
            } else {
                self.data.toolbar_configure_miss_count = 0;
            }

            if self.data.toolbar_configure_miss_count > Self::TOOLBAR_CONFIGURE_FAIL_THRESHOLD {
                warn!(
                    "Toolbar layer surfaces did not configure after {} frames; falling back to inline toolbars",
                    self.data.toolbar_configure_miss_count
                );
                self.toolbar.destroy_all();
                self.data.inline_toolbars = true;
                self.set_toolbar_needs_recreate(true);
                self.data.toolbar_configure_miss_count = 0;
                // Re-run visibility sync with inline mode enabled.
                self.sync_toolbar_visibility(qh);
                return;
            }

            if self.toolbar_needs_recreate() {
                self.toolbar.destroy_all();
                self.set_toolbar_needs_recreate(false);
            }
            let snapshot = self.toolbar_snapshot();
            self.apply_toolbar_offsets(&snapshot);
            if let Some(layer_shell) = self.layer_shell.as_ref() {
                let scale = self.surface.scale();
                self.toolbar.ensure_created(
                    qh,
                    &self.compositor_state,
                    layer_shell,
                    scale,
                    &snapshot,
                );
            }
        }

        if !any_visible {
            self.clear_inline_toolbar_hits();
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

    fn point_in_rect(&self, px: f64, py: f64, x: f64, y: f64, w: f64, h: f64) -> bool {
        px >= x && px <= x + w && py >= y && py <= y + h
    }

    pub(super) fn inline_toolbars_active(&self) -> bool {
        self.data.inline_toolbars
    }

    /// Base X position for the top toolbar when laid out inline.
    /// When a drag is in progress we freeze this base to avoid shifting the top bar while moving the side bar.
    fn inline_top_base_x(&self, snapshot: &ToolbarSnapshot) -> f64 {
        if self.is_move_dragging()
            && let Some(x) = self.data.drag_top_base_x
        {
            return x;
        }
        let side_visible = self.toolbar.is_side_visible();
        let side_size = side_size(snapshot);
        let top_size = top_size(snapshot);
        let side_start_y = Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset;
        let top_bottom_y = Self::INLINE_TOP_Y + self.data.toolbar_top_offset_y + top_size.1 as f64;
        let side_overlaps_top = side_visible && side_start_y < top_bottom_y;
        let base = Self::INLINE_SIDE_X + self.data.toolbar_side_offset_x;
        // When dragging the side toolbar, don't push the top bar; keep its base stable so it
        // doesn't shift while moving the side bar.
        if side_overlaps_top && self.active_move_drag_kind() != Some(MoveDragKind::Side) {
            base + side_size.0 as f64 + Self::INLINE_TOP_PUSH
        } else {
            base
        }
    }

    fn inline_top_base_y(&self) -> f64 {
        if self.is_move_dragging()
            && let Some(y) = self.data.drag_top_base_y
        {
            return y;
        }
        Self::INLINE_TOP_Y
    }

    /// Convert a toolbar-local coordinate into a screen-relative coordinate so that
    /// dragging continues to work even after the surface has moved.
    fn local_to_screen_coords(&self, kind: MoveDragKind, local_coord: (f64, f64)) -> (f64, f64) {
        match kind {
            MoveDragKind::Top => (
                self.inline_top_base_x(&self.toolbar_snapshot())
                    + self.data.toolbar_top_offset
                    + local_coord.0,
                self.inline_top_base_y() + self.data.toolbar_top_offset_y + local_coord.1,
            ),
            MoveDragKind::Side => (
                Self::SIDE_BASE_MARGIN_LEFT + self.data.toolbar_side_offset_x + local_coord.0,
                Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset + local_coord.1,
            ),
        }
    }

    fn clamp_toolbar_offsets(&mut self, snapshot: &ToolbarSnapshot) -> bool {
        let width = self.surface.width() as f64;
        let height = self.surface.height() as f64;
        if width == 0.0 || height == 0.0 {
            drag_log(format!(
                "skip clamp: surface not configured (width={}, height={})",
                width, height
            ));
            return false;
        }
        let (top_w, top_h) = top_size(snapshot);
        let (side_w, side_h) = side_size(snapshot);
        let top_base_x = self.inline_top_base_x(snapshot);
        let top_base_y = self.inline_top_base_y();

        let mut max_top_x = (width - top_w as f64 - top_base_x - Self::TOP_MARGIN_RIGHT).max(0.0);
        let mut max_top_y = (height - top_h as f64 - top_base_y - Self::TOP_MARGIN_BOTTOM).max(0.0);
        let mut max_side_y =
            (height - side_h as f64 - Self::SIDE_BASE_MARGIN_TOP - Self::SIDE_MARGIN_BOTTOM)
                .max(0.0);
        let mut max_side_x =
            (width - side_w as f64 - Self::SIDE_BASE_MARGIN_LEFT - Self::SIDE_MARGIN_RIGHT)
                .max(0.0);

        // Ensure max bounds remain non-negative.
        max_top_x = max_top_x.max(0.0);
        max_top_y = max_top_y.max(0.0);
        max_side_x = max_side_x.max(0.0);
        max_side_y = max_side_y.max(0.0);

        // Allow negative offsets so toolbars can reach screen edges by cancelling base margins
        let min_top_x = -top_base_x;
        let min_top_y = -top_base_y;
        let min_side_x = -Self::SIDE_BASE_MARGIN_LEFT;
        let min_side_y = -Self::SIDE_BASE_MARGIN_TOP;

        let before_top = (self.data.toolbar_top_offset, self.data.toolbar_top_offset_y);
        let before_side = (
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset,
        );
        self.data.toolbar_top_offset = self.data.toolbar_top_offset.clamp(min_top_x, max_top_x);
        self.data.toolbar_top_offset_y = self.data.toolbar_top_offset_y.clamp(min_top_y, max_top_y);
        self.data.toolbar_side_offset = self.data.toolbar_side_offset.clamp(min_side_y, max_side_y);
        self.data.toolbar_side_offset_x = self
            .data
            .toolbar_side_offset_x
            .clamp(min_side_x, max_side_x);
        drag_log(format!(
            "clamp offsets: before=({:.3}, {:.3})/({:.3}, {:.3}), after=({:.3}, {:.3})/({:.3}, {:.3}), max=({:.3}, {:.3})/({:.3}, {:.3}), size=({}, {}), top_base_x={:.3}, top_base_y={:.3}",
            before_top.0,
            before_top.1,
            before_side.0,
            before_side.1,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset,
            max_top_x,
            max_top_y,
            max_side_x,
            max_side_y,
            width,
            height,
            top_base_x,
            top_base_y
        ));
        true
    }

    fn begin_toolbar_move_drag(&mut self, kind: MoveDragKind, local_coord: (f64, f64)) {
        if self.data.toolbar_move_drag.is_none() {
            log::debug!(
                "Begin toolbar move drag: kind={:?}, local_coord=({:.3}, {:.3})",
                kind,
                local_coord.0,
                local_coord.1
            );
            // Store as local coords since the initial press is on the toolbar surface
            self.data.toolbar_move_drag = Some(MoveDrag {
                kind,
                last_coord: local_coord,
                coord_is_screen: false,
            });
            // Freeze base positions so the other toolbar doesn't push while dragging.
            let snapshot = self.toolbar_snapshot();
            self.data.drag_top_base_x = Some(self.inline_top_base_x(&snapshot));
            self.data.drag_top_base_y = Some(self.inline_top_base_y());
        }
        self.data.active_drag_kind = Some(kind);
        self.set_toolbar_dragging(true);
    }

    fn apply_toolbar_offsets(&mut self, snapshot: &ToolbarSnapshot) {
        if self.surface.width() == 0 || self.surface.height() == 0 {
            drag_log(format!(
                "skip apply_toolbar_offsets: surface not configured (width={}, height={})",
                self.surface.width(),
                self.surface.height()
            ));
            return;
        }
        let _ = self.clamp_toolbar_offsets(snapshot);
        if self.layer_shell.is_some() {
            let top_base_x = self.inline_top_base_x(snapshot);
            let top_margin_left = (top_base_x + self.data.toolbar_top_offset).round() as i32;
            let top_margin_top =
                (Self::TOP_BASE_MARGIN_TOP + self.data.toolbar_top_offset_y).round() as i32;
            let side_margin_top =
                (Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset).round() as i32;
            let side_margin_left =
                (Self::SIDE_BASE_MARGIN_LEFT + self.data.toolbar_side_offset_x).round() as i32;
            drag_log(format!(
                "apply_toolbar_offsets: top_margin_left={}, top_margin_top={}, side_margin_top={}, side_margin_left={}, offsets=({}, {})/({}, {}), scale={}, top_base_x={}",
                top_margin_left,
                top_margin_top,
                side_margin_top,
                side_margin_left,
                self.data.toolbar_top_offset,
                self.data.toolbar_top_offset_y,
                self.data.toolbar_side_offset_x,
                self.data.toolbar_side_offset,
                self.surface.scale(),
                top_base_x
            ));
            if debug_toolbar_drag_logging_enabled() {
                debug!(
                    "apply_toolbar_offsets: top_margin_left={} (last={:?}), top_margin_top={} (last={:?}), side_margin_top={} (last={:?}), side_margin_left={} (last={:?}), offsets=({}, {})/({}, {}), top_base_x={}",
                    top_margin_left,
                    self.data.last_applied_top_margin,
                    top_margin_top,
                    self.data.last_applied_top_margin_top,
                    side_margin_top,
                    self.data.last_applied_side_margin,
                    side_margin_left,
                    self.data.last_applied_side_margin_left,
                    self.data.toolbar_top_offset,
                    self.data.toolbar_top_offset_y,
                    self.data.toolbar_side_offset_x,
                    self.data.toolbar_side_offset,
                    top_base_x
                );
            }
            self.data.last_applied_top_margin = Some(top_margin_left);
            self.data.last_applied_side_margin = Some(side_margin_top);
            self.data.last_applied_top_margin_top = Some(top_margin_top);
            self.data.last_applied_side_margin_left = Some(side_margin_left);
            self.toolbar.set_top_margin_left(top_margin_left);
            self.toolbar.set_top_margin_top(top_margin_top);
            self.toolbar.set_side_margin_top(side_margin_top);
            self.toolbar.set_side_margin_left(side_margin_left);
            self.toolbar.mark_dirty();
        }
    }

    /// Handle toolbar move with toolbar-surface-local coordinates.
    /// On layer-shell, toolbar-local coords stay consistent as the toolbar moves,
    /// so we use them directly for delta calculation.
    pub(super) fn handle_toolbar_move(&mut self, kind: MoveDragKind, local_coord: (f64, f64)) {
        if self.pointer_lock_active() {
            return;
        }
        // For layer-shell surfaces, use local coordinates directly since they're
        // consistent within the toolbar surface. Only convert to screen coords
        // when transitioning to/from main surface.
        self.handle_toolbar_move_local(kind, local_coord);
    }

    /// Handle toolbar move with toolbar-surface-local coordinates.
    fn handle_toolbar_move_local(&mut self, kind: MoveDragKind, local_coord: (f64, f64)) {
        let snapshot = self
            .toolbar
            .last_snapshot()
            .cloned()
            .unwrap_or_else(|| self.toolbar_snapshot());

        // Check if we need to transition coordinate systems
        let (last_coord, coord_is_screen) = match &self.data.toolbar_move_drag {
            Some(d) if d.kind == kind => (d.last_coord, d.coord_is_screen),
            _ => (local_coord, false), // Start fresh with local coords
        };

        // If last coord was screen-based, convert current local to screen for comparison
        let last_screen = if coord_is_screen {
            last_coord
        } else {
            self.local_to_screen_coords(kind, last_coord)
        };
        let effective_coord = self.local_to_screen_coords(kind, local_coord);

        self.data.active_drag_kind = Some(kind);

        let delta = (
            effective_coord.0 - last_screen.0,
            effective_coord.1 - last_screen.1,
        );
        log::debug!(
            "handle_toolbar_move_local: kind={:?}, local_coord=({:.3}, {:.3}), effective_coord=({:.3}, {:.3}), last_coord=({:.3}, {:.3}), delta=({:.3}, {:.3}), offsets=({}, {})/({}, {})",
            kind,
            local_coord.0,
            local_coord.1,
            effective_coord.0,
            effective_coord.1,
            last_screen.0,
            last_screen.1,
            delta.0,
            delta.1,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset
        );

        match kind {
            MoveDragKind::Top => {
                self.data.toolbar_top_offset += delta.0;
                self.data.toolbar_top_offset_y += delta.1;
            }
            MoveDragKind::Side => {
                self.data.toolbar_side_offset_x += delta.0;
                self.data.toolbar_side_offset += delta.1;
            }
        }
        log::debug!(
            "After update offsets: top=({}, {}), side=({}, {})",
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset
        );

        self.data.toolbar_move_drag = Some(MoveDrag {
            kind,
            last_coord: effective_coord,
            coord_is_screen: true,
        });
        self.apply_toolbar_offsets(&snapshot);
        // Force commits so compositors apply new margins immediately.
        if let Some(layer) = self.toolbar.top_layer_surface() {
            layer.wl_surface().commit();
        }
        if let Some(layer) = self.toolbar.side_layer_surface() {
            layer.wl_surface().commit();
        }
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
        self.clamp_toolbar_offsets(&snapshot);

        if self.layer_shell.is_none() || self.inline_toolbars_active() {
            self.clear_inline_toolbar_hits();
        }
    }

    /// Handle toolbar move with screen-relative coordinates (no conversion).
    /// Use this when coords are already in screen space (e.g., from main overlay surface).
    pub(super) fn handle_toolbar_move_screen(
        &mut self,
        kind: MoveDragKind,
        screen_coord: (f64, f64),
    ) {
        if self.pointer_lock_active() {
            return;
        }
        let snapshot = self
            .toolbar
            .last_snapshot()
            .cloned()
            .unwrap_or_else(|| self.toolbar_snapshot());

        // Get last coord, converting from local to screen if needed
        let last_screen_coord = match self.data.toolbar_move_drag {
            Some(d) if d.kind == kind => {
                if d.coord_is_screen {
                    d.last_coord
                } else {
                    self.local_to_screen_coords(kind, d.last_coord)
                }
            }
            _ => screen_coord, // Start fresh
        };

        self.data.active_drag_kind = Some(kind);

        let delta = (
            screen_coord.0 - last_screen_coord.0,
            screen_coord.1 - last_screen_coord.1,
        );
        log::debug!(
            "handle_toolbar_move_screen: kind={:?}, screen_coord=({:.3}, {:.3}), last_screen_coord=({:.3}, {:.3}), delta=({:.3}, {:.3}), offsets=({}, {})/({}, {})",
            kind,
            screen_coord.0,
            screen_coord.1,
            last_screen_coord.0,
            last_screen_coord.1,
            delta.0,
            delta.1,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset
        );
        match kind {
            MoveDragKind::Top => {
                self.data.toolbar_top_offset += delta.0;
                self.data.toolbar_top_offset_y += delta.1;
            }
            MoveDragKind::Side => {
                self.data.toolbar_side_offset_x += delta.0;
                self.data.toolbar_side_offset += delta.1;
            }
        }

        self.data.toolbar_move_drag = Some(MoveDrag {
            kind,
            last_coord: screen_coord,
            coord_is_screen: true,
        });
        self.apply_toolbar_offsets(&snapshot);
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;

        // Ensure we don't drift off-screen.
        self.clamp_toolbar_offsets(&snapshot);

        if self.layer_shell.is_none() || self.inline_toolbars_active() {
            // Inline mode uses cached rects, so force a relayout.
            self.clear_inline_toolbar_hits();
        }
    }

    /// Apply a relative delta to toolbar offsets (used with locked pointer + relative motion).
    pub(super) fn apply_toolbar_relative_delta(&mut self, kind: MoveDragKind, delta: (f64, f64)) {
        let snapshot = self
            .toolbar
            .last_snapshot()
            .cloned()
            .unwrap_or_else(|| self.toolbar_snapshot());

        match kind {
            MoveDragKind::Top => {
                self.data.toolbar_top_offset += delta.0;
                self.data.toolbar_top_offset_y += delta.1;
            }
            MoveDragKind::Side => {
                self.data.toolbar_side_offset_x += delta.0;
                self.data.toolbar_side_offset += delta.1;
            }
        }

        self.clamp_toolbar_offsets(&snapshot);
        self.apply_toolbar_offsets(&snapshot);
        // Commit both toolbar surfaces immediately to force the compositor to apply margins.
        if let Some(layer) = self.toolbar.top_layer_surface() {
            layer.wl_surface().commit();
        }
        if let Some(layer) = self.toolbar.side_layer_surface() {
            layer.wl_surface().commit();
        }

        drag_log(format!(
            "relative delta applied: kind={:?}, delta=({:.3}, {:.3}), offsets=({}, {})/({}, {})",
            kind,
            delta.0,
            delta.1,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset
        ));

        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
    }

    pub(super) fn end_toolbar_move_drag(&mut self) {
        if self.data.toolbar_move_drag.is_some() {
            self.data.toolbar_move_drag = None;
            self.set_toolbar_dragging(false);
            self.set_pointer_over_toolbar(false);
            self.data.active_drag_kind = None;
            self.data.drag_top_base_x = None;
            self.data.drag_top_base_y = None;
            self.save_toolbar_pin_config();
            self.unlock_pointer();
        }
    }

    fn clear_inline_toolbar_hits(&mut self) {
        self.data.inline_top_hits.clear();
        self.data.inline_side_hits.clear();
        self.data.inline_top_rect = None;
        self.data.inline_side_rect = None;
        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;
    }

    fn render_inline_toolbars(&mut self, ctx: &cairo::Context, snapshot: &ToolbarSnapshot) {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            self.clear_inline_toolbar_hits();
            return;
        }

        self.clear_inline_toolbar_hits();
        self.clamp_toolbar_offsets(snapshot);

        let top_size = top_size(snapshot);
        let side_size = side_size(snapshot);

        // Position inline toolbars with padding and keep top bar to the right of the side bar.
        let side_offset = (
            Self::INLINE_SIDE_X + self.data.toolbar_side_offset_x,
            Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset,
        );

        let top_base_x = self.inline_top_base_x(snapshot);
        let top_offset = (
            top_base_x + self.data.toolbar_top_offset,
            self.inline_top_base_y() + self.data.toolbar_top_offset_y,
        );

        // Top toolbar
        let top_hover_local = self
            .data
            .inline_top_hover
            .map(|(x, y)| (x - top_offset.0, y - top_offset.1));
        let _ = ctx.save();
        ctx.translate(top_offset.0, top_offset.1);
        if let Err(err) = render_top_strip(
            ctx,
            top_size.0 as f64,
            top_size.1 as f64,
            snapshot,
            &mut self.data.inline_top_hits,
            top_hover_local,
        ) {
            log::warn!("Failed to render inline top toolbar: {}", err);
        }
        let _ = ctx.restore();
        for hit in &mut self.data.inline_top_hits {
            hit.rect.0 += top_offset.0;
            hit.rect.1 += top_offset.1;
        }
        self.data.inline_top_rect = Some((
            top_offset.0,
            top_offset.1,
            top_size.0 as f64,
            top_size.1 as f64,
        ));

        // Side toolbar
        let side_hover_local = self
            .data
            .inline_side_hover
            .map(|(x, y)| (x - side_offset.0, y - side_offset.1));
        let _ = ctx.save();
        ctx.translate(side_offset.0, side_offset.1);
        if let Err(err) = render_side_palette(
            ctx,
            side_size.0 as f64,
            side_size.1 as f64,
            snapshot,
            &mut self.data.inline_side_hits,
            side_hover_local,
        ) {
            log::warn!("Failed to render inline side toolbar: {}", err);
        }
        let _ = ctx.restore();
        for hit in &mut self.data.inline_side_hits {
            hit.rect.0 += side_offset.0;
            hit.rect.1 += side_offset.1;
        }
        self.data.inline_side_rect = Some((
            side_offset.0,
            side_offset.1,
            side_size.0 as f64,
            side_size.1 as f64,
        ));
    }

    fn inline_toolbar_hit_at(
        &self,
        position: (f64, f64),
    ) -> Option<(crate::backend::wayland::toolbar_intent::ToolbarIntent, bool)> {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return None;
        }
        self.data
            .inline_top_hits
            .iter()
            .chain(self.data.inline_side_hits.iter())
            .find_map(|hit| intent_for_hit(hit, position.0, position.1))
    }

    fn inline_toolbar_drag_at(
        &self,
        position: (f64, f64),
    ) -> Option<crate::backend::wayland::toolbar_intent::ToolbarIntent> {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return None;
        }
        // If we have an active move drag, generate intent directly from it
        // This allows dragging to continue even when mouse is outside the hit region
        if let Some(intent) = self.move_drag_intent(position.0, position.1) {
            return Some(intent);
        }
        self.data
            .inline_top_hits
            .iter()
            .chain(self.data.inline_side_hits.iter())
            .find_map(|hit| drag_intent_for_hit(hit, position.0, position.1))
    }

    /// Generate a drag intent from the active toolbar move drag state.
    /// This bypasses hit testing to allow dragging to continue when the mouse
    /// moves outside the original drag handle region.
    pub(super) fn move_drag_intent(
        &self,
        x: f64,
        y: f64,
    ) -> Option<crate::backend::wayland::toolbar_intent::ToolbarIntent> {
        use crate::backend::wayland::toolbar_intent::ToolbarIntent;
        use crate::ui::toolbar::ToolbarEvent;

        match self.data.toolbar_move_drag {
            Some(MoveDrag {
                kind: MoveDragKind::Top,
                ..
            }) => Some(ToolbarIntent(ToolbarEvent::MoveTopToolbar { x, y })),
            Some(MoveDrag {
                kind: MoveDragKind::Side,
                ..
            }) => Some(ToolbarIntent(ToolbarEvent::MoveSideToolbar { x, y })),
            None => None,
        }
    }

    /// Returns true if we're currently in a toolbar move drag operation.
    pub(super) fn is_move_dragging(&self) -> bool {
        self.data.toolbar_move_drag.is_some()
    }

    pub(super) fn active_move_drag_kind(&self) -> Option<MoveDragKind> {
        self.data.active_drag_kind
    }

    pub(super) fn inline_toolbar_motion(&mut self, position: (f64, f64)) -> bool {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }

        self.set_current_mouse(position.0 as i32, position.1 as i32);
        let (mx, my) = self.current_mouse();
        self.input_state.update_pointer_position(mx, my);

        let was_top_hover = self.data.inline_top_hover;
        let was_side_hover = self.data.inline_side_hover;

        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;

        let mut over_toolbar = false;

        if let Some((x, y, w, h)) = self.data.inline_top_rect
            && self.point_in_rect(position.0, position.1, x, y, w, h)
        {
            over_toolbar = true;
            self.data.inline_top_hover = Some(position);
        }

        if let Some((x, y, w, h)) = self.data.inline_side_rect
            && self.point_in_rect(position.0, position.1, x, y, w, h)
        {
            over_toolbar = true;
            self.data.inline_side_hover = Some(position);
        }

        if self.toolbar_dragging()
            && let Some(intent) = self.inline_toolbar_drag_at(position)
        {
            let evt = intent_to_event(intent, self.toolbar.last_snapshot());
            self.handle_toolbar_event(evt);
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            over_toolbar = true;
        } else if self.toolbar_dragging() {
            if let Some(kind) = self.active_move_drag_kind() {
                self.handle_toolbar_move(kind, position);
            }
            over_toolbar = true;
        }

        if was_top_hover != self.data.inline_top_hover
            || was_side_hover != self.data.inline_side_hover
        {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }

        if over_toolbar {
            self.set_pointer_over_toolbar(true);
        } else if !self.toolbar_dragging() {
            self.set_pointer_over_toolbar(false);
        }

        over_toolbar
    }

    pub(super) fn inline_toolbar_press(&mut self, position: (f64, f64)) -> bool {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }
        if let Some((intent, drag)) = self.inline_toolbar_hit_at(position) {
            self.set_toolbar_dragging(drag);
            let evt = intent_to_event(intent, self.toolbar.last_snapshot());
            self.handle_toolbar_event(evt);
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            self.set_pointer_over_toolbar(true);
            return true;
        }
        false
    }

    pub(super) fn inline_toolbar_leave(&mut self) {
        if !self.inline_toolbars_active() {
            return;
        }
        let had_hover =
            self.data.inline_top_hover.is_some() || self.data.inline_side_hover.is_some();
        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;
        self.set_pointer_over_toolbar(false);
        // Don't clear drag state if we're in a move drag - the drag continues outside
        if !self.is_move_dragging() {
            self.set_toolbar_dragging(false);
            self.end_toolbar_move_drag();
        }
        if had_hover {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }
    }

    pub(super) fn inline_toolbar_release(&mut self, position: (f64, f64)) -> bool {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }
        if self.pointer_over_toolbar() || self.toolbar_dragging() {
            if self.toolbar_dragging()
                && let Some(intent) = self.inline_toolbar_drag_at(position)
            {
                let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                self.handle_toolbar_event(evt);
                self.toolbar.mark_dirty();
                self.input_state.needs_redraw = true;
            }
            self.set_toolbar_dragging(false);
            self.set_pointer_over_toolbar(false);
            self.end_toolbar_move_drag();
            return true;
        }
        false
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
        let board_mode = self.input_state.board_mode();
        let suppressed = self.overlay_suppressed()
            && !(self.data.overlay_suppression == OverlaySuppression::Zoom
                && board_mode != BoardMode::Transparent);

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

        if !suppressed {
            let allow_background_image =
                !(self.zoom.is_engaged() && board_mode != BoardMode::Transparent);
            let zoom_render_image = if self.zoom.active && allow_background_image {
                self.zoom.image().or_else(|| self.frozen.image())
            } else {
                None
            };
            let zoom_render_active = self.zoom.active && zoom_render_image.is_some();
            let zoom_transform_active = self.zoom.active;
            let background_image = if zoom_render_active {
                zoom_render_image
            } else if allow_background_image {
                self.frozen.image()
            } else {
                None
            };

            if let Some(image) = background_image {
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
                if zoom_render_active {
                    let scale_x_safe = scale_x.max(f64::MIN_POSITIVE);
                    let scale_y_safe = scale_y.max(f64::MIN_POSITIVE);
                    let offset_x = self.zoom.view_offset.0 * (scale as f64) / scale_x_safe;
                    let offset_y = self.zoom.view_offset.1 * (scale as f64) / scale_y_safe;
                    ctx.scale(scale_x * self.zoom.scale, scale_y * self.zoom.scale);
                    ctx.translate(-offset_x, -offset_y);
                } else if (scale_x - 1.0).abs() > f64::EPSILON
                    || (scale_y - 1.0).abs() > f64::EPSILON
                {
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

            if zoom_transform_active {
                let _ = ctx.save();
                ctx.scale(self.zoom.scale, self.zoom.scale);
                ctx.translate(-self.zoom.view_offset.0, -self.zoom.view_offset.1);
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

            if let DrawingState::Drawing {
                tool: Tool::Eraser,
                points,
                ..
            } = &self.input_state.state
                && self.input_state.eraser_mode == EraserMode::Stroke
            {
                let radius = (self.input_state.eraser_size / 2.0).max(1.0);
                let ids = self.input_state.hit_test_all_for_points(points, radius);
                if !ids.is_empty() {
                    let hover_ids: HashSet<_> = ids.into_iter().collect();
                    let frame = self.input_state.canvas_set.active_frame();
                    for drawn in &frame.shapes {
                        if hover_ids.contains(&drawn.id) {
                            crate::draw::render_selection_halo(&ctx, drawn);
                        }
                    }
                }
            }

            // Render provisional shape if actively drawing
            // Use optimized method that avoids cloning for freehand
            let (mx, my) = if zoom_transform_active {
                self.zoomed_world_coords(
                    self.current_mouse().0 as f64,
                    self.current_mouse().1 as f64,
                )
            } else {
                self.current_mouse()
            };
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

            if zoom_transform_active {
                let _ = ctx.restore();
            }

            // Render frozen badge even if status bar is hidden
            if self.input_state.frozen_active()
                && !self.zoom.active
                && self.config.ui.show_frozen_badge
            {
                crate::ui::render_frozen_badge(&ctx, width, height);
            }
            // Render a zoom badge when the status bar is hidden or zoom is locked.
            if self.input_state.zoom_active()
                && (!self.input_state.show_status_bar || self.input_state.zoom_locked())
            {
                crate::ui::render_zoom_badge(
                    &ctx,
                    width,
                    height,
                    self.input_state.zoom_scale(),
                    self.input_state.zoom_locked(),
                );
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

            if !self.zoom.active {
                crate::ui::render_properties_panel(&ctx, &self.input_state, width, height);

                if self.input_state.is_context_menu_open() {
                    self.input_state
                        .update_context_menu_layout(&ctx, width, height);
                } else {
                    self.input_state.clear_context_menu_layout();
                }

                // Render context menu if open
                crate::ui::render_context_menu(&ctx, &self.input_state, width, height);
            } else {
                self.input_state.clear_context_menu_layout();
            }

            // Inline toolbars (xdg fallback) render directly into main surface when layer-shell is unavailable.
            if self.toolbar.is_visible() && self.inline_toolbars_active() {
                let snapshot = self.toolbar_snapshot();
                if self.toolbar.update_snapshot(&snapshot) {
                    self.toolbar.mark_dirty();
                }
                self.render_inline_toolbars(&ctx, &snapshot);
            }

            let _ = ctx.restore();
        }

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

        // Capture damage hints for diagnostics. We still apply full damage below to avoid missed
        // redraws, but logging the computed regions helps pinpoint under-reporting issues.
        let logical_damage = resolve_damage_regions(
            self.surface.width().min(i32::MAX as u32) as i32,
            self.surface.height().min(i32::MAX as u32) as i32,
            self.input_state.take_dirty_regions(),
        );
        if debug_damage_logging_enabled() {
            let scaled_damage = scale_damage_regions(logical_damage.clone(), scale);
            debug!(
                "Damage hints (scaled): count={}, {}",
                scaled_damage.len(),
                damage_summary(&scaled_damage)
            );
        }

        // Prefer correctness over micro-optimizations: full damage avoids cases where incomplete
        // hints result in stale pixels (reported as disappearing/reappearing strokes). If we ever
        // return to partial damage, implement per-buffer damage tracking instead of draining a
        // single accumulator.
        wl_surface.damage_buffer(0, 0, phys_width as i32, phys_height as i32);

        let force_frame_callback = self.frozen.preflight_pending() || self.zoom.preflight_pending();
        if self.config.performance.enable_vsync {
            debug!("Requesting frame callback (vsync enabled)");
            wl_surface.frame(qh, wl_surface.clone());
        } else if force_frame_callback {
            debug!("Requesting frame callback (preflight)");
            wl_surface.frame(qh, wl_surface.clone());
            self.surface.set_frame_callback_pending(true);
        } else {
            debug!("Skipping frame callback (vsync disabled - allows back-to-back renders)");
        }

        wl_surface.commit();
        debug!("=== RENDER COMPLETE ===");

        // Render toolbar overlays if visible, only when state/hover changed.
        if self.toolbar.is_visible() && !self.inline_toolbars_active() {
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
        match event {
            ToolbarEvent::MoveTopToolbar { x, y } => {
                self.begin_toolbar_move_drag(MoveDragKind::Top, (x, y));
                self.handle_toolbar_move(MoveDragKind::Top, (x, y));
                return;
            }
            ToolbarEvent::MoveSideToolbar { x, y } => {
                self.begin_toolbar_move_drag(MoveDragKind::Side, (x, y));
                self.handle_toolbar_move(MoveDragKind::Side, (x, y));
                return;
            }
            _ => {}
        }

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
                | ToolbarEvent::SetMarkerOpacity(_)
                | ToolbarEvent::SetEraserMode(_)
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
        self.config.ui.toolbar.top_offset = self.data.toolbar_top_offset;
        self.config.ui.toolbar.top_offset_y = self.data.toolbar_top_offset_y;
        self.config.ui.toolbar.side_offset = self.data.toolbar_side_offset;
        self.config.ui.toolbar.side_offset_x = self.data.toolbar_side_offset_x;
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
        self.config.drawing.default_eraser_mode = self.input_state.eraser_mode;
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
        self.exit_overlay_suppression(OverlaySuppression::Capture);
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

        // Suppress overlay before capture to prevent capturing the overlay itself
        self.enter_overlay_suppression(OverlaySuppression::Capture);
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

#[allow(dead_code)]
fn resolve_damage_regions(width: i32, height: i32, mut regions: Vec<Rect>) -> Vec<Rect> {
    regions.retain(Rect::is_valid);

    if regions.is_empty()
        && width > 0
        && height > 0
        && let Some(full) = Rect::new(0, 0, width, height)
    {
        regions.push(full);
    }

    regions
}

#[allow(dead_code)]
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

fn damage_summary(regions: &[Rect]) -> String {
    if regions.is_empty() {
        return "[]".to_string();
    }

    let mut parts = Vec::with_capacity(regions.len());
    for r in regions.iter().take(5) {
        parts.push(format!("({},{}) {}x{}", r.x, r.y, r.width, r.height));
    }
    if regions.len() > 5 {
        parts.push(format!("... +{} more", regions.len() - 5));
    }
    parts.join(", ")
}

fn parse_boolish_env(raw: &str) -> bool {
    let v = raw.to_ascii_lowercase();
    !(v.is_empty() || v == "0" || v == "false" || v == "off")
}

fn parse_debug_damage_env(raw: &str) -> bool {
    parse_boolish_env(raw)
}

fn debug_damage_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_debug_damage_env(&std::env::var("WAYSCRIBER_DEBUG_DAMAGE").unwrap_or_default())
    })
}

pub(super) fn surface_id(surface: &wl_surface::WlSurface) -> u32 {
    surface.id().protocol_id()
}

pub(super) fn debug_toolbar_drag_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_boolish_env(&std::env::var("WAYSCRIBER_DEBUG_TOOLBAR_DRAG").unwrap_or_default())
    })
}

fn drag_log(message: impl AsRef<str>) {
    if debug_toolbar_drag_logging_enabled() {
        log::info!("{}", message.as_ref());
    }
}

fn force_inline_env_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_boolish_env(&std::env::var("WAYSCRIBER_FORCE_INLINE_TOOLBARS").unwrap_or_default())
    })
}

fn force_inline_toolbars_requested(config: &Config) -> bool {
    config.ui.toolbar.force_inline || force_inline_env_enabled()
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

    // NOTE: The functions below are used for diagnostic logging only; the renderer currently applies
    // full-surface damage for correctness. These tests document the intended behavior if we ever
    // reintroduce partial damage handling.
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

    #[test]
    fn full_damage_policy_is_explicit() {
        // This documents that we intentionally call damage_buffer over the full surface to avoid
        // stale pixels with buffer reuse. If you switch back to partial damage, implement
        // per-buffer damage tracking instead of draining a single accumulator.
    }

    #[test]
    fn scale_damage_regions_multiplies_by_scale() {
        let regions = vec![Rect {
            x: 2,
            y: 3,
            width: 4,
            height: 5,
        }];
        let scaled = scale_damage_regions(regions, 2);
        assert_eq!(scaled.len(), 1);
        assert_eq!(scaled[0], Rect::new(4, 6, 8, 10).unwrap());
    }

    #[test]
    fn debug_damage_logging_env_parses_falsey() {
        assert!(!parse_debug_damage_env(""));
        assert!(!parse_debug_damage_env("0"));
        assert!(!parse_debug_damage_env("false"));
        assert!(!parse_debug_damage_env("off"));
        assert!(parse_debug_damage_env("1"));
        assert!(parse_debug_damage_env("true"));
    }
}
