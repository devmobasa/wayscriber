//! Drawing state machine and input state management.

pub const MIN_STROKE_THICKNESS: f64 = 1.0;
pub const MAX_STROKE_THICKNESS: f64 = 40.0;

use super::{
    index::SpatialGrid,
    menus::{ContextMenuLayout, ContextMenuState},
    properties::ShapePropertiesPanel,
    selection::SelectionState,
};
use crate::config::{Action, BoardConfig, KeyBinding};
use crate::draw::frame::ShapeSnapshot;
use crate::draw::{CanvasSet, Color, DirtyTracker, FontDescriptor, ShapeId};
use crate::input::state::highlight::{ClickHighlightSettings, ClickHighlightState};
use crate::input::{modifiers::Modifiers, tool::Tool};
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
}

pub struct InputState {
    /// Multi-frame canvas management (transparent, whiteboard, blackboard)
    pub canvas_set: CanvasSet,
    /// Current drawing color (changed with color keys: R, G, B, etc.)
    pub current_color: Color,
    /// Current pen/line thickness in pixels (changed with +/- keys)
    pub current_thickness: f64,
    /// Current font size for text mode (from config)
    pub current_font_size: f64,
    /// Font descriptor for text rendering (family, weight, style)
    pub font_descriptor: FontDescriptor,
    /// Whether to draw background behind text
    pub text_background_enabled: bool,
    /// Arrowhead length in pixels (from config)
    pub arrow_length: f64,
    /// Arrowhead angle in degrees (from config)
    pub arrow_angle: f64,
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
    /// Maximum number of shapes allowed per frame (0 = unlimited)
    pub max_shapes_per_frame: usize,
    /// Click highlight animation state
    pub(crate) click_highlight: ClickHighlightState,
    /// Optional tool override independent of modifier keys
    pub(super) tool_override: Option<Tool>,
    /// Current selection information
    pub selection_state: SelectionState,
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
    /// Whether to show extended color palette
    pub show_more_colors: bool,
    /// Whether to show the Actions section (undo all, redo all, etc.)
    pub show_actions_section: bool,
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
    /// * `font_size` - Font size for text mode in points
    /// * `font_descriptor` - Font configuration for text rendering
    /// * `text_background_enabled` - Whether to draw background behind text
    /// * `arrow_length` - Arrowhead length in pixels
    /// * `arrow_angle` - Arrowhead angle in degrees
    /// * `show_status_bar` - Whether the status bar starts visible
    /// * `board_config` - Board mode configuration
    /// * `action_map` - Keybinding action map
    #[allow(clippy::too_many_arguments)]
    pub fn with_defaults(
        color: Color,
        thickness: f64,
        fill_enabled: bool,
        font_size: f64,
        font_descriptor: FontDescriptor,
        text_background_enabled: bool,
        arrow_length: f64,
        arrow_angle: f64,
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
        let mut state = Self {
            canvas_set: CanvasSet::new(),
            current_color: color,
            current_thickness: thickness,
            current_font_size: font_size,
            font_descriptor,
            text_background_enabled,
            arrow_length,
            arrow_angle,
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
            screen_width: 0,
            screen_height: 0,
            board_previous_color: None,
            board_config,
            dirty_tracker: DirtyTracker::new(),
            last_provisional_bounds: None,
            last_text_preview_bounds: None,
            action_map,
            pending_capture_action: None,
            max_shapes_per_frame,
            click_highlight: ClickHighlightState::new(click_highlight_settings),
            tool_override: None,
            selection_state: SelectionState::None,
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
            pending_history: None,
            context_menu_layout: None,
            spatial_index: None,
            last_pointer_position: (0, 0),
            pending_menu_hover_recalc: false,
            shape_properties_panel: None,
            frozen_active: false,
            pending_frozen_toggle: false,
            show_more_colors: false,
            show_actions_section: true, // Show by default
        };

        if state.click_highlight.uses_pen_color() {
            state.sync_highlight_color();
        }

        state
    }
}
