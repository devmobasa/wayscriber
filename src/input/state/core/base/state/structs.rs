use super::super::super::{
    BoardTransitions, CanvasIndex, ChromeModes, Feedback, Keymap, PointerTracking, SessionFlags,
    ToolbarInteraction, ViewState,
    selection::{SelectionClipboard, SelectionInteraction},
};
use super::super::InputEffectOutbox;
use super::super::types::{CompositorCapabilities, DrawingState, PendingOnboardingUsage};
use crate::draw::DirtyTracker;
use crate::input::BoardManager;
use crate::input::modifiers::Modifiers;
use crate::input::state::highlight::ClickHighlightState;
use crate::input::state::input_hud::InputHudState;
use crate::render_profiles::RenderProfileSet;

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
    /// Whether the display needs to be redrawn.
    pub needs_redraw: bool,
    /// Dirty, preflight, save-as, and last-capture session state.
    pub(in crate::input::state) session_flags: SessionFlags,
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
    /// Presenter, focus, and light mode flags, configuration, and restore snapshots.
    pub(in crate::input::state) modes: ChromeModes,
    /// Configured render color profiles and active preview state.
    pub(crate) render_profiles: RenderProfileSet,
    /// Cached geometry and pointer interaction state for the status HUD.
    pub(crate) status_hud: crate::input::state::core::status_hud::StatusHudState,
    /// Toolbar visibility, preferences, resolved layout, and interaction state.
    pub(in crate::input::state) toolbar: ToolbarInteraction,
    /// Precise numeric entry popup opened from a pill numeral, when open.
    pub(crate) precision_entry: Option<crate::input::state::PrecisionEntryState>,
    /// Board switch color, recents, delete confirmations, and restore queues.
    pub(in crate::input::state) board_transitions: BoardTransitions,
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
}
