use super::super::super::{
    index::SpatialGrid,
    menus::{ContextMenuLayout, ContextMenuState},
    properties::{PropertiesPanelLayout, ShapePropertiesPanel},
    selection::SelectionState,
};
use super::super::types::{
    DelayedHistory, DrawingState, PresetAction, PresetFeedbackState, SelectionAxis, TextClickState,
    TextInputMode, ToolbarDrawerTab, UiToastState, ZoomAction,
};
use crate::config::{Action, BoardConfig, KeyBinding, PresenterModeConfig, ToolPresetConfig};
use crate::draw::frame::ShapeSnapshot;
use crate::draw::{CanvasSet, Color, DirtyTracker, EraserKind, FontDescriptor, Shape, ShapeId};
use crate::input::state::highlight::ClickHighlightState;
use crate::input::{
    modifiers::Modifiers,
    tool::{EraserMode, Tool},
};
use crate::util::Rect;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PresenterRestore {
    pub(crate) show_status_bar: Option<bool>,
    pub(crate) show_tool_preview: Option<bool>,
    pub(crate) toolbar_visible: Option<bool>,
    pub(crate) toolbar_top_visible: Option<bool>,
    pub(crate) toolbar_side_visible: Option<bool>,
    pub(crate) click_highlight_enabled: Option<bool>,
    pub(crate) tool_override: Option<Option<Tool>>,
}

pub struct InputState {
    /// Multi-frame canvas management (transparent, whiteboard, blackboard)
    pub canvas_set: CanvasSet,
    /// Current drawing color (changed with color keys: R, G, B, etc.)
    pub current_color: Color,
    /// Current pen/line thickness in pixels (changed with +/- keys)
    pub current_thickness: f64,
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
    /// Current modifier key state
    pub modifiers: Modifiers,
    /// Current drawing mode state machine
    pub state: DrawingState,
    /// Whether user requested to exit the overlay
    pub should_exit: bool,
    /// Whether the display needs to be redrawn
    pub needs_redraw: bool,
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
    /// Whether the status bar is currently visible (toggled via keybinding)
    pub show_status_bar: bool,
    /// Whether presenter mode is currently enabled
    pub presenter_mode: bool,
    /// Presenter mode behavior configuration
    pub presenter_mode_config: PresenterModeConfig,
    /// Previous UI state to restore after presenter mode exits
    pub(crate) presenter_restore: Option<PresenterRestore>,
    /// Whether both toolbars are visible (combined flag, prefer top/side specific)
    pub toolbar_visible: bool,
    /// Whether the top toolbar panel is visible
    pub toolbar_top_visible: bool,
    /// Whether the side toolbar panel is visible
    pub toolbar_side_visible: bool,
    /// Whether fill is enabled for fill-capable shapes (rect, ellipse)
    pub fill_enabled: bool,
    /// Whether the top toolbar is pinned (saved to config, opens at startup)
    pub toolbar_top_pinned: bool,
    /// Whether the side toolbar is pinned (saved to config, opens at startup)
    pub toolbar_side_pinned: bool,
    /// Whether to use icons instead of text labels in toolbars
    pub toolbar_use_icons: bool,
    /// Current toolbar layout complexity
    pub toolbar_layout_mode: crate::config::ToolbarLayoutMode,
    /// Optional per-mode overrides for toolbar sections
    pub toolbar_mode_overrides: crate::config::ToolbarModeOverrides,
    /// Whether the simple-mode shape picker is expanded
    pub toolbar_shapes_expanded: bool,
    /// Whether the toolbar drawer is open
    pub toolbar_drawer_open: bool,
    /// Active toolbar drawer tab
    pub toolbar_drawer_tab: ToolbarDrawerTab,
    /// Screen width in pixels (set by backend after configuration)
    pub screen_width: u32,
    /// Screen height in pixels (set by backend after configuration)
    pub screen_height: u32,
    /// Previous color before entering board mode (for restoration)
    pub board_previous_color: Option<Color>,
    /// Board mode configuration
    pub board_config: BoardConfig,
    /// Tracks dirty regions between renders
    pub(crate) dirty_tracker: DirtyTracker,
    /// Cached bounds for the current provisional shape (if any)
    pub(crate) last_provisional_bounds: Option<Rect>,
    /// Cached bounds for live text preview/caret (if any)
    pub(crate) last_text_preview_bounds: Option<Rect>,
    /// Keybinding action map for efficient lookup
    pub(in crate::input::state::core) action_map: HashMap<KeyBinding, Action>,
    /// Pending capture action (to be handled by WaylandState)
    pub(in crate::input::state::core) pending_capture_action: Option<Action>,
    /// Pending zoom action (to be handled by WaylandState)
    pub(in crate::input::state::core) pending_zoom_action: Option<ZoomAction>,
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
    /// Whether context menu interactions are enabled
    pub(in crate::input::state::core) context_menu_enabled: bool,
    /// Cached hit-test bounds per shape id
    pub(in crate::input::state::core) hit_test_cache: HashMap<ShapeId, Rect>,
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
    /// Pending UI toast (errors/warnings/info)
    pub(crate) ui_toast: Option<UiToastState>,
    /// Cached bounds of the rendered toast for click detection (x, y, w, h)
    pub(crate) ui_toast_bounds: Option<(f64, f64, f64, f64)>,
    /// Copied selection shapes for paste operations
    pub(in crate::input::state::core) selection_clipboard: Option<Vec<Shape>>,
    /// Offset applied to successive paste operations
    pub(in crate::input::state::core) clipboard_paste_offset: i32,
    /// Last capture path (for quick open-folder action)
    pub(in crate::input::state::core) last_capture_path: Option<PathBuf>,
    /// Last text/note click used for double-click detection
    pub(crate) last_text_click: Option<TextClickState>,
    /// Tracks an in-progress text edit target (existing shape to replace)
    pub(crate) text_edit_target: Option<(ShapeId, ShapeSnapshot)>,
    /// Pending delayed history playback state
    pub(in crate::input::state::core) pending_history: Option<DelayedHistory>,
    /// Cached layout details for the currently open context menu
    pub context_menu_layout: Option<ContextMenuLayout>,
    /// Optional spatial index for accelerating hit-testing when many shapes are present
    pub(in crate::input::state::core) spatial_index: Option<SpatialGrid>,
    /// Last known pointer position (for keyboard anchors and hover refresh)
    pub(in crate::input::state::core) last_pointer_position: (i32, i32),
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
    /// Whether zoom mode is currently active
    pub(in crate::input::state::core) zoom_active: bool,
    /// Whether zoom view is locked
    pub(in crate::input::state::core) zoom_locked: bool,
    /// Current zoom scale (1.0 = no zoom)
    pub(in crate::input::state::core) zoom_scale: f64,
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
    /// Whether to show the presets section
    pub show_presets: bool,
    /// Whether to show the Step Undo/Redo section
    pub show_step_section: bool,
    /// Whether to keep text controls visible when text is inactive
    pub show_text_controls: bool,
    /// Whether to show the Settings section
    pub show_settings_section: bool,
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
    /// Whether the guided tour is currently active
    pub tour_active: bool,
    /// Current step in the guided tour (0-indexed)
    pub tour_step: usize,
}
