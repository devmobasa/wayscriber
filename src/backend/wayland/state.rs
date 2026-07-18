// Holds the live Wayland protocol state shared by the backend loop and the handler
// submodules; provides rendering, capture routing, and overlay helpers used across them.
use anyhow::{Context, Result};
use log::{debug, info, warn};
use smithay_client_toolkit::seat::pointer::CursorIcon;
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
use std::time::{Duration, Instant};
use wayland_client::{
    Proxy, QueueHandle,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface, wl_touch},
};
#[cfg(feature = "tablet-input")]
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

#[cfg(feature = "tablet-input")]
use crate::input::tablet::TabletSettings;
use crate::{
    backend::ExitAfterCaptureMode,
    canvas_export::{
        BoardExportSnapshot, BoardPdfExportSnapshot, CanvasExportBackdropSnapshot,
        CanvasExportSnapshot, CanvasExportViewport, render_board_pdf, render_canvas_png,
    },
    capture::{
        CaptureDestination, CaptureManager, DesktopBackdropCaptureRequest,
        DesktopBackdropCaptureResult, DesktopBackdropGeometry, DesktopBackdropOutputGeometry,
        DocumentDeliveryRequest, ImageDeliveryRequest, ImageFormatMetadata, ImageOperationKind,
        RenderedDocument,
        file::{FileSaveConfig, expand_tilde},
        types::CaptureType,
    },
    config::{Action, Config},
    input::state::ClipboardPasteRequest,
    input::{DrawingState, EraserMode, InputState, Tool, ZoomAction},
    session::SessionOptions,
    ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot},
};

use self::data::{MoveDrag, StateData};
pub use self::data::{MoveDragKind, OverlaySuppression, XdgFrozenFullscreenState};
use super::{
    capture::{CapturePreflightRequest, CaptureState, PendingPdfExport},
    clipboard::{
        ClipboardOperationController, ClipboardOperationIdSource, ClipboardPasteCompletion,
        ClipboardPublishCompletion,
    },
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

mod activation;
mod boards;
mod buffer_damage;
mod canvas_layer;
mod capture;
mod clipboard;
mod color_picker;
mod core;
mod data;
mod eyedropper;
mod gtk_toolbar;
mod helpers;
mod input_actions;
mod keybindings;
mod onboarding;
mod pdf_export;
mod perf;
mod render;
mod toolbar;
#[cfg(feature = "toolbar-gtk")]
pub(crate) use toolbar::clamp_floating_axis_offset;
mod zoom;

#[cfg(test)]
mod tests;

type ScreencopyManager = wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

pub(in crate::backend::wayland) use self::buffer_damage::FullDamageReason;
pub(in crate::backend::wayland) use self::perf::{
    PerfDamageDiagnostics, PerfFrameDamageContext, PerfInputSource, PerfRenderBreakdown,
    PerfRenderProfileKind, PerfRenderSkipReason, damage_covers_logical_surface,
};
pub(super) use helpers::{
    color_log, damage_summary, debug_damage_logging_enabled, debug_toolbar_color_logging_enabled,
    debug_toolbar_drag_logging_enabled, drag_log, force_inline_toolbars_requested,
    scale_damage_regions, surface_id, toolbar_drag_preview_enabled, toolbar_drag_throttle_interval,
    toolbar_pointer_lock_enabled,
};

pub(in crate::backend::wayland) struct WaylandGlobals {
    pub registry_state: RegistryState,
    pub compositor_state: CompositorState,
    pub layer_shell: Option<LayerShell>,
    pub xdg_shell: Option<XdgShell>,
    pub activation: Option<ActivationState>,
    pub shm: Shm,
    pub pointer_constraints_state: PointerConstraintsState,
    pub relative_pointer_state: RelativePointerState,
    pub output_state: OutputState,
    pub seat_state: SeatState,
}

pub(in crate::backend::wayland) struct WaylandStateInit {
    pub globals: WaylandGlobals,
    pub config: Config,
    pub input_state: InputState,
    pub onboarding: crate::onboarding::OnboardingStore,
    pub capture_manager: CaptureManager,
    pub session_options: Option<SessionOptions>,
    pub persistence: crate::backend::wayland::session::PersistenceController,
    pub runtime_wake: crate::backend::wayland::RuntimeWakeHandle,
    pub tokio_handle: tokio::runtime::Handle,
    pub exit_after_capture_mode: ExitAfterCaptureMode,
    pub frozen_enabled: bool,
    pub preferred_output_identity: Option<String>,
    pub xdg_fullscreen: bool,
    pub main_surface_uses_overlay_layer: bool,
    pub pending_freeze_on_start: bool,
    pub screencopy_manager: Option<ScreencopyManager>,
    #[cfg(feature = "tablet-input")]
    pub tablet_manager: Option<ZwpTabletManagerV2>,
}

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
    /// Per-buffer damage tracking for correct incremental rendering.
    pub(super) buffer_damage: buffer_damage::BufferDamageTracker,
    /// Baked committed-shapes layer for panned canvas rendering.
    pub(super) canvas_layer_cache: canvas_layer::CanvasLayerCache,

    // Configuration
    pub(super) config: Config,

    // Input state
    pub(super) input_state: InputState,
    pub(super) clipboard_publish: ClipboardOperationController<u64, ClipboardPublishCompletion>,
    pub(super) clipboard_paste:
        ClipboardOperationController<ClipboardPasteRequest, ClipboardPasteCompletion>,
    pub(super) clipboard_hex_copy: ClipboardOperationController<String, Result<(), String>>,
    pub(super) pending_hex_copy: Option<String>,
    /// GTK toolbar frontend; `None` means the built-in bars are in charge.
    pub(super) gtk_toolbar: Option<crate::toolbar_gtk::GtkToolbarBridge>,
    pub(super) onboarding: crate::onboarding::OnboardingStore,
    // Next scheduled tick for UI animations (toasts/highlights/preset feedback).
    pub(super) ui_animation_next_tick: Option<Instant>,
    // Animation interval; None means uncapped (render every frame while active).
    pub(super) ui_animation_interval: Option<Duration>,

    // Capture manager
    pub(super) capture: CaptureState,
    pub(super) frozen: FrozenState,
    pub(super) zoom: ZoomState,
    perf: perf::PerfMetrics,

    // Overlay behavior
    pub(super) exit_after_capture_mode: ExitAfterCaptureMode,

    // Pointer cursor
    pub(super) themed_pointer: Option<ThemedPointer<PointerData>>,
    #[allow(dead_code)] // Retains the WlTouch protocol object while the seat advertises touch.
    pub(super) touch: Option<wl_touch::WlTouch>,
    pub(super) active_touch: TouchState,
    pub(super) active_touch_surface: Option<wl_surface::WlSurface>,
    pub(super) locked_pointer: Option<ZwpLockedPointerV1>,
    pub(super) current_pointer_shape: Option<CursorIcon>,
    pub(super) relative_pointer: Option<ZwpRelativePointerV1>,
    pub(super) cursor_hidden: bool,

    // Tablet / stylus (feature-gated)
    #[cfg(feature = "tablet-input")]
    pub(super) tablet_manager: Option<ZwpTabletManagerV2>,
    #[cfg(feature = "tablet-input")]
    pub(super) tablet_seats: Vec<ZwpTabletSeatV2>,
    #[cfg(feature = "tablet-input")]
    pub(super) tablets: Vec<ZwpTabletV2>,
    #[cfg(feature = "tablet-input")]
    pub(super) tablet_tools: Vec<ZwpTabletToolV2>,
    #[cfg(feature = "tablet-input")]
    pub(super) tablet_pads: Vec<ZwpTabletPadV2>,
    #[cfg(feature = "tablet-input")]
    pub(super) tablet_pad_groups: Vec<ZwpTabletPadGroupV2>,
    #[cfg(feature = "tablet-input")]
    pub(super) tablet_pad_rings: Vec<ZwpTabletPadRingV2>,
    #[cfg(feature = "tablet-input")]
    pub(super) tablet_pad_strips: Vec<ZwpTabletPadStripV2>,
    #[cfg(feature = "tablet-input")]
    pub(super) tablet_settings: TabletSettings,
    #[cfg(feature = "tablet-input")]
    pub(super) tablet_found_logged: bool,
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_tip_down: bool,
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_on_overlay: bool,
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_on_toolbar: bool,
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_base_thickness: Option<f64>,
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_pressure_thickness: Option<f64>,
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_surface: Option<wl_surface::WlSurface>,
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_last_pos: Option<(f64, f64)>,
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_peak_thickness: Option<f64>,
    #[cfg(feature = "tablet-input")]
    pub(super) pending_stylus_frame: PendingStylusFrame,
    /// Map of tool object IDs to their physical types (pen, eraser, etc.)
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_tool_types: std::collections::HashMap<
        wayland_client::backend::ObjectId,
        crate::backend::wayland::TabletToolType,
    >,
    /// Whether we auto-switched to eraser (if true, restore previous tool on proximity out)
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_auto_switched_to_eraser: bool,
    /// Tool override that was active before auto-switching to eraser
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_pre_eraser_tool_override: Option<crate::input::Tool>,

    // Session persistence
    pub(super) session: SessionState,
    pub(super) persistence: crate::backend::wayland::session::PersistenceController,
    session_dialog: self::toolbar::SessionFileDialogController,
    pub(super) durable_action_finish: Option<crate::daemon::protocol_v2::ClaimedAction>,
    pub(super) durable_action_retry_at: Option<Instant>,

    // Tokio runtime handle for async operations
    pub(super) tokio_handle: tokio::runtime::Handle,
}

#[cfg(feature = "tablet-input")]
#[derive(Clone, Debug, Default)]
pub(super) struct PendingStylusFrame {
    pub(super) motion: Option<(f64, f64)>,
    pub(super) pressure: Option<u32>,
    pub(super) down: bool,
    pub(super) up: bool,
    pub(super) button_presses: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum TouchTarget {
    #[default]
    None,
    Overlay,
    Toolbar,
    InlineToolbar,
    Other,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TouchState {
    active_id: Option<i32>,
    target: TouchTarget,
    last_position: Option<(f64, f64)>,
}

impl TouchState {
    pub(super) fn begin(&mut self, id: i32, position: (f64, f64)) -> bool {
        if self.active_id.is_some() {
            return false;
        }
        self.active_id = Some(id);
        self.target = TouchTarget::None;
        self.last_position = Some(position);
        true
    }

    pub(super) fn update_position(&mut self, id: i32, position: (f64, f64)) -> bool {
        if self.active_id != Some(id) {
            return false;
        }
        self.last_position = Some(position);
        true
    }

    pub(super) fn is_active_id(&self, id: i32) -> bool {
        self.active_id == Some(id)
    }

    pub(super) fn is_active(&self) -> bool {
        self.active_id.is_some()
    }

    pub(super) fn set_target(&mut self, target: TouchTarget) {
        if self.active_id.is_some() {
            self.target = target;
        }
    }

    pub(super) fn target(&self) -> TouchTarget {
        self.target
    }

    pub(super) fn last_position(&self) -> Option<(f64, f64)> {
        self.last_position
    }

    pub(super) fn clear(&mut self) {
        self.active_id = None;
        self.target = TouchTarget::None;
        self.last_position = None;
    }
}

#[cfg(feature = "tablet-input")]
impl PendingStylusFrame {
    pub(super) fn is_empty(&self) -> bool {
        self.motion.is_none()
            && self.pressure.is_none()
            && !self.down
            && !self.up
            && self.button_presses.is_empty()
    }
}

impl WaylandState {
    fn ui_animation_interval_from_fps(fps: u32) -> Option<Duration> {
        if fps == 0 {
            None
        } else {
            Some(Duration::from_secs_f64(1.0 / fps as f64))
        }
    }

    const TOP_MARGIN_RIGHT: f64 = 12.0;
    const TOP_BASE_MARGIN_TOP: f64 = 12.0;
    const TOP_MARGIN_BOTTOM: f64 = 0.0;
    const SIDE_BASE_MARGIN_TOP: f64 = 24.0;
    const SIDE_MARGIN_BOTTOM: f64 = 24.0;
    const SIDE_BASE_MARGIN_LEFT: f64 = 24.0;
    const SIDE_MARGIN_RIGHT: f64 = 0.0;
    const INLINE_TOP_Y: f64 = Self::TOP_BASE_MARGIN_TOP;
    const INLINE_SIDE_X: f64 = 24.0;
    const TOOLBAR_CONFIGURE_FAIL_THRESHOLD: u32 = 180;
    const INLINE_TOP_PUSH: f64 = 16.0;
    const ZOOM_STEP_KEY: f64 = 1.2;
    const ZOOM_STEP_SCROLL: f64 = 1.1;
    pub(super) const ZOOM_PAN_STEP: f64 = 32.0;
    pub(super) const ZOOM_PAN_STEP_LARGE: f64 = 96.0;
}

impl WaylandState {
    pub(super) fn update_ui_animation_tick(&mut self, now: Instant, active: bool) {
        if !active {
            self.ui_animation_next_tick = None;
            return;
        }
        if let Some(interval) = self.ui_animation_interval {
            self.ui_animation_next_tick = Some(now + interval);
        } else {
            self.ui_animation_next_tick = None;
        }
    }

    pub(super) fn ui_animation_timeout(&self, now: Instant) -> Option<Duration> {
        self.ui_animation_interval?;
        self.ui_animation_next_tick
            .map(|next| next.saturating_duration_since(now))
    }

    pub(super) fn ui_animation_due(&self, now: Instant) -> bool {
        if self.ui_animation_interval.is_none() {
            return false;
        }
        self.ui_animation_next_tick.is_some_and(|next| now >= next)
    }
}
