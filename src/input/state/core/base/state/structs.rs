use super::super::super::{
    board_picker::{
        BoardPickerDrag, BoardPickerLayout, BoardPickerPageDrag, BoardPickerPageEdit,
        BoardPickerPageTarget, BoardPickerState,
    },
    color_picker_popup::{
        ColorPickerPopupAction, ColorPickerPopupLayout, ColorPickerPopupState, HexPasteTarget,
    },
    index::SpatialGrid,
    menus::{ContextMenuLayout, ContextMenuState},
    properties::{PropertiesPanelLayout, ShapePropertiesPanel},
    radial_menu::{RadialMenuLayout, RadialMenuState},
    selection::SelectionState,
};
use super::super::toast_queue::ToastQueue;
use super::super::types::{
    BlockedActionFeedback, BoardPickerClickState, ClipboardPasteRequest, CompositorCapabilities,
    DelayedHistory, DrawingState, OutputFocusAction, PendingBackendAction, PendingBoardDelete,
    PendingClipboardFallback, PendingOnboardingUsage, PendingPageDelete,
    PendingSelectionClipboardPublish, PolygonClickState, PresetAction, PresetFeedbackState,
    PressureThicknessEditMode, PressureThicknessEntryMode, SelectionAxis, SelectionPublishState,
    StatusChangeHighlight, TextClickState, TextEditEntryFeedback, TextInputMode, UiToastState,
    ZoomAction,
};
use crate::config::{
    Action, KeyBinding, PresenterModeConfig, QuickColorPalette, RadialMenuMouseBinding,
    ResolvedToolbarItems, ToolPresetConfig, ToolbarItemId, ToolbarItemOrderGroup,
    ToolbarItemsConfig,
};
use crate::draw::frame::ShapeSnapshot;
use crate::draw::{Color, DirtyTracker, EraserKind, FontDescriptor, Shape, ShapeId};
use crate::input::BoardManager;
use crate::input::boards::{
    BoardRestoreRequest, PageRestoreRequest, PendingBoardConfigUpdate, PendingBoardRuntimeUiAction,
};
use crate::input::state::highlight::ClickHighlightState;
use crate::input::{
    Key, MouseButton,
    modifiers::{DragToolBindings, Modifiers},
    tool::{EraserMode, PerToolDrawingSettings, Tool},
};
use crate::render_profiles::RenderProfileSet;
use crate::session::SessionOptions;
use crate::ui::toolbar::ToolbarSideSection;
use crate::util::Rect;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PresenterRestore {
    pub(crate) show_status_bar: Option<bool>,
    pub(crate) show_tool_preview: Option<bool>,
    pub(crate) toolbar_visible: Option<bool>,
    pub(crate) toolbar_top_visible: Option<bool>,
    pub(crate) toolbar_side_visible: Option<bool>,
    /// Top-strip form/minimize state before presenter mapped the strip to
    /// the micro chip (`[presenter_mode] toolbar_mode = "micro"`).
    pub(crate) toolbar_top_display_mode: Option<crate::config::TopDisplayMode>,
    pub(crate) toolbar_top_minimized: Option<bool>,
    pub(crate) click_highlight_enabled: Option<bool>,
    pub(crate) tool_override: Option<Option<Tool>>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LightModeRestore {
    pub(crate) show_status_bar: bool,
    pub(crate) show_tool_preview: bool,
    pub(crate) toolbar_visible: bool,
    pub(crate) toolbar_top_visible: bool,
    pub(crate) toolbar_side_visible: bool,
    pub(crate) click_highlight_enabled: bool,
    pub(crate) tool_override: Option<Tool>,
}

pub struct InputState {
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
    /// Current font size for text mode (from config)
    pub current_font_size: f64,
    /// Font descriptor for text rendering (family, weight, style)
    pub font_descriptor: FontDescriptor,
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
    /// Whether the display needs to be redrawn
    pub needs_redraw: bool,
    /// Whether session persistence should capture changes (cleared after autosave check)
    pub(crate) session_dirty: bool,
    /// Runtime session options used to preflight clone-heavy actions before mutation.
    pub(crate) session_preflight_options: Option<SessionOptions>,
    /// Save Session As target waiting for explicit overwrite confirmation.
    pub(crate) pending_save_as_overwrite: Option<PathBuf>,
    /// Whether the help overlay is currently visible (toggled with F10)
    pub show_help: bool,
    /// Active help overlay page index
    pub help_overlay_page: usize,
    /// Current help overlay search query
    pub help_overlay_search: String,
    /// Current help overlay scroll offset (pixels)
    pub help_overlay_scroll: f64,
    /// Max scrollable height for help overlay (pixels)
    pub help_overlay_scroll_max: f64,
    /// Help targets resolved under pending help-overlay presses, keyed by input
    /// modality so only the modality that owned a press can resolve it. Each
    /// matching release only runs a row when press and release land on the SAME
    /// target. Mirrors the toast press/release contract and guards destructive
    /// rows (e.g. Clear) against a press-drag-release that starts off-row and
    /// ends on the row.
    pub(crate) help_overlay_pending_presses: Vec<(
        crate::input::state::HelpOverlayPressSource,
        crate::input::state::HelpOverlayClick,
    )>,
    /// Help-owned presses whose overlay generation ended before physical
    /// release. Their eventual releases must still be swallowed, but can no
    /// longer resolve an action against either the old or a reopened layout.
    pub(crate) help_overlay_consume_only_presses: Vec<crate::input::state::HelpOverlayPressSource>,
    /// Board picker quick search query
    pub board_picker_search: String,
    /// Time of last board picker search input
    pub board_picker_search_last_input: Option<Instant>,
    /// Whether the command palette is currently visible
    pub command_palette_open: bool,
    /// Current command palette search query
    pub command_palette_query: String,
    /// Currently selected command index in the palette
    pub command_palette_selected: usize,
    /// Scroll offset for command palette (first visible item index)
    pub command_palette_scroll: usize,
    /// Held command-palette navigation key for synthetic repeat.
    pub(crate) command_palette_repeat_key: Option<Key>,
    /// Next synthetic command-palette repeat tick.
    pub(crate) command_palette_repeat_next_tick: Option<Instant>,
    /// Most recently executed command palette actions (most recent first)
    pub command_palette_recent: Vec<Action>,
    /// Whether the recents changed since the backend last persisted them.
    pub(crate) command_palette_recents_dirty: bool,
    /// Action whose next keyboard chord is being captured for rebinding.
    pub keybinding_capture_action: Option<Action>,
    /// Duration for command palette action toasts (ms)
    pub command_palette_toast_duration_ms: u64,
    /// Whether the status bar is currently visible (toggled via keybinding)
    pub show_status_bar: bool,
    /// Whether status HUD segments consume clicks to open their surfaces
    /// (`[ui] status_bar_interactive`); false keeps the bar display-only
    pub status_bar_interactive: bool,
    /// Whether to show the board label in the status bar
    pub show_status_board_badge: bool,
    /// Whether to show the page counter in the status bar
    pub show_status_page_badge: bool,
    /// Whether to show the hidden-toolbar hint chip in the status bar
    /// (`[ui] show_toolbar_hint`)
    pub show_toolbar_hint: bool,
    /// Whether to show the board/page badge when the status bar is visible
    pub show_floating_badge_always: bool,
    /// Whether the floating board/page badge may render at all (runtime
    /// toggle via `Action::ToggleFloatingBadge`; not persisted)
    pub show_floating_badge: bool,
    /// Whether the bottom-right zoom chip may render while
    /// `show_zoom_actions` is on (runtime toggle via
    /// `Action::ToggleZoomChip`; not persisted)
    pub show_zoom_chip: bool,
    /// Whether presenter mode is currently enabled
    pub presenter_mode: bool,
    /// Presenter mode behavior configuration
    pub presenter_mode_config: PresenterModeConfig,
    /// Configured render color profiles and active preview state.
    pub(crate) render_profiles: RenderProfileSet,
    /// Previous UI state to restore after presenter mode exits
    pub(crate) presenter_restore: Option<PresenterRestore>,
    /// Whether passthrough light mode is currently enabled
    pub light_mode: bool,
    /// Whether light mode is temporarily accepting drawing input
    pub light_mode_drawing: bool,
    /// Previous UI state to restore after light mode exits
    pub(crate) light_mode_restore: Option<LightModeRestore>,
    /// Whether both toolbars are visible (combined flag, prefer top/side specific)
    pub toolbar_visible: bool,
    /// Whether the top toolbar panel is visible
    pub toolbar_top_visible: bool,
    /// Whether the side toolbar panel is visible
    pub toolbar_side_visible: bool,
    /// Whether fill is enabled for fill-capable shapes (rect, ellipse)
    pub fill_enabled: bool,
    /// Current side count for regular polygon drawing.
    pub polygon_sides: u8,
    /// Whether the top toolbar is pinned (saved to config, opens at startup)
    pub toolbar_top_pinned: bool,
    /// Whether the side toolbar is pinned (saved to config, opens at startup)
    pub toolbar_side_pinned: bool,
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
    /// Whether the simple-mode shape picker is expanded
    pub toolbar_shapes_expanded: bool,
    /// Whether the top strip's overflow menu (width-dropped items) is open.
    pub toolbar_top_overflow_open: bool,
    /// Whether the Session popover (anchored to the top overflow toggle) is
    /// open. Mutually exclusive with the Settings popover and the overflow.
    pub toolbar_session_popover_open: bool,
    /// Whether the Settings popover (anchored to the top overflow toggle)
    /// is open. Mutually exclusive with the Session popover and the overflow.
    pub toolbar_settings_popover_open: bool,
    /// Whether the Canvas popover (anchored to the top overflow toggle) is
    /// open. Mutually exclusive with the Session/Settings popovers and the
    /// overflow.
    pub toolbar_canvas_popover_open: bool,
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
    /// Whether the side palette is minimized to its edge restore tab.
    pub toolbar_side_minimized: bool,
    /// Where the side-palette functions live. Under the default `Pill`
    /// layout the side surface never appears; the deprecated `Panel`
    /// escape hatch keeps the classic side palette. Startup init applies
    /// the config value; this struct field deliberately defaults to
    /// `Panel` so side-palette tests exercise the panel without setup.
    pub toolbar_side_layout: crate::config::ToolbarSideLayout,
    /// Modifier chord that turns a toolbar click into shortcut rebinding.
    /// Used to generate onboarding copy (the tour's rebind hint) without
    /// hardcoding key strings. Startup init applies the config value.
    pub toolbar_rebind_modifier: crate::config::ToolbarRebindModifier,
    /// Last HSV triple committed from the side palette's color picker;
    /// preserves hue/saturation across gray colors where RGB loses them.
    pub toolbar_picker_hsv: Option<(f64, f64, f64)>,
    /// Whether the toolbar drawer is open
    /// Active toolbar drawer tab
    pub toolbar_side_pane: crate::ui::toolbar::SidePane,
    /// Runtime scroll offsets per side-palette pane (Draw/Canvas/Session/Settings).
    pub toolbar_side_scroll: [f64; 4],
    /// Whether the Settings drawer is showing the toolbar item customization sub-panel
    pub toolbar_customize_items_open: bool,
    /// Selected toolbar item customization group in the Settings drawer sub-panel
    pub toolbar_customize_items_group: Option<crate::ui::toolbar::ToolbarItemCustomizeGroup>,
    /// Screen width in pixels (set by backend after configuration)
    pub screen_width: u32,
    /// Screen height in pixels (set by backend after configuration)
    pub screen_height: u32,
    /// Whether to show active output badge in status bar.
    pub show_active_output_badge: bool,
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
    /// Keybinding action map for efficient lookup
    pub(in crate::input::state::core) action_map: HashMap<KeyBinding, Action>,
    /// Ordered keybindings per action (as configured)
    pub(in crate::input::state::core) action_bindings: HashMap<Action, Vec<KeyBinding>>,
    /// Pending backend output action (to be handled by WaylandState)
    pub(in crate::input::state::core) pending_backend_action: Option<PendingBackendAction>,
    /// Pending output focus action (to be handled by WaylandState)
    pub(in crate::input::state::core) pending_output_focus_action: Option<OutputFocusAction>,
    /// Pending zoom action (to be handled by WaylandState)
    pub(in crate::input::state::core) pending_zoom_action: Option<ZoomAction>,
    /// Pending first-run onboarding usage markers to persist in onboarding store
    pub(crate) pending_onboarding_usage: PendingOnboardingUsage,
    /// Color snapshot for the newest pending copy-hex request.
    pub(crate) pending_copy_hex: Option<Color>,
    /// Destination owned by the newest pending paste-hex request.
    pub(crate) pending_paste_hex: Option<HexPasteTarget>,
    /// Maximum number of shapes allowed per frame (0 = unlimited)
    pub max_shapes_per_frame: usize,
    /// Click highlight animation state
    pub(crate) click_highlight: ClickHighlightState,
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
    /// Whether to show the delay sliders in Actions section
    pub show_delay_sliders: bool,
    /// Whether to show the marker opacity slider in the side toolbar
    pub show_marker_opacity_section: bool,
    /// Whether to show preset action toast notifications
    pub show_preset_toasts: bool,
    /// Whether to show the cursor tool preview bubble
    pub show_tool_preview: bool,
    /// Active (visible) UI toast (errors/warnings/info)
    pub(crate) ui_toast: Option<UiToastState>,
    /// Pending toasts waiting behind the active one, plus rate-limit memory
    pub(crate) toast_queue: ToastQueue,
    /// Cached bounds of the rendered toast for click detection (x, y, w, h)
    pub(crate) ui_toast_bounds: Option<(f64, f64, f64, f64)>,
    /// Copied selection shapes for paste operations
    pub(in crate::input::state::core) selection_clipboard: Option<Vec<Shape>>,
    /// Local clipboard generation for the copied shape selection.
    pub(in crate::input::state::core) selection_clipboard_generation: u64,
    /// System clipboard publication state for the current local selection.
    pub(in crate::input::state::core) selection_publish_state: SelectionPublishState,
    /// Per-process id embedded in private Wayscriber clipboard payloads.
    pub(in crate::input::state::core) clipboard_app_instance_id: String,
    /// Pending private selection clipboard publish request for the backend.
    pub(in crate::input::state::core) pending_selection_clipboard_publish:
        Option<PendingSelectionClipboardPublish>,
    /// Pending system clipboard paste request for the backend.
    pub(in crate::input::state::core) pending_clipboard_paste_request:
        Option<ClipboardPasteRequest>,
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
    /// Animation state for text edit mode entry (teal glow pulse)
    pub(crate) text_edit_entry_feedback: Option<TextEditEntryFeedback>,
    /// Pending delayed history playback state
    pub(in crate::input::state::core) pending_history: Option<DelayedHistory>,
    /// Cached layout details for the currently open context menu
    pub context_menu_layout: Option<ContextMenuLayout>,
    /// Cached layout details for the board picker overlay
    pub board_picker_layout: Option<BoardPickerLayout>,
    /// Cached layout details for the status HUD (segmented status bar)
    pub status_hud_layout: Option<crate::ui::StatusHudLayout>,
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
    /// Optional spatial index for accelerating hit-testing when many shapes are present
    pub(in crate::input::state::core) spatial_index: Option<SpatialGrid>,
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
    /// Pending toggle request for the backend (handled in the Wayland loop)
    pub(in crate::input::state::core) pending_frozen_toggle: bool,
    /// Screen-color eyedropper UI lifecycle.
    pub(in crate::input::state::core) eyedropper_ui_state:
        crate::input::state::core::EyedropperUiState,
    /// Pending eyedropper activation request for the Wayland backend.
    pub(in crate::input::state::core) pending_eyedropper_toggle: bool,
    /// Whether zoom mode is currently active
    pub(in crate::input::state::core) zoom_active: bool,
    /// Whether zoom view is locked
    pub(in crate::input::state::core) zoom_locked: bool,
    /// Current zoom scale (1.0 = no zoom)
    pub(in crate::input::state::core) zoom_scale: f64,
    /// Current zoom view offset in canvas/world space
    pub(in crate::input::state::core) zoom_view_offset: (f64, f64),
    /// Whether to show extended color palette
    pub show_more_colors: bool,
    /// Whether to show the Actions section (undo all, redo all, etc.)
    pub show_actions_section: bool,
    /// Whether to show advanced action buttons
    pub show_actions_advanced: bool,
    /// Whether to show zoom actions
    pub show_zoom_actions: bool,
    /// Whether to show the Pages section
    pub show_pages_section: bool,
    /// Whether to show the Boards section
    pub show_boards_section: bool,
    /// Whether to show the presets section
    pub show_presets: bool,
    /// Whether to show the Step Undo/Redo section
    pub show_step_section: bool,
    /// Whether to keep text controls visible when text is inactive
    pub show_text_controls: bool,
    /// Whether to enable context-aware UI that shows/hides controls based on active tool
    pub context_aware_ui: bool,
    /// Whether to show the Settings section
    pub show_settings_section: bool,
    /// Side drawer sections whose body content is collapsed for this runtime.
    pub toolbar_collapsed_side_sections: BTreeSet<ToolbarSideSection>,
    /// Number of preset slots to display
    pub preset_slot_count: usize,
    /// Preset slots for quick tool switching
    pub presets: Vec<Option<ToolPresetConfig>>,
    /// Last applied preset slot (for UI highlight)
    pub active_preset_slot: Option<usize>,
    /// Transient preset feedback for toolbar animations
    pub(crate) preset_feedback: Vec<Option<PresetFeedbackState>>,
    /// Pending preset save/clear action for backend persistence
    pub(in crate::input::state::core) pending_preset_action: Option<PresetAction>,
    /// Pending boards config update (persisted by backend)
    pub(in crate::input::state::core) pending_board_config: Option<PendingBoardConfigUpdate>,
    /// Ordered runtime UI actions for board pins and board identity changes.
    pub(in crate::input::state::core) pending_board_runtime_ui: Vec<PendingBoardRuntimeUiAction>,
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
    /// Whether the help overlay is in quick-reference mode
    pub help_overlay_quick_mode: bool,
    /// Cursor position within the help overlay search input
    #[allow(dead_code)]
    pub help_overlay_search_cursor: usize,
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
