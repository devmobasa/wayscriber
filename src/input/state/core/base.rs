//! Drawing state machine and input state management.

use super::{
    index::SpatialGrid,
    menus::{ContextMenuLayout, ContextMenuState},
    properties::ShapePropertiesPanel,
    selection::SelectionState,
};
use crate::config::{Action, BoardConfig, KeyBinding};
use crate::draw::frame::UndoAction;
use crate::draw::{CanvasSet, Color, DirtyTracker, FontDescriptor, ShapeId};
use crate::input::state::highlight::{ClickHighlightSettings, ClickHighlightState};
use crate::input::{modifiers::Modifiers, tool::Tool};
use crate::legacy;
use crate::util::Rect;
use std::collections::HashMap;
use std::process::{Command, Stdio};

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
    action_map: HashMap<KeyBinding, Action>,
    /// Pending capture action (to be handled by WaylandState)
    pending_capture_action: Option<Action>,
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
            context_menu_layout: None,
            spatial_index: None,
            last_pointer_position: (0, 0),
            pending_menu_hover_recalc: false,
            shape_properties_panel: None,
        };

        if state.click_highlight.uses_pen_color() {
            state.sync_highlight_color();
        }

        state
    }

    /// Updates the cached pointer location.
    pub fn update_pointer_position(&mut self, x: i32, y: i32) {
        self.last_pointer_position = (x, y);
    }

    /// Updates the undo stack limit for subsequent actions.
    pub fn set_undo_stack_limit(&mut self, limit: usize) {
        self.undo_stack_limit = limit.max(1);
    }

    pub fn apply_action_side_effects(&mut self, action: &UndoAction) {
        self.invalidate_hit_cache_from_action(action);
        self.mark_dirty_from_action(action);
        self.clear_selection();
        self.needs_redraw = true;
    }

    fn mark_dirty_from_action(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } | UndoAction::Delete { shapes } => {
                for (_, shape) in shapes {
                    self.dirty_tracker.mark_shape(&shape.shape);
                }
            }
            UndoAction::Modify {
                before,
                after,
                shape_id,
                ..
            } => {
                self.dirty_tracker.mark_shape(&before.shape);
                self.dirty_tracker.mark_shape(&after.shape);
                self.invalidate_hit_cache_for(*shape_id);
            }
            UndoAction::Reorder { shape_id, .. } => {
                if let Some(shape) = self.canvas_set.active_frame().shape(*shape_id) {
                    self.dirty_tracker.mark_shape(&shape.shape);
                    self.invalidate_hit_cache_for(*shape_id);
                }
            }
            UndoAction::Compound(actions) => {
                for action in actions {
                    self.mark_dirty_from_action(action);
                }
            }
        }
    }

    fn invalidate_hit_cache_from_action(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } | UndoAction::Delete { shapes } => {
                for (_, shape) in shapes {
                    self.invalidate_hit_cache_for(shape.id);
                }
            }
            UndoAction::Modify { shape_id, .. } => {
                self.invalidate_hit_cache_for(*shape_id);
            }
            UndoAction::Reorder { shape_id, .. } => {
                self.invalidate_hit_cache_for(*shape_id);
            }
            UndoAction::Compound(actions) => {
                for action in actions {
                    self.invalidate_hit_cache_from_action(action);
                }
            }
        }
    }

    pub(crate) fn launch_configurator(&self) {
        let binary = legacy::configurator_override()
            .unwrap_or_else(|| "wayscriber-configurator".to_string());

        match Command::new(&binary)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                log::info!(
                    "Launched wayscriber-configurator (binary: {binary}, pid: {})",
                    child.id()
                );
            }
            Err(err) => {
                log::error!("Failed to launch wayscriber-configurator using '{binary}': {err}");
                log::error!(
                    "Set WAYSCRIBER_CONFIGURATOR (or legacy HYPRMARKER_CONFIGURATOR) to override the executable path if needed."
                );
            }
        }
    }

    /// Updates screen dimensions after backend configuration.
    ///
    /// This should be called by the backend when it receives the actual
    /// screen dimensions from the display server.
    ///
    /// # Arguments
    /// * `width` - Screen width in pixels
    /// * `height` - Screen height in pixels
    pub fn update_screen_dimensions(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Drains pending dirty rectangles for the current surface size.
    pub fn take_dirty_regions(&mut self) -> Vec<Rect> {
        let width = self.screen_width.min(i32::MAX as u32) as i32;
        let height = self.screen_height.min(i32::MAX as u32) as i32;
        self.dirty_tracker.take_regions(width, height)
    }

    /// Look up an action for the given key and modifiers.
    pub(crate) fn find_action(&self, key_str: &str) -> Option<Action> {
        // Try to find a matching keybinding
        for (binding, action) in &self.action_map {
            if binding.matches(
                key_str,
                self.modifiers.ctrl,
                self.modifiers.shift,
                self.modifiers.alt,
            ) {
                return Some(*action);
            }
        }
        None
    }

    /// Adjusts the current font size by a delta, clamping to valid range.
    ///
    /// Font size is clamped to 8.0-72.0px range (same as config validation).
    /// Triggers a redraw to update the status bar display.
    ///
    /// # Arguments
    /// * `delta` - Amount to adjust font size (positive to increase, negative to decrease)
    pub fn adjust_font_size(&mut self, delta: f64) {
        self.current_font_size = (self.current_font_size + delta).clamp(8.0, 72.0);
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        log::debug!("Font size adjusted to {:.1}px", self.current_font_size);
    }

    /// Takes and clears any pending capture action.
    ///
    /// This is called by WaylandState to retrieve capture actions that need
    /// to be handled with access to CaptureManager.
    ///
    /// # Returns
    /// The pending capture action if any, None otherwise
    pub fn take_pending_capture_action(&mut self) -> Option<Action> {
        self.pending_capture_action.take()
    }

    /// Stores a capture action for retrieval by the backend.
    pub(crate) fn set_pending_capture_action(&mut self, action: Action) {
        self.pending_capture_action = Some(action);
    }
}
