use super::super::super::{
    board_picker::{
        BoardPickerDrag, BoardPickerLayout, BoardPickerPageDrag, BoardPickerPageEdit,
        BoardPickerPageTarget, BoardPickerState,
    },
    color_picker_popup::{ColorPickerPopupAction, ColorPickerPopupLayout, ColorPickerPopupState},
    index::SpatialIndexCache,
    menus::{ContextMenuLayout, ContextMenuState},
    properties::{PropertiesPanelLayout, ShapePropertiesPanel},
    radial_menu::{RadialMenuLayout, RadialMenuState},
    selection::SelectionState,
    status_hud::StatusHudRebuildInputs,
};
use super::super::InputEffectOutbox;
use super::super::toast_queue::ToastQueue;
use super::super::types::{
    BlockedActionFeedback, BoardPickerClickState, CompositorCapabilities, DelayedHistory,
    DrawingState, PendingBoardDelete, PendingClipboardFallback, PendingOnboardingUsage,
    PendingPageDelete, PolygonClickState, PresetFeedbackState, PressureThicknessEditMode,
    PressureThicknessEntryMode, SelectionAxis, SelectionPublishState, StatusChangeHighlight,
    TextBlockDrag, TextClickState, TextEditEntryFeedback, TextInputMode, UiToastState,
};
use crate::config::{
    Action, PresenterModeConfig, QuickColorPalette, RadialMenuMouseBinding, ResolvedToolbarItems,
    Shortcut, ToolPresetConfig, ToolbarItemId, ToolbarItemOrderGroup, ToolbarItemsConfig,
};
use crate::draw::frame::ShapeSnapshot;
use crate::draw::{
    ArrowStyle, BlurStyle, Color, DirtyTracker, EraserKind, FontDescriptor, Shape, ShapeId,
};
use crate::input::BoardManager;
use crate::input::boards::{BoardRestoreRequest, PageRestoreRequest};
use crate::input::state::highlight::ClickHighlightState;
use crate::input::state::input_hud::InputHudState;
use crate::input::{
    MouseButton,
    modifiers::{DragToolBindings, Modifiers},
    tool::{EraserMode, PerToolDrawingSettings, Tool},
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
    /// Current drawing color (changed with color keys: R, G, B, etc.)
    pub current_color: Color,
    /// Colors selected by quick color actions and palette UI.
    pub(crate) quick_colors: QuickColorPalette,
    /// Session-only recently applied colors, most-recent-first, deduped and
    /// capped (like `board_recent`, never persisted). Shown as the radial
    /// color ring's appended recents arc.
    pub(crate) recent_colors: Vec<Color>,
    /// Current pen/line thickness in pixels (changed with +/- keys)
    pub current_thickness: f64,
    /// Independent color/thickness values for drawing tools.
    pub(crate) tool_settings: PerToolDrawingSettings,
    /// Threshold (in pixels) before storing pressure-sensitive strokes.
    pub(crate) pressure_variation_threshold: f64,
    /// How selection thickness edits apply to pressure-sensitive strokes.
    pub(crate) pressure_thickness_edit_mode: PressureThicknessEditMode,
    /// When to show a thickness entry for pressure-sensitive selections.
    pub(crate) pressure_thickness_entry_mode: PressureThicknessEntryMode,
    /// Per-step scale factor when using scale mode for pressure thickness edits.
    pub(crate) pressure_thickness_scale_step: f64,
    /// Current eraser size in pixels
    pub eraser_size: f64,
    /// Current eraser brush shape
    pub eraser_kind: EraserKind,
    /// Current eraser behavior mode
    pub eraser_mode: EraserMode,
    /// Opacity multiplier for marker tool strokes
    pub marker_opacity: f64,
    /// Release-time smoothing passes applied to finished freehand and marker
    /// strokes. 0 keeps the exact drawn path.
    pub pen_smoothing: u8,
    /// How the blur tool obscures the region it covers
    pub blur_style: BlurStyle,
    /// Alpha of the dim layer outside every spotlight
    pub spotlight_dim_opacity: f64,
    /// Fraction of each spotlight radius spent fading out at the edge
    pub spotlight_feather: f64,
    /// Magnification copied into the next Spotlight shape.
    pub spotlight_magnification: f64,
    /// Current font size for text mode (from config)
    pub current_font_size: f64,
    /// Font descriptor for text rendering (family, weight, style)
    pub font_descriptor: FontDescriptor,
    /// Families the font-cycle action steps through. Empty turns it off.
    pub(crate) font_cycle: Vec<String>,
    /// State owned by the system font-picker modal.
    pub(crate) font_picker: crate::input::state::core::font_picker::FontPickerState,
    /// Whether to draw background behind text
    pub text_background_enabled: bool,
    /// Optional wrap width for text input (None = auto)
    pub text_wrap_width: Option<i32>,
    /// Which text input style is active (plain vs sticky note)
    pub text_input_mode: TextInputMode,
    /// Arrowhead length in pixels (from config)
    pub arrow_length: f64,
    /// Arrowhead angle in degrees (from config)
    pub arrow_angle: f64,
    /// Whether the arrowhead is placed at the end of the line
    pub arrow_head_at_end: bool,
    /// Style copied into the next arrow drawn
    pub arrow_style: ArrowStyle,
    /// Whether auto-numbered arrow labels are enabled
    pub arrow_label_enabled: bool,
    /// Next label value for auto-numbered arrows
    pub arrow_label_counter: u32,
    /// Next label value for step markers
    pub step_marker_counter: u32,
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
    pub help_overlay: crate::input::state::core::HelpOverlayState,
    /// Board picker quick search query
    pub board_picker_search: String,
    /// Time of last board picker search input
    pub board_picker_search_last_input: Option<Instant>,
    /// State owned by the command palette modal.
    pub command_palette: crate::input::state::core::command_palette::CommandPaletteState,
    /// Action whose next keyboard chord is being captured for rebinding.
    pub keybinding_capture_action: Option<Action>,
    /// Duration for command palette action toasts (ms)
    pub command_palette_toast_duration_ms: u64,
    /// Runtime visibility preferences for overlay chrome and toolbar sections.
    pub ui_visibility: crate::input::state::UiVisibility,
    /// When the zoom chip shows: always, or only while zoom is active
    /// (`[ui.toolbar] zoom_chip_display`)
    pub zoom_chip_display: crate::config::ZoomChipDisplay,
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
    /// Hovered status HUD segment (idle pointer only; drives the hover
    /// backdrop and the pointer cursor over the pill)
    pub status_hud_hover: Option<crate::ui::StatusHudSegmentKind>,
    /// Hovered zoom chip button (idle pointer only; same affordance)
    pub zoom_chip_hover: Option<crate::ui::ZoomChipButtonKind>,
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
    /// Whether fill is enabled for fill-capable shapes (rect, ellipse)
    pub fill_enabled: bool,
    /// Current side count for regular polygon drawing.
    pub polygon_sides: u8,
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
    /// Cached bounds for live text preview/caret (if any)
    pub(crate) last_text_preview_bounds: Option<Rect>,
    /// Coalesced request for the Wayland backend to publish the current text
    /// caret rectangle to text-input-v3.
    pub(crate) text_input_cursor_rect_dirty: bool,
    /// Whether the coalesced text-input update was caused outside the input
    /// method (keyboard editing, pointer placement, or clipboard completion).
    pub(crate) text_input_external_change_dirty: bool,
    /// Identity of the current text-edit session. Async clipboard completions
    /// use this to avoid pasting into a later edit.
    pub(crate) text_input_generation: u64,
    /// Buffer mutation revision within the current text-edit session. Deferred
    /// cuts use it to reject completions after any intervening edit, even when
    /// the same bytes and selection are later restored.
    pub(crate) text_input_revision: u64,
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
    /// Optional tool override independent of modifier keys
    pub(in crate::input::state::core) tool_override: Option<Tool>,
    /// Current selection information
    pub selection_state: SelectionState,
    /// Last axis used for selection nudges (used to resolve Home/End axis)
    pub last_selection_axis: Option<SelectionAxis>,
    /// Current context menu state
    pub context_menu_state: ContextMenuState,
    /// Page context target for the context menu
    pub(in crate::input::state::core) context_menu_page_target: Option<BoardPickerPageTarget>,
    /// Whether context menu interactions are enabled
    pub(in crate::input::state::core) context_menu_enabled: bool,
    /// Current board picker state
    pub board_picker_state: BoardPickerState,
    /// Active board picker drag state (full mode reorder)
    pub board_picker_drag: Option<BoardPickerDrag>,
    /// Active board picker page drag state (thumbnail reorder)
    pub board_picker_page_drag: Option<BoardPickerPageDrag>,
    /// Active board picker page rename state
    pub board_picker_page_edit: Option<BoardPickerPageEdit>,
    /// Current color picker popup state
    pub color_picker_popup_state: ColorPickerPopupState,
    /// Cached layout details for the color picker popup
    pub color_picker_popup_layout: Option<ColorPickerPopupLayout>,
    /// Identity of the currently open color picker popup.
    pub(in crate::input::state) color_picker_popup_generation: u64,
    /// Popup action button owned by the current left-button press.
    pub(in crate::input::state) color_picker_popup_pressed_action: Option<ColorPickerPopupAction>,
    /// Current radial menu state
    pub radial_menu_state: RadialMenuState,
    /// Cached layout details for the radial menu
    pub radial_menu_layout: Option<RadialMenuLayout>,
    /// Mouse button used to toggle the radial menu.
    pub radial_menu_mouse_binding: RadialMenuMouseBinding,
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
    /// Maximum number of undo actions retained in history
    pub undo_stack_limit: usize,
    /// Delay between steps when running undo-all via delay (ms)
    pub undo_all_delay_ms: u64,
    /// Delay between steps when running redo-all via delay (ms)
    pub redo_all_delay_ms: u64,
    /// Delay between steps for custom undo (ms)
    pub custom_undo_delay_ms: u64,
    /// Delay between steps for custom redo (ms)
    pub custom_redo_delay_ms: u64,
    /// Number of steps to perform for custom undo
    pub custom_undo_steps: usize,
    /// Number of steps to perform for custom redo
    pub custom_redo_steps: usize,
    /// Whether the custom undo/redo section is visible
    pub custom_section_enabled: bool,
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
    /// Copied selection shapes for paste operations
    pub(in crate::input::state::core) selection_clipboard: Option<Vec<Shape>>,
    /// Local clipboard generation for the copied shape selection.
    pub(in crate::input::state::core) selection_clipboard_generation: u64,
    /// System clipboard publication state for the current local selection.
    pub(in crate::input::state::core) selection_publish_state: SelectionPublishState,
    /// Per-process id embedded in private Wayscriber clipboard payloads.
    pub(in crate::input::state::core) clipboard_app_instance_id: String,
    /// Monotonic id source for paste requests.
    pub(in crate::input::state::core) clipboard_paste_request_counter: u64,
    /// Latest paste request id whose completion should still be accepted.
    pub(in crate::input::state::core) active_clipboard_paste_request_id: Option<u64>,
    /// Last capture path (for quick open-folder action)
    pub(in crate::input::state::core) last_capture_path: Option<PathBuf>,
    /// Last text/note click used for double-click detection
    pub(crate) last_text_click: Option<TextClickState>,
    /// Last freeform polygon point click used for double-click completion.
    pub(crate) last_polygon_click: Option<PolygonClickState>,
    /// Last board picker row click used for double-click detection
    pub(crate) last_board_picker_click: Option<BoardPickerClickState>,
    /// Tracks an in-progress text edit target (existing shape to replace)
    pub(crate) text_edit_target: Option<(ShapeId, ShapeSnapshot)>,
    /// In-progress Alt+left-drag that repositions the active text block.
    pub(crate) text_block_drag: Option<TextBlockDrag>,
    /// Animation state for text edit mode entry (teal glow pulse)
    pub(crate) text_edit_entry_feedback: Option<TextEditEntryFeedback>,
    /// Input-method composition state (preedit + pending IME batch) for the
    /// active text/note edit.
    pub(crate) ime: super::super::super::ime::ImeCompositionState,
    /// Pending delayed history playback state
    pub(in crate::input::state::core) pending_history: Option<DelayedHistory>,
    /// Cached layout details for the currently open context menu
    pub context_menu_layout: Option<ContextMenuLayout>,
    /// Cached layout details for the board picker overlay
    pub board_picker_layout: Option<BoardPickerLayout>,
    /// Cached layout details for the status HUD (segmented status bar)
    pub status_hud_layout: Option<crate::ui::StatusHudLayout>,
    /// Last screen/config inputs used to build the status HUD. Retained while
    /// UI rendering is active so a content toggle can synchronously refresh
    /// `status_hud_layout`, keeping policy exact — including width degradation
    /// on narrow outputs — until the next rendered frame. Suppression clears
    /// this so policy cannot mistake configured content for chrome that is
    /// currently on screen.
    pub(in crate::input::state::core) status_hud_rebuild_inputs: Option<StatusHudRebuildInputs>,
    /// Set when the internal pointer-routing chain consumed a left press on
    /// the status HUD (tablet and other paths that bypass the backend's own
    /// press→release flag); the matching release activates the chip.
    pub(in crate::input::state) status_hud_press_pending: bool,
    /// Cached layout details for the interactive bottom-right zoom chip
    pub zoom_chip_layout: Option<crate::ui::ZoomChipLayout>,
    /// The chip press a left press recorded, set when the internal
    /// pointer-routing chain consumed that press (tablet and other paths that
    /// bypass the backend's own press→release flag). `Button(kind)` records the
    /// pressed button so the matching release fires only when it lands on the
    /// SAME button; `Passive` marks a press on the passive `NN%` readout (or an
    /// inter-piece gap) so its release is still consumed but fires nothing;
    /// `None` means no chip press is pending.
    pub(in crate::input::state) zoom_chip_press_pending: crate::ui::ZoomChipPress,
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
    /// Optional properties panel describing the current selection
    pub(in crate::input::state::core) shape_properties_panel: Option<ShapePropertiesPanel>,
    /// Cached layout details for the current properties panel
    pub properties_panel_layout: Option<PropertiesPanelLayout>,
    /// Recompute properties hover next time layout is available
    pub(in crate::input::state::core) pending_properties_hover_recalc: bool,
    /// Refresh properties panel entries on the next layout pass
    pub(in crate::input::state::core) properties_panel_needs_refresh: bool,
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
    /// Number of preset slots to display
    pub preset_slot_count: usize,
    /// Preset slots for quick tool switching
    pub presets: Vec<Option<ToolPresetConfig>>,
    /// Last applied preset slot (for UI highlight)
    pub active_preset_slot: Option<usize>,
    /// Transient preset feedback for toolbar animations
    pub(crate) preset_feedback: Vec<Option<PresetFeedbackState>>,
    /// Whether the guided tour is currently active
    pub tour_active: bool,
    /// Current step in the guided tour (0-indexed)
    pub tour_step: usize,
    /// Compositor capabilities (layer-shell, screencopy, etc.)
    pub compositor_capabilities: CompositorCapabilities,
    /// Capabilities snapshot the capability warning toast last evaluated;
    /// `None` until first evaluated, re-evaluated whenever capabilities change
    /// (read/written each tick by the wayland backend's capability toast).
    pub(crate) capability_toast_caps: Option<CompositorCapabilities>,
    /// Blocked action visual feedback state (red flash)
    pub(crate) blocked_action_feedback: Option<BlockedActionFeedback>,
    /// Pending clipboard fallback for failed copy operations
    pub(crate) pending_clipboard_fallback: Option<PendingClipboardFallback>,
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
