// Holds the live Wayland protocol state shared by the backend loop and the handler
// submodules; provides rendering, capture routing, and overlay helpers used across them.
use anyhow::{Context, Result};
use log::{debug, info, warn};
use smithay_client_toolkit::{
    activation::{ActivationHandler, RequestData},
    globals::ProvidesBoundGlobal,
    shell::wlr_layer::KeyboardInteractivity,
};
use std::time::{Duration, Instant};
use wayland_client::{QueueHandle, protocol::wl_output};
#[cfg(feature = "tablet-input")]
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_manager_v2::ZwpTabletManagerV2;
use wayland_protocols::wp::{
    pointer_constraints::zv1::client::zwp_pointer_constraints_v1,
    text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3,
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
    input::{DrawingState, EraserMode, InputState, Tool, ZoomAction},
    session::SessionOptions,
    ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot},
};

use self::data::StateData;
pub use self::data::{
    MoveDragKind, OverlaySuppression, OverlaySuppressionKeyboardPolicy, XdgFrozenFullscreenState,
};
pub(in crate::backend::wayland) use self::region_capture::WindowSnapDirection;
use super::{
    RuntimeOperationController, RuntimeOperationIdSource,
    capture::{CapturePreflightRequest, CaptureState, PendingPdfExport},
    frozen::{ExtImageCopyManagers, FrozenState},
    overlay_passthrough::set_surface_clickthrough,
    session::SessionState,
    surface::SurfaceState,
    toolbar::{ToolbarSurfaceManager, layout::top_size, render::render_top_strip},
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
mod clipboard_runtime;
pub(in crate::backend::wayland) use clipboard_runtime::{
    HexCopyOutcome, TextCopyOutcome, TextPasteOutcome,
};
mod color_picker;
mod core;
mod data;
mod desktop_open;
mod eyedropper;
mod focus;
mod font_catalog;
mod gtk_toolbar;
mod helper_launch;
mod helpers;
mod input_actions;
mod input_hud;
mod key_repeat;
mod keybindings;
pub(in crate::backend::wayland) use keybindings::queue_keybinding_edit;
mod ocr;
mod onboarding;
mod pdf_export;
mod perf;
mod pointer_runtime;
pub(in crate::backend::wayland) use pointer_runtime::TouchTarget;
mod preference_stores;
mod protocol_globals;
pub(in crate::backend::wayland) use protocol_globals::{ProtocolGlobals, ProtocolGlobalsSeed};
mod region_capture;
pub(in crate::backend::wayland) use region_capture::RegionCaptureIntent;
#[cfg(test)]
pub(in crate::backend::wayland) use region_capture::RegionPickerOptions;
pub(in crate::backend::wayland) use region_capture::RegionReviewPress;
mod render;
mod screen_image;
mod spotlight_runtime;
#[cfg(feature = "tablet-input")]
mod tablet_runtime;
mod text_clipboard;
mod text_input;
mod toolbar;
mod ui_animation;
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

pub(in crate::backend::wayland) struct WaylandStateInit {
    pub globals: ProtocolGlobals,
    pub config: Config,
    pub input_state: InputState,
    pub startup_activation_token: Option<String>,
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
    // Bound Wayland globals and toolkit handler state.
    pub(super) protocol: ProtocolGlobals,

    // Surface and buffer management
    pub(super) surface: SurfaceState,
    pub(super) toolbar: ToolbarSurfaceManager,
    pub(super) toolbar_chrome: toolbar::ToolbarChrome,
    pub(super) toolbar_drag: toolbar::ToolbarDrag,
    data: StateData,
    /// Keyboard, pointer, activation, and focus-loss lifecycle.
    pub(super) focus: focus::FocusState,
    /// Per-buffer damage tracking for correct incremental rendering.
    pub(super) buffer_damage: buffer_damage::BufferDamageTracker,
    /// Baked committed-shapes layer for panned canvas rendering.
    pub(super) canvas_layer_cache: canvas_layer::CanvasLayerCache,
    /// Render memory, warning latches, and wheel timing for Spotlight effects.
    pub(super) spotlight: spotlight_runtime::SpotlightRuntime,

    // Configuration
    pub(super) config: Config,
    /// Authored `[session]` settings were unavailable and the live options are
    /// defaults. Destructive session actions must refuse to use those options.
    pub(super) session_config_failed: bool,
    /// Durable UI preference stores and their background writers.
    pub(super) preferences: preference_stores::PreferenceStores,

    // Input state
    pub(super) input_state: InputState,
    /// One-shot worker that enumerates the system font catalog after the first
    /// committed frame instead of inside a picker-opening input callback.
    pub(super) font_catalog: font_catalog::FontCatalogPrewarm,
    /// System-reader lifecycle and reconciliation latches for the input HUD.
    pub(super) input_hud: input_hud::InputHudRuntime,
    pub(super) clipboard: clipboard_runtime::ClipboardRuntime,
    /// Desktop-open work completes off-dispatch; successful completion is what
    /// requests overlay exit, so runtime-owned broker teardown cannot race it.
    pub(super) desktop_open: RuntimeOperationController<DesktopOpenRequest, Result<(), String>>,
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
    /// Capacity-one Review cut preview. Independent of capture delivery so a
    /// replaceable preview cannot occupy the capture reservation slot.
    pub(super) region_cut_preview: RuntimeOperationController<
        region_capture::CutPreviewKey,
        region_capture::CutPreviewOutcome,
    >,
    /// Capacity-one screen text recognition. A busy controller reports
    /// busy rather than queuing a region the user has moved on from.
    pub(super) ocr: crate::ocr::OcrController,
    /// GTK toolbar frontend; `None` means the built-in bars are in charge.
    pub(super) gtk_toolbar: Option<crate::toolbar_gtk::GtkToolbarBridge>,
    /// Tick scheduling for toasts, highlights, and preset feedback.
    pub(super) ui_animation: ui_animation::UiAnimationClock,

    // Capture manager
    pub(super) capture: CaptureState,
    pub(super) frozen: FrozenState,
    pub(super) zoom: ZoomState,
    perf: perf::PerfMetrics,

    // Overlay behavior
    pub(super) exit_after_capture_mode: ExitAfterCaptureMode,

    // Pointer, cursor, pointer-lock, and touch protocol runtime.
    pub(super) pointer: pointer_runtime::PointerRuntime,

    // Manual key repeat. The keyboard is created without sctk's calloop-based
    // repeat (this loop is a manual poll), so `repeat_key` never fires.
    pub(super) key_repeat: key_repeat::KeyRepeatState,

    // IME / text-input-v3 protocol ownership and synchronization.
    pub(super) text_input: text_input::TextInputState,

    #[cfg(feature = "tablet-input")]
    pub(super) tablet: tablet_runtime::TabletState,

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
    const TOP_MARGIN_RIGHT: f64 = 12.0;
    const TOP_BASE_MARGIN_TOP: f64 = 12.0;
    const TOP_MARGIN_BOTTOM: f64 = 0.0;
    const INLINE_TOP_Y: f64 = Self::TOP_BASE_MARGIN_TOP;
    const INLINE_TOP_X: f64 = 24.0;
    const ZOOM_STEP_KEY: f64 = 1.2;
    const ZOOM_STEP_SCROLL: f64 = 1.1;
    pub(super) const ZOOM_PAN_STEP: f64 = 32.0;
    pub(super) const ZOOM_PAN_STEP_LARGE: f64 = 96.0;
}
