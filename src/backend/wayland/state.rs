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
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use wayland_client::{
    Proxy, QueueHandle,
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface, wl_touch},
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
    text_input::zv3::client::{
        zwp_text_input_manager_v3::ZwpTextInputManagerV3, zwp_text_input_v3::ZwpTextInputV3,
    },
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
        ImageFormatMetadata, ImageOperationKind, RenderedDocument,
        file::{FileSaveConfig, expand_tilde},
        types::CaptureType,
    },
    config::{Action, Config},
    desktop_open::DesktopOpenRequest,
    input::state::{ClipboardPasteRequest, TextClipboardRequest, TextPasteTarget},
    input::{DrawingState, EraserMode, InputState, Key, Tool, ZoomAction},
    session::SessionOptions,
    ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot},
};

use self::data::{MoveDrag, StateData};
pub use self::data::{
    MoveDragKind, OverlaySuppression, OverlaySuppressionKeyboardPolicy, XdgFrozenFullscreenState,
};
pub(in crate::backend::wayland) use self::region_capture::WindowSnapDirection;
use super::{
    RuntimeOperationController, RuntimeOperationIdSource,
    capture::{CapturePreflightRequest, CaptureState, PendingPdfExport},
    clipboard::{ClipboardPasteCompletion, ClipboardPublishCompletion},
    frozen::{ExtImageCopyManagers, FrozenState},
    overlay_passthrough::set_surface_clickthrough,
    session::SessionState,
    surface::SurfaceState,
    toolbar::{
        ToolbarSurfaceManager,
        hit::{drag_intent_for_hit, intent_for_hit, quick_color_slot_for_hit},
        layout::top_size,
        render::render_top_strip,
    },
    toolbar_intent::intent_to_event,
    zoom::ZoomState,
};

mod acquisition;
mod activation;
mod boards;
mod buffer_damage;
mod canvas_layer;
mod capture;
mod clipboard;
mod color_picker;
mod core;
mod data;
mod desktop_open;
mod eyedropper;
mod gtk_toolbar;
mod helpers;
mod input_actions;
mod input_hud;
mod keybindings;
pub(in crate::backend::wayland) use keybindings::queue_keybinding_edit;
mod marker_snap;
mod ocr;
mod onboarding;
mod pdf_export;
mod perf;
mod region_capture;
pub(in crate::backend::wayland) use region_capture::RegionCaptureIntent;
#[cfg(test)]
pub(in crate::backend::wayland) use region_capture::RegionPickerOptions;
mod render;
mod screen_image;
mod text_clipboard;
mod toolbar;
#[cfg(feature = "toolbar-gtk")]
pub(crate) use toolbar::clamp_floating_axis_offset;
pub(in crate::backend::wayland) use toolbar::{queue_preset_action, queue_quick_color_edit};
mod zoom;

#[cfg(test)]
mod tests;

type ScreencopyManager = wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

pub(in crate::backend::wayland) use self::buffer_damage::FullDamageReason;
pub(in crate::backend::wayland) use self::perf::{
    PerfDamageDiagnostics, PerfFrameDamageContext, PerfInputSource, PerfRenderBreakdown,
    PerfRenderProfileKind, PerfRenderSkipReason, damage_covers_logical_surface,
};
pub(in crate::backend::wayland) use self::render::RenderOutcome;
pub(super) use helpers::{
    damage_summary, debug_damage_logging_enabled, debug_toolbar_drag_logging_enabled, drag_log,
    force_inline_toolbars_requested, scale_damage_regions, surface_id,
    toolbar_drag_preview_enabled, toolbar_drag_throttle_interval, toolbar_pointer_lock_enabled,
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
    pub palette_recents: crate::palette_recents::PaletteRecentsWriter,
    pub capture_manager: CaptureManager,
    pub session_options: Option<SessionOptions>,
    pub session_config_failed: bool,
    pub persistence: crate::backend::wayland::session::PersistenceController,
    pub runtime_ui: Option<crate::backend::wayland::runtime_ui_state::ToolbarRuntimeState>,
    pub runtime_ui_unavailable: Option<crate::ui::toolbar::RuntimeUiPersistenceSnapshot>,
    pub runtime_wake: crate::backend::wayland::RuntimeWakeHandle,
    pub tokio_handle: tokio::runtime::Handle,
    pub exit_after_capture_mode: ExitAfterCaptureMode,
    pub frozen_enabled: bool,
    pub preferred_output_identity: Option<String>,
    pub xdg_fullscreen: bool,
    pub main_surface_uses_overlay_layer: bool,
    pub pending_freeze_on_start: bool,
    pub screencopy_manager: Option<ScreencopyManager>,
    pub ext_image_copy_managers: Option<ExtImageCopyManagers>,
    pub portal_freeze_supported: bool,
    pub text_input_manager: Option<ZwpTextInputManagerV3>,
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
    // Both drive pointer lock for toolbar drags; see
    // state/toolbar/visibility/pointer.rs.
    pub(super) pointer_constraints_state: PointerConstraintsState,
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
    /// Whether the frame just rendered carried a spotlight dim layer.
    ///
    /// Removing the last spotlight makes `has_spotlight()` false, but the buffer
    /// on screen still holds the full-screen dim. One more full-damage frame is
    /// needed to wash it out, so the decision looks at the previous frame too.
    pub(super) spotlight_dimmed_last_frame: bool,
    /// Reused bounded Cairo snapshots for Spotlight loupe rendering.
    pub(super) spotlight_magnifier_scratch: crate::draw::SpotlightMagnifierScratch,
    /// Deduplicates the live render-failure warning.
    pub(super) spotlight_magnifier_warning_active: bool,
    /// When an in-flight wheel adjustment of a loupe stops counting as one
    /// burst. Discrete wheels send no end-of-gesture signal, so the boundary
    /// is a quiet period: without it two visits minutes apart, at unchanged
    /// coordinates, would merge into a single undo entry.
    pub(super) spotlight_wheel_idle_deadline: Option<std::time::Instant>,
    /// Availability that the standing "this page cannot magnify" warning was
    /// last shown for, so loading or switching pages warns once rather than
    /// once per frame, and warns again once availability changes.
    pub(super) spotlight_magnifier_page_warned_source:
        Option<crate::draw::SpotlightMagnifierSource>,

    // Configuration
    pub(super) config: Config,
    /// Authored `[session]` settings were unavailable and the live options are
    /// defaults. Destructive session actions must refuse to use those options.
    pub(super) session_config_failed: bool,
    pub(super) runtime_ui: Option<crate::backend::wayland::runtime_ui_state::ToolbarRuntimeState>,
    pub(super) runtime_ui_unavailable: Option<crate::ui::toolbar::RuntimeUiPersistenceSnapshot>,
    pub(super) runtime_ui_unavailable_previews:
        crate::backend::wayland::runtime_ui_state::UnavailablePersistencePreviews,

    // Input state
    pub(super) input_state: InputState,
    /// Wake handle the input HUD's system reader pokes after sending chips.
    /// Cloned from the shared runtime source at startup so the reader can be
    /// started and stopped whenever the HUD toggles.
    #[cfg(feature = "input-monitor")]
    pub(super) input_monitor_wake: crate::backend::wayland::RuntimeWakeHandle,
    /// Live system-wide input reader (`Some` only while the HUD runs in system
    /// mode); dropping it stops the thread.
    #[cfg(feature = "input-monitor")]
    pub(super) input_monitor: Option<crate::backend::wayland::input_monitor::InputMonitor>,
    /// Latch for the "system capture unavailable" guidance: one warning per
    /// denied episode, whether the request came from startup config, a
    /// toggle, or a mode change. Reset when system capture starts or stops
    /// being requested.
    pub(super) input_hud_system_warned: bool,
    /// A runtime enable is waiting to announce which source it got. Held
    /// across the reader thread's readiness handshake, so the toast names the
    /// source the HUD actually ended up with.
    pub(super) input_hud_announce_pending: bool,
    /// The HUD request the reader thread was last reconciled against.
    ///
    /// Every path that can move the HUD has to reach `sync_input_monitor`, and
    /// they do not all run through one handler: a command-palette entry
    /// clicked with the mouse, and a rollback undoing a rejected write, both
    /// change the flag somewhere else entirely. The event loop compares this
    /// against the live request each pass instead of asking each of them to
    /// remember, so the reader can never be left running for a HUD that is
    /// already off.
    pub(super) last_input_hud_request: Option<(bool, crate::config::InputHudMode)>,
    pub(super) clipboard_publish: RuntimeOperationController<u64, ClipboardPublishCompletion>,
    pub(super) clipboard_paste:
        RuntimeOperationController<ClipboardPasteRequest, ClipboardPasteCompletion>,
    pub(super) clipboard_hex_copy: RuntimeOperationController<String, Result<(), String>>,
    /// Desktop-open work completes off-dispatch; successful completion is what
    /// requests overlay exit, so runtime-owned broker teardown cannot race it.
    pub(super) desktop_open: RuntimeOperationController<DesktopOpenRequest, Result<(), String>>,
    pub(super) pending_hex_copy: Option<String>,
    /// Async wl-copy pipeline for text-editor selections (Ctrl+C / Ctrl+X).
    pub(super) clipboard_text_copy:
        RuntimeOperationController<TextClipboardRequest, Result<(), String>>,
    pub(super) pending_text_copy: VecDeque<TextClipboardRequest>,
    /// Async wl-paste pipeline for text-editor paste requests (Ctrl+V).
    pub(super) clipboard_text_paste:
        RuntimeOperationController<TextPasteTarget, Result<Option<String>, String>>,
    /// Capacity-one compositor window query for the current native region picker.
    /// Its context owns the picker/source correlation, so stale workers cannot
    /// mutate a later picker generation.
    pub(super) window_query: RuntimeOperationController<
        region_capture::WindowSnapQuery,
        Result<
            crate::capture::window_geometry::WindowQueryResult,
            crate::capture::window_geometry::WindowGeometryError,
        >,
    >,
    /// Text paste requests waiting behind an active read. Repeated requests in
    /// the current edit generation remain distinct; a new generation replaces
    /// stale queued requests from the old edit session.
    pub(super) pending_text_paste: VecDeque<TextPasteTarget>,
    /// Capacity-one screen text recognition. A busy controller reports
    /// busy rather than queuing a region the user has moved on from.
    pub(super) ocr: crate::ocr::OcrController,
    /// Capacity-one text-row layout scans for the marker's snap mode. Separate
    /// from `ocr` so a scan the user did not ask for can never make
    /// `Copy text from screen` wait behind it.
    pub(super) text_layout: crate::ocr::TextLayoutController,
    /// GTK toolbar frontend; `None` means the built-in bars are in charge.
    pub(super) gtk_toolbar: Option<crate::toolbar_gtk::GtkToolbarBridge>,
    pub(super) onboarding: crate::onboarding::OnboardingStore,
    /// Background persistence worker for command-palette recents.
    pub(super) palette_recents: crate::palette_recents::PaletteRecentsWriter,
    /// Off-dispatch writer for the three explicit `config.toml` edit gestures.
    pub(super) config_edits: crate::backend::wayland::config_edits::ConfigEditWorker,
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

    // Manual key repeat. The keyboard is created without sctk's calloop-based
    // repeat (this loop is a manual poll), so `repeat_key` never fires; this
    // synthesizes repeats for the held key from the event loop instead.
    pub(super) key_repeat_key: Option<Key>,
    pub(super) key_repeat_next_tick: Option<Instant>,

    // IME / text-input-v3. The manager is bound at startup; one seat-bound
    // `text_input` object is created for the first keyboard seat.
    // `focused` tracks compositor text-input focus (Enter/Leave); `enabled`
    // tracks whether we have an active enable() outstanding; `serial` counts
    // our commit() calls to match against Done events.
    pub(super) text_input_manager: Option<ZwpTextInputManagerV3>,
    pub(super) text_input: Option<ZwpTextInputV3>,
    pub(super) text_input_seat: Option<wl_seat::WlSeat>,
    pub(super) text_input_focused: bool,
    pub(super) text_input_enabled: bool,
    pub(super) text_input_serial: u32,
    /// A visible editor change received against a stale `done` serial. The
    /// resulting cursor rectangle must be published after a matching `done`.
    pub(super) text_input_cursor_update_pending: bool,
    /// The pending cursor/surrounding-text update was authored outside the
    /// input method and must carry text-input-v3's `Other` change cause.
    pub(super) text_input_external_change_pending: bool,
    /// Commit serial that must be acknowledged before another cursor-only
    /// update is sent. Set only after observing a stale `done`.
    pub(super) text_input_cursor_update_blocked_until: Option<u32>,

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
    /// The pen is still physically down, but the contact was disowned when a
    /// screen-region modal took over. Wayscriber's own flags were cleared, yet
    /// the compositor keeps reporting this contact until the tip lifts — so it
    /// is tracked separately and ignored until then, rather than being mistaken
    /// for a fresh press made during the wait.
    #[cfg(feature = "tablet-input")]
    pub(super) stylus_contact_retired: bool,
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
    const INLINE_TOP_Y: f64 = Self::TOP_BASE_MARGIN_TOP;
    const INLINE_TOP_X: f64 = 24.0;
    const TOOLBAR_CONFIGURE_FAIL_THRESHOLD: u32 = 180;
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
