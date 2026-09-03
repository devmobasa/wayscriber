use super::super::super::{
    index::SpatialIndexCache,
    selection::{SelectionClipboard, SelectionInteraction},
};
use super::super::InputEffectOutbox;
use super::super::toast_queue::ToastQueue;
use super::super::types::{
    BlockedActionFeedback, CompositorCapabilities, DrawingState, PendingBoardDelete,
    PendingOnboardingUsage, PendingPageDelete, StatusChangeHighlight, UiToastState,
};
use crate::config::{
    Action, PresenterModeConfig, ResolvedToolbarItems, Shortcut, ToolbarItemId,
    ToolbarItemOrderGroup, ToolbarItemsConfig,
};
use crate::draw::{Color, DirtyTracker, ShapeId};
use crate::input::BoardManager;
use crate::input::boards::{BoardRestoreRequest, PageRestoreRequest};
use crate::input::state::highlight::ClickHighlightState;
use crate::input::state::input_hud::InputHudState;
use crate::input::{
    MouseButton,
    modifiers::{DragToolBindings, Modifiers},
    tool::Tool,
};
use crate::render_profiles::RenderProfileSet;
use crate::session::SessionOptions;
use crate::util::Rect;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PresenterRestore {
    pub(crate) show_status_bar: Option<bool>,
    pub(crate) show_tool_preview: Option<bool>,
    pub(crate) toolbar_visible: Option<bool>,
    pub(crate) toolbar_top_visible: Option<bool>,
    /// Top-strip form/minimize state before presenter mapped the strip to
    /// the micro chip (`[presenter_mode] toolbar_mode = "micro"`).
    pub(crate) toolbar_top_display_mode: Option<crate::config::TopDisplayMode>,
    pub(crate) toolbar_top_minimized: Option<bool>,
    pub(crate) click_highlight_enabled: Option<bool>,
    pub(crate) input_hud_enabled: Option<bool>,
    pub(crate) tool_override: Option<Option<Tool>>,
}

/// Chrome visibility snapshot taken when focus mode hides every persistent
/// UI surface; restored exactly on the second `ToggleFocusMode` press.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FocusModeRestore {
    pub(crate) show_status_bar: bool,
    pub(crate) toolbar_visible: bool,
    pub(crate) toolbar_top_visible: bool,
    pub(crate) toolbar_top_display_mode: crate::config::TopDisplayMode,
    pub(crate) show_floating_badge: bool,
    pub(crate) show_zoom_chip: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LightModeRestore {
    pub(crate) show_status_bar: bool,
    pub(crate) show_tool_preview: bool,
    pub(crate) toolbar_visible: bool,
    pub(crate) toolbar_top_visible: bool,
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
    /// Tool mapping for drag gestures with modifier keys
    pub drag_tool_bindings: DragToolBindings,
    /// Mouse button that started the active pointer drag, if any.
    pub(crate) active_drag_button: Option<MouseButton>,
    /// Per-drag color override, if the current drag binding configured one.
    pub(crate) active_drag_color: Option<Color>,
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
    /// Action whose next keyboard chord is being captured for rebinding.
    pub keybinding_capture_action: Option<Action>,
    /// Duration for command palette action toasts (ms)
    pub command_palette_toast_duration_ms: u64,
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
    /// Whether the toolbar is visible (combined flag, prefer `toolbar_top_visible`)
    pub toolbar_visible: bool,
    /// Whether the top toolbar panel is visible
    pub toolbar_top_visible: bool,
    /// Whether the top toolbar is pinned (saved to config, opens at startup)
    pub toolbar_top_pinned: bool,
    /// Whether to use icons instead of text labels in toolbars
    pub toolbar_use_icons: bool,
    /// Scale factor for toolbar UI (icons + layout)
    pub toolbar_scale: f64,
    /// Current toolbar layout complexity
    pub toolbar_layout_mode: crate::config::ToolbarLayoutMode,
    /// Optional per-mode overrides for toolbar sections
    pub toolbar_mode_overrides: crate::config::ToolbarModeOverrides,
    /// Raw item-level toolbar visibility config, preserving unknown IDs.
    pub toolbar_items: ToolbarItemsConfig,
    /// Resolved known item-level toolbar visibility config.
    pub resolved_toolbar_items: ResolvedToolbarItems,
    /// Active toolbar customization reorder drag source.
    pub toolbar_customize_drag: Option<(ToolbarItemOrderGroup, ToolbarItemId)>,
    /// The one open top-strip menu, if any. A single typed state makes the
    /// shape picker, overflow, and Canvas/Session/Settings popovers mutually
    /// exclusive by construction.
    pub(crate) toolbar_top_menu: crate::input::state::TopMenuState,
    /// Internal scroll offset of the open Canvas/Session/Settings popover
    /// (logical pixels, clamped at render; reset when a popover opens).
    pub toolbar_top_popover_scroll: f64,
    /// Whether the top strip is minimized to its edge restore tab.
    pub toolbar_top_minimized: bool,
    /// Display form of the top strip (full strip / micro chip / cycle-hidden).
    /// Sibling of `toolbar_top_minimized`; minimized wins when both are set.
    pub toolbar_top_display_mode: crate::config::TopDisplayMode,
    /// When drawing input last started or committed a stroke; drives the
    /// top-strip idle fade.
    pub(crate) last_draw_activity: Instant,
    /// Precise numeric entry popup opened from a pill numeral, when open.
    pub(crate) precision_entry: Option<crate::input::state::PrecisionEntryState>,
    /// Modifier chord that turns a toolbar click into shortcut rebinding.
    /// Used to generate onboarding copy (the tour's rebind hint) without
    /// hardcoding key strings. Startup init applies the config value.
    pub toolbar_rebind_modifier: crate::config::ToolbarRebindModifier,
    /// Whether the Settings drawer is showing the toolbar item customization sub-panel
    pub toolbar_customize_items_open: bool,
    /// Selected toolbar item customization group in the Settings drawer sub-panel
    pub toolbar_customize_items_group: Option<crate::ui::toolbar::ToolbarItemCustomizeGroup>,
    /// Whether the Settings drawer is showing status-bar content controls
    pub toolbar_status_bar_contents_open: bool,
    /// Screen width in pixels (set by backend after configuration)
    pub screen_width: u32,
    /// Screen height in pixels (set by backend after configuration)
    pub screen_height: u32,
    /// Active output label shown in status bar when configured.
    pub active_output_label: Option<String>,
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
    /// Cached bounds for the current provisional shape (if any)
    pub(crate) last_provisional_bounds: Option<Rect>,
    /// Keybinding action map for efficient lookup
    pub(in crate::input::state::core) action_map: HashMap<Shortcut, Action>,
    /// Ordered keybindings per action (as configured)
    pub(in crate::input::state::core) action_bindings: HashMap<Action, Vec<Shortcut>>,
    /// Keyboard sequence trie derived from `action_map`.
    pub(in crate::input::state::core) sequence_trie:
        crate::input::state::core::utility::SequenceTrie,
    /// In-progress multi-step keyboard sequence, if any.
    pub(in crate::input::state::core) pending_sequence:
        Option<crate::input::state::core::utility::PendingSequence>,
    /// Auxiliary pointer codes whose press dispatched a shortcut, so the
    /// matching release is consumed instead of starting a stroke or UI action.
    pub(crate) consumed_pointer_buttons: HashSet<u32>,
    /// Bumped whenever the keymap is replaced. Shortcut labels feed command
    /// scoring, so the palette's result cache keys on this.
    pub(in crate::input::state::core) keymap_revision: u64,
    /// Shape and pre-gesture snapshot for an in-flight wheel adjustment of a
    /// Spotlight's magnification.
    ///
    /// A wheel burst is one user action, so the snapshot is held here and a
    /// single undo entry is pushed when the gesture ends rather than one per
    /// tick.
    pub(in crate::input::state) spotlight_magnification_gesture:
        Option<crate::input::state::SpotlightMagnificationGesture>,
    /// Unconsumed high-resolution wheel units and the Spotlight that owns
    /// them. Wayland defines 120 units as one logical wheel step.
    pub(in crate::input::state) spotlight_wheel_value120_remainder: Option<(ShapeId, i32)>,
    /// Pending first-run onboarding usage markers to persist in onboarding store
    pub(crate) pending_onboarding_usage: PendingOnboardingUsage,
    /// Maximum number of shapes allowed per frame (0 = unlimited)
    pub max_shapes_per_frame: usize,
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
    /// Cached hit-test bounds per shape id
    pub(in crate::input::state::core) hit_test_cache: HashMap<ShapeId, Rect>,
    /// Monotonic counter bumped whenever committed shape content may have
    /// changed (piggybacks on hit-cache invalidation). Used by render-side
    /// caches to detect content changes cheaply.
    pub(in crate::input::state::core) canvas_content_generation: u64,
    /// Hit test tolerance in pixels
    pub hit_test_tolerance: f64,
    /// Threshold before enabling spatial indexing
    pub max_linear_hit_test: usize,
    /// Undo retention, delayed playback settings, and active playback state.
    pub(crate) history_limits: crate::input::state::core::HistoryLimits,
    /// The scan-band overlay shown while screen text recognition runs, and the
    /// outcome card that follows it.
    pub(crate) ocr_scan: Option<crate::input::state::core::utility::ocr_scan::OcrScan>,
    /// Active (visible) UI toast (errors/warnings/info)
    pub(crate) ui_toast: Option<UiToastState>,
    /// Pending toasts waiting behind the active one, plus rate-limit memory
    pub(crate) toast_queue: ToastQueue,
    /// Cached bounds of the rendered toast for click detection (x, y, w, h)
    pub(crate) ui_toast_bounds: Option<(f64, f64, f64, f64)>,
    /// Cached bounds of up to two rendered toast action chips.
    pub(crate) ui_toast_action_bounds: [Option<(f64, f64, f64, f64)>; 2],
    /// Local selection clipboard, publication, paste request, and image fallback state.
    pub(in crate::input::state::core) selection_clipboard: SelectionClipboard,
    /// Last capture path (for quick open-folder action)
    pub(in crate::input::state::core) last_capture_path: Option<PathBuf>,

    /// Spatial grid plus guarded ShapeId-to-z-order indices for large-frame hit-testing.
    pub(in crate::input::state::core) spatial_index: Option<SpatialIndexCache>,
    /// Last known pointer position in screen coordinates (for overlays and hover refresh)
    pub(in crate::input::state::core) last_pointer_position: (i32, i32),
    /// Last known pointer position in canvas/world coordinates
    pub(in crate::input::state::core) last_canvas_pointer_position: (i32, i32),
    /// Whether a real pointer position has been observed.
    pub(in crate::input::state::core) pointer_seen: bool,
    /// Recompute hover next time layout is available
    pub(in crate::input::state::core) pending_menu_hover_recalc: bool,
    /// Lifecycle, cached geometry, and deferred refresh state for the properties panel.
    pub(crate) properties: crate::input::state::core::properties::PropertiesPanelState,
    /// Whether frozen mode is currently active
    pub(in crate::input::state::core) frozen_active: bool,
    /// Screen-color eyedropper UI lifecycle.
    pub(in crate::input::state::core) eyedropper_ui_state:
        crate::input::state::core::EyedropperUiState,
    /// Generalized screen-region selector lifecycle.
    pub(in crate::input::state::core) region_select_ui_state:
        crate::input::state::core::RegionSelectUiState,
    /// Whether zoom mode is currently active
    pub(in crate::input::state::core) zoom_active: bool,
    /// Whether zoom view is locked
    pub(in crate::input::state::core) zoom_locked: bool,
    /// Current zoom scale (1.0 = no zoom)
    pub(in crate::input::state::core) zoom_scale: f64,
    /// Current zoom view offset in canvas/world space
    pub(in crate::input::state::core) zoom_view_offset: (f64, f64),
    /// Runtime preset values, active selection, and transient feedback.
    pub(crate) preset_slots: crate::input::state::core::PresetSlots,
    /// Lifecycle and navigation state for the guided tour.
    pub(crate) tour: crate::input::state::core::TourState,
    /// Compositor capabilities (layer-shell, screencopy, etc.)
    pub compositor_capabilities: CompositorCapabilities,
    /// Capabilities snapshot the capability warning toast last evaluated;
    /// `None` until first evaluated, re-evaluated whenever capabilities change
    /// (read/written each tick by the wayland backend's capability toast).
    pub(crate) capability_toast_caps: Option<CompositorCapabilities>,
    /// Blocked action visual feedback state (red flash)
    pub(crate) blocked_action_feedback: Option<BlockedActionFeedback>,
    /// Recently deleted boards available for undo (with deletion timestamp)
    pub(in crate::input::state::core) deleted_boards: Vec<(BoardRestoreRequest, Instant)>,
    /// Status bar change highlight animation state
    #[allow(dead_code)]
    pub(crate) status_change_highlight: Option<StatusChangeHighlight>,
}

impl InputState {
    /// Record drawing activity (stroke start/commit); resets the top-strip
    /// idle-fade clock.
    pub(crate) fn mark_draw_activity(&mut self) {
        self.last_draw_activity = Instant::now();
    }

    /// When drawing input last started or committed a stroke.
    pub fn last_draw_activity(&self) -> Instant {
        self.last_draw_activity
    }
}
