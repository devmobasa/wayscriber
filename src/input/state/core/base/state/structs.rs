use super::super::super::{
    CanvasIndex, Feedback, Keymap, PointerTracking, ToolbarInteraction, ToolbarVisibility,
    ViewState,
    selection::{SelectionClipboard, SelectionInteraction},
};
use super::super::InputEffectOutbox;
use super::super::types::{
    CompositorCapabilities, DrawingState, PendingBoardDelete, PendingOnboardingUsage,
    PendingPageDelete,
};
use crate::config::PresenterModeConfig;
use crate::draw::{Color, DirtyTracker};
use crate::input::BoardManager;
use crate::input::boards::{BoardRestoreRequest, PageRestoreRequest};
use crate::input::state::highlight::ClickHighlightState;
use crate::input::state::input_hud::InputHudState;
use crate::input::{modifiers::Modifiers, tool::Tool};
use crate::render_profiles::RenderProfileSet;
use crate::session::SessionOptions;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PresenterRestore {
    pub(crate) show_status_bar: Option<bool>,
    pub(crate) show_tool_preview: Option<bool>,
    /// Toolbar visibility before presenter hid or mapped the strip.
    pub(crate) toolbar_visibility: Option<ToolbarVisibility>,
    pub(crate) click_highlight_enabled: Option<bool>,
    pub(crate) input_hud_enabled: Option<bool>,
    pub(crate) tool_override: Option<Option<Tool>>,
}

/// Chrome visibility snapshot taken when focus mode hides every persistent
/// UI surface; restored exactly on the second `ToggleFocusMode` press.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FocusModeRestore {
    pub(crate) show_status_bar: bool,
    pub(crate) toolbar_visibility: ToolbarVisibility,
    pub(crate) show_floating_badge: bool,
    pub(crate) show_zoom_chip: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LightModeRestore {
    pub(crate) show_status_bar: bool,
    pub(crate) show_tool_preview: bool,
    pub(crate) toolbar_visibility: ToolbarVisibility,
    pub(crate) click_highlight_enabled: bool,
    pub(crate) tool_override: Option<Tool>,
}

pub struct InputState {
    /// Typed handoff of in-process work whose side effects belong to the backend.
    /// The outbox owns FIFO, coalesced, and last-wins storage policy plus the
    /// ordered runtime and durable-shutdown drain inventories.
    pub(in crate::input::state::core) input_effects: InputEffectOutbox,
    /// Multi-board canvas management
    pub boards: BoardManager,
    /// Runtime drawing defaults and per-tool appearance settings.
    pub(crate) style: crate::input::state::core::DrawingStyle,
    /// State owned by the system font-picker modal.
    pub(crate) font_picker: crate::input::state::core::font_picker::FontPickerState,
    /// Text-editor mode, asynchronous identity, composition, and pointer state.
    pub(crate) text_editing: crate::input::state::core::TextEditing,
    /// Current modifier key state
    pub modifiers: Modifiers,
    /// Configured shortcuts plus transient keyboard and pointer-dispatch state.
    pub(in crate::input::state) keymap: Keymap,
    /// Zoom, frozen-mode, screen geometry, and active-output state.
    pub(in crate::input::state) view: ViewState,
    /// Pointer positions and transient pointer-driven bookkeeping.
    pub(in crate::input::state) pointer: PointerTracking,
    /// Hit-test caches, indexing policy, and frame shape cap.
    pub(in crate::input::state) canvas_index: CanvasIndex,
    /// Current drawing mode state machine
    pub state: DrawingState,
    /// Whether user requested to exit the overlay
    pub should_exit: bool,
    /// Exit that must not be deferred by XDG stay-mode focus loss (for example
    /// exit-after-capture). Consumed together with the Wayland explicit-close bit.
    pub(crate) explicit_exit_requested: bool,
    /// Whether the display needs to be redrawn
    pub needs_redraw: bool,
    /// Whether session persistence should capture changes (cleared after autosave check)
    pub(crate) session_dirty: bool,
    /// Runtime session options used to preflight clone-heavy actions before mutation.
    pub(crate) session_preflight_options: Option<SessionOptions>,
    /// Save Session As target waiting for explicit overwrite confirmation.
    pub(crate) pending_save_as_overwrite: Option<PathBuf>,
    /// Visibility, navigation, and pointer bookkeeping for the help overlay.
    pub(crate) help_overlay: crate::input::state::core::HelpOverlayState,
    /// Modal, layout, search, edit, and drag state for the board picker.
    pub(crate) board_picker: crate::input::state::core::BoardPickerPanel,
    /// State owned by the command palette modal.
    pub command_palette: crate::input::state::core::command_palette::CommandPaletteState,
    /// Toast, geometry, blocked-action, and capability feedback state.
    pub(in crate::input::state) feedback: Feedback,
    /// Runtime visibility preferences for overlay chrome and toolbar sections.
    pub ui_visibility: crate::input::state::UiVisibility,
    /// Display policy, cached geometry, and pointer interaction state for the zoom chip.
    pub(crate) zoom_chip: crate::input::state::core::zoom_chip::ZoomChipState,
    /// Whether presenter mode is currently enabled
    pub presenter_mode: bool,
    /// Presenter mode behavior configuration
    pub presenter_mode_config: PresenterModeConfig,
    /// Configured render color profiles and active preview state.
    pub(crate) render_profiles: RenderProfileSet,
    /// Previous UI state to restore after presenter mode exits
    pub(crate) presenter_restore: Option<PresenterRestore>,
    /// Chrome snapshot while focus mode is active (`Some` = active).
    pub(crate) focus_mode_restore: Option<FocusModeRestore>,
    /// Cached geometry and pointer interaction state for the status HUD.
    pub(crate) status_hud: crate::input::state::core::status_hud::StatusHudState,
    /// Whether passthrough light mode is currently enabled
    pub light_mode: bool,
    /// Whether light mode is temporarily accepting drawing input
    pub light_mode_drawing: bool,
    /// Previous UI state to restore after light mode exits
    pub(crate) light_mode_restore: Option<LightModeRestore>,
    /// Toolbar visibility, preferences, resolved layout, and interaction state.
    pub(in crate::input::state) toolbar: ToolbarInteraction,
    /// Precise numeric entry popup opened from a pill numeral, when open.
    pub(crate) precision_entry: Option<crate::input::state::PrecisionEntryState>,
    /// Previous color before entering board mode (for restoration)
    pub board_previous_color: Option<Color>,
    /// Most recently used board ids (most recent first)
    pub board_recent: Vec<String>,
    /// Pending confirmation for deleting a board
    pub(in crate::input::state::core) pending_board_delete: Option<PendingBoardDelete>,
    /// Pending confirmation for deleting a page
    pub(in crate::input::state::core) pending_page_delete: Option<PendingPageDelete>,
    /// Recently deleted pages (for undo), with expiration timestamps
    pub(in crate::input::state::core) deleted_pages: Vec<(PageRestoreRequest, Instant)>,
    /// Tracks dirty regions between renders
    pub(crate) dirty_tracker: DirtyTracker,
    /// In-flight Spotlight magnification undo gesture and wheel remainder.
    pub(in crate::input::state) spotlight_wheel: crate::input::state::SpotlightWheelGesture,
    /// Pending first-run onboarding usage markers to persist in onboarding store
    pub(crate) pending_onboarding_usage: PendingOnboardingUsage,
    /// Click highlight animation state
    pub(crate) click_highlight: ClickHighlightState,
    /// On-screen input HUD (keystroke/click chips) state
    pub(crate) input_hud: InputHudState,
    /// Selection membership, nudge direction, and polygon click timing.
    pub(in crate::input::state) selection_interaction: SelectionInteraction,
    /// Lifecycle, target, and cached layout for the context menu.
    pub(crate) context_menu: crate::input::state::core::menus::ContextMenuPanel,

    /// Modal state, cached geometry, and press identity for the color picker popup.
    pub(crate) color_picker_popup: crate::input::state::core::ColorPickerPopupPanel,
    /// Lifecycle, layout, and configured pointer trigger for the radial menu.
    pub(crate) radial_menu: crate::input::state::core::RadialMenuPanel,
    /// Undo retention, delayed playback settings, and active playback state.
    pub(crate) history_limits: crate::input::state::core::HistoryLimits,
    /// The scan-band overlay shown while screen text recognition runs, and the
    /// outcome card that follows it.
    pub(crate) ocr_scan: Option<crate::input::state::core::utility::ocr_scan::OcrScan>,
    /// Local selection clipboard, publication, paste request, and image fallback state.
    pub(in crate::input::state::core) selection_clipboard: SelectionClipboard,
    /// Last capture path (for quick open-folder action)
    pub(in crate::input::state::core) last_capture_path: Option<PathBuf>,
    /// Lifecycle, cached geometry, and deferred refresh state for the properties panel.
    pub(crate) properties: crate::input::state::core::properties::PropertiesPanelState,
    /// Screen-color eyedropper UI lifecycle.
    pub(in crate::input::state::core) eyedropper_ui_state:
        crate::input::state::core::EyedropperUiState,
    /// Generalized screen-region selector lifecycle.
    pub(in crate::input::state::core) region_select_ui_state:
        crate::input::state::core::RegionSelectUiState,
    /// Runtime preset values, active selection, and transient feedback.
    pub(crate) preset_slots: crate::input::state::core::PresetSlots,
    /// Lifecycle and navigation state for the guided tour.
    pub(crate) tour: crate::input::state::core::TourState,
    /// Compositor capabilities (layer-shell, screencopy, etc.)
    pub compositor_capabilities: CompositorCapabilities,
    /// Recently deleted boards available for undo (with deletion timestamp)
    pub(in crate::input::state::core) deleted_boards: Vec<(BoardRestoreRequest, Instant)>,
}
