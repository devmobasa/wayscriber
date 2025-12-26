//! Drawing state machine and input state management.

pub const MIN_STROKE_THICKNESS: f64 = 1.0;
pub const MAX_STROKE_THICKNESS: f64 = 50.0;
pub const PRESET_FEEDBACK_DURATION_MS: u64 = 450;
pub const PRESET_TOAST_DURATION_MS: u64 = 1300;
pub const UI_TOAST_DURATION_MS: u64 = 5000;

use super::{
    index::SpatialGrid,
    menus::{ContextMenuLayout, ContextMenuState},
    properties::ShapePropertiesPanel,
    selection::SelectionState,
};
use crate::config::{Action, BoardConfig, KeyBinding, PRESET_SLOTS_MAX, ToolPresetConfig};
use crate::draw::frame::ShapeSnapshot;
use crate::draw::{CanvasSet, Color, DirtyTracker, EraserKind, FontDescriptor, ShapeId};
use crate::input::state::highlight::{ClickHighlightSettings, ClickHighlightState};
use crate::input::{
    modifiers::Modifiers,
    tool::{EraserMode, Tool},
};
use crate::util::Rect;
use std::collections::HashMap;
use std::time::Instant;
/// Current drawing mode state machine.
///
/// Tracks whether the user is idle, actively drawing a shape, or entering text.
/// State transitions occur based on mouse and keyboard events.
#[derive(Debug)]
pub enum DrawingState {
    /// Not actively drawing - waiting for user input
    Idle,
    /// Actively drawing a shape (mouse button held down)
    Drawing {
        /// Which tool is being used for this shape
        tool: Tool,
        /// Starting X coordinate (where mouse was pressed)
        start_x: i32,
        /// Starting Y coordinate (where mouse was pressed)
        start_y: i32,
        /// Accumulated points for freehand drawing
        points: Vec<(i32, i32)>,
    },
    /// Text input mode - user is typing text to place on screen
    TextInput {
        /// X coordinate where text will be placed
        x: i32,
        /// Y coordinate where text will be placed
        y: i32,
        /// Accumulated text buffer
        buffer: String,
    },
    /// Pending click on text/note to detect double-click editing
    PendingTextClick {
        /// Starting X coordinate
        x: i32,
        /// Starting Y coordinate
        y: i32,
        /// Active tool when the click began
        tool: Tool,
        /// Shape id that was clicked
        shape_id: ShapeId,
    },
    /// Selection move mode - user is dragging selected shapes
    MovingSelection {
        /// Last pointer X coordinate applied
        last_x: i32,
        /// Last pointer Y coordinate applied
        last_y: i32,
        /// Snapshots of shapes prior to movement (for undo/cancel)
        snapshots: Vec<(ShapeId, ShapeSnapshot)>,
        /// Whether any translation has been applied
        moved: bool,
    },
    /// Resize text/note wrap width by dragging a handle
    ResizingText {
        /// Shape id being resized
        shape_id: ShapeId,
        /// Snapshot of the shape prior to resizing (for undo/cancel)
        snapshot: ShapeSnapshot,
        /// Text baseline X coordinate (wrap width is measured from here)
        base_x: i32,
        /// Font size used to set minimum width
        size: f64,
    },
}

/// Describes which kind of text input is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputMode {
    Plain,
    StickyNote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomAction {
    In,
    Out,
    Reset,
    ToggleLock,
    RefreshCapture,
}

#[derive(Debug, Clone)]
pub enum PresetAction {
    Save {
        slot: usize,
        preset: ToolPresetConfig,
    },
    Clear {
        slot: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetFeedbackKind {
    Apply,
    Save,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiToastKind {
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub(crate) struct PresetFeedbackState {
    pub kind: PresetFeedbackKind,
    pub started: Instant,
}

#[derive(Debug, Clone)]
pub(crate) struct UiToastState {
    pub kind: UiToastKind,
    pub message: String,
    pub started: Instant,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TextClickState {
    pub shape_id: ShapeId,
    pub x: i32,
    pub y: i32,
    pub at: Instant,
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
    /// Whether the status bar is currently visible (toggled via keybinding)
    pub show_status_bar: bool,
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
    pub(super) action_map: HashMap<KeyBinding, Action>,
    /// Pending capture action (to be handled by WaylandState)
    pub(super) pending_capture_action: Option<Action>,
    /// Pending zoom action (to be handled by WaylandState)
    pub(super) pending_zoom_action: Option<ZoomAction>,
    /// Maximum number of shapes allowed per frame (0 = unlimited)
    pub max_shapes_per_frame: usize,
    /// Click highlight animation state
    pub(crate) click_highlight: ClickHighlightState,
    /// Optional tool override independent of modifier keys
    pub(super) tool_override: Option<Tool>,
    /// Current selection information
    pub selection_state: SelectionState,
    /// Last axis used for selection nudges (used to resolve Home/End axis)
    pub last_selection_axis: Option<SelectionAxis>,
    /// Current context menu state
    pub context_menu_state: ContextMenuState,
    /// Whether context menu interactions are enabled
    pub(super) context_menu_enabled: bool,
    /// Cached hit-test bounds per shape id
    pub(super) hit_test_cache: HashMap<ShapeId, Rect>,
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
    /// Last text/note click used for double-click detection
    pub(crate) last_text_click: Option<TextClickState>,
    /// Tracks an in-progress text edit target (existing shape to replace)
    pub(crate) text_edit_target: Option<(ShapeId, ShapeSnapshot)>,
    /// Pending delayed history playback state
    pub(super) pending_history: Option<DelayedHistory>,
    /// Cached layout details for the currently open context menu
    pub context_menu_layout: Option<ContextMenuLayout>,
    /// Optional spatial index for accelerating hit-testing when many shapes are present
    pub(super) spatial_index: Option<SpatialGrid>,
    /// Last known pointer position (for keyboard anchors and hover refresh)
    pub(super) last_pointer_position: (i32, i32),
    /// Recompute hover next time layout is available
    pub(super) pending_menu_hover_recalc: bool,
    /// Optional properties panel describing the current selection
    pub(super) shape_properties_panel: Option<ShapePropertiesPanel>,
    /// Whether frozen mode is currently active
    pub(super) frozen_active: bool,
    /// Pending toggle request for the backend (handled in the Wayland loop)
    pub(super) pending_frozen_toggle: bool,
    /// Whether zoom mode is currently active
    pub(super) zoom_active: bool,
    /// Whether zoom view is locked
    pub(super) zoom_locked: bool,
    /// Current zoom scale (1.0 = no zoom)
    pub(super) zoom_scale: f64,
    /// Whether to show extended color palette
    pub show_more_colors: bool,
    /// Whether to show the Actions section (undo all, redo all, etc.)
    pub show_actions_section: bool,
    /// Whether to show advanced action buttons
    pub show_actions_advanced: bool,
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
    pub(super) pending_preset_action: Option<PresetAction>,
}

/// Tracks in-progress delayed undo/redo playback.
pub(super) struct DelayedHistory {
    pub mode: HistoryMode,
    pub remaining: usize,
    pub delay_ms: u64,
    pub next_due: Instant,
}

#[derive(Clone, Copy)]
pub(super) enum HistoryMode {
    Undo,
    Redo,
}

impl InputState {
    /// Creates a new InputState with specified defaults.
    ///
    /// Screen dimensions default to 0 and should be updated by the backend
    /// after surface configuration (see `update_screen_dimensions`).
    ///
    /// # Arguments
    /// * `color` - Initial drawing color
    /// * `thickness` - Initial pen thickness in pixels
    /// * `eraser_size` - Initial eraser size in pixels
    /// * `eraser_mode` - Initial eraser behavior mode
    /// * `font_size` - Font size for text mode in points
    /// * `font_descriptor` - Font configuration for text rendering
    /// * `text_background_enabled` - Whether to draw background behind text
    /// * `arrow_length` - Arrowhead length in pixels
    /// * `arrow_angle` - Arrowhead angle in degrees
    /// * `arrow_head_at_end` - Whether arrowhead is drawn at the end
    /// * `show_status_bar` - Whether the status bar starts visible
    /// * `board_config` - Board mode configuration
    /// * `action_map` - Keybinding action map
    #[allow(clippy::too_many_arguments)]
    pub fn with_defaults(
        color: Color,
        thickness: f64,
        eraser_size: f64,
        eraser_mode: EraserMode,
        marker_opacity: f64,
        fill_enabled: bool,
        font_size: f64,
        font_descriptor: FontDescriptor,
        text_background_enabled: bool,
        arrow_length: f64,
        arrow_angle: f64,
        arrow_head_at_end: bool,
        show_status_bar: bool,
        board_config: BoardConfig,
        action_map: HashMap<KeyBinding, Action>,
        max_shapes_per_frame: usize,
        click_highlight_settings: ClickHighlightSettings,
        undo_all_delay_ms: u64,
        redo_all_delay_ms: u64,
        custom_section_enabled: bool,
        custom_undo_delay_ms: u64,
        custom_redo_delay_ms: u64,
        custom_undo_steps: usize,
        custom_redo_steps: usize,
    ) -> Self {
        let clamped_eraser = eraser_size.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        let mut state = Self {
            canvas_set: CanvasSet::new(),
            current_color: color,
            current_thickness: thickness,
            eraser_size: clamped_eraser,
            eraser_kind: EraserKind::Circle,
            eraser_mode,
            marker_opacity,
            current_font_size: font_size,
            font_descriptor,
            text_background_enabled,
            text_wrap_width: None,
            text_input_mode: TextInputMode::Plain,
            arrow_length,
            arrow_angle,
            arrow_head_at_end,
            modifiers: Modifiers::new(),
            state: DrawingState::Idle,
            should_exit: false,
            needs_redraw: true,
            show_help: false,
            show_status_bar,
            toolbar_visible: false,
            toolbar_top_visible: false,
            toolbar_side_visible: false,
            fill_enabled,
            toolbar_top_pinned: false,
            toolbar_side_pinned: false,
            toolbar_use_icons: true, // Default to icon mode
            toolbar_layout_mode: crate::config::ToolbarLayoutMode::Regular,
            toolbar_mode_overrides: crate::config::ToolbarModeOverrides::default(),
            toolbar_shapes_expanded: false,
            screen_width: 0,
            screen_height: 0,
            board_previous_color: None,
            board_config,
            dirty_tracker: DirtyTracker::new(),
            last_provisional_bounds: None,
            last_text_preview_bounds: None,
            action_map,
            pending_capture_action: None,
            pending_zoom_action: None,
            max_shapes_per_frame,
            click_highlight: ClickHighlightState::new(click_highlight_settings),
            tool_override: None,
            selection_state: SelectionState::None,
            last_selection_axis: None,
            context_menu_state: ContextMenuState::Hidden,
            context_menu_enabled: true,
            hit_test_cache: HashMap::new(),
            hit_test_tolerance: 6.0,
            max_linear_hit_test: 400,
            undo_stack_limit: 100,
            undo_all_delay_ms,
            redo_all_delay_ms,
            custom_undo_delay_ms,
            custom_redo_delay_ms,
            custom_undo_steps,
            custom_redo_steps,
            custom_section_enabled,
            show_delay_sliders: false, // Default to hidden
            show_marker_opacity_section: false,
            show_preset_toasts: true,
            show_tool_preview: false,
            ui_toast: None,
            last_text_click: None,
            text_edit_target: None,
            pending_history: None,
            context_menu_layout: None,
            spatial_index: None,
            last_pointer_position: (0, 0),
            pending_menu_hover_recalc: false,
            shape_properties_panel: None,
            frozen_active: false,
            pending_frozen_toggle: false,
            zoom_active: false,
            zoom_locked: false,
            zoom_scale: 1.0,
            show_more_colors: false,
            show_actions_section: true, // Show by default
            show_actions_advanced: false,
            show_presets: true,
            show_step_section: false,
            show_text_controls: false,
            show_settings_section: true,
            preset_slot_count: PRESET_SLOTS_MAX,
            presets: vec![None; PRESET_SLOTS_MAX],
            active_preset_slot: None,
            preset_feedback: vec![None; PRESET_SLOTS_MAX],
            pending_preset_action: None,
        };

        if state.click_highlight.uses_pen_color() {
            state.sync_highlight_color();
        }

        state
    }

    /// Resets all tracked keyboard modifiers to the "released" state.
    ///
    /// This is used as a safety net when external UI (portals, other windows)
    /// or focus transitions may cause us to miss key release events from
    /// the compositor, which would otherwise leave modifiers "stuck" and break
    /// shortcut handling and tool selection.
    pub fn reset_modifiers(&mut self) {
        self.modifiers.shift = false;
        self.modifiers.ctrl = false;
        self.modifiers.alt = false;
        self.modifiers.tab = false;
    }

    /// Synchronize modifier state from backend-provided values (e.g. compositor).
    ///
    /// This lets us correct cases where a key release event was missed but the compositor's
    /// authoritative modifier state is still accurate.
    pub fn sync_modifiers(&mut self, shift: bool, ctrl: bool, alt: bool) {
        self.modifiers.shift = shift;
        self.modifiers.ctrl = ctrl;
        self.modifiers.alt = alt;
        // Tab has no direct compositor flag; leave it unchanged.
    }
}
