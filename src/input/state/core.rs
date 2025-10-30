//! Drawing state machine and input state management.

use super::highlight::{ClickHighlightSettings, ClickHighlightState};
use crate::config::{Action, BoardConfig, KeyBinding};
use crate::draw::{
    CanvasSet, Color, DirtyTracker, FontDescriptor,
    shape::{
        bounding_box_for_arrow, bounding_box_for_ellipse, bounding_box_for_line,
        bounding_box_for_points, bounding_box_for_rect, bounding_box_for_text,
    },
};
use crate::input::{board_mode::BoardMode, modifiers::Modifiers, tool::Tool};
use crate::legacy;
use crate::util::{self, Rect};
use std::collections::HashMap;
use std::process::{Command, Stdio};
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
}

/// Main input state containing all drawing session state.
///
/// This struct holds the current frame (all drawn shapes), drawing parameters,
/// modifier keys, drawing mode, and UI flags. It processes all keyboard and
/// mouse events to update the drawing state and determine when redraws are needed.
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
    tool_override: Option<Tool>,
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
        };

        if state.click_highlight.uses_pen_color() {
            state.sync_highlight_color();
        }

        state
    }

    pub(super) fn launch_configurator(&self) {
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

    /// Clears any cached provisional shape bounds and marks their damage region.
    pub(crate) fn clear_provisional_dirty(&mut self) {
        if let Some(prev) = self.last_provisional_bounds.take() {
            self.dirty_tracker.mark_rect(prev);
        }
    }

    /// Updates tracked provisional shape bounds for dirty-region purposes.
    pub(crate) fn update_provisional_dirty(&mut self, current_x: i32, current_y: i32) {
        let new_bounds = self.compute_provisional_bounds(current_x, current_y);
        let previous = self.last_provisional_bounds;

        if new_bounds != previous {
            if let Some(prev) = previous {
                self.dirty_tracker.mark_rect(prev);
            }
        }

        if let Some(bounds) = new_bounds {
            self.dirty_tracker.mark_rect(bounds);
            self.last_provisional_bounds = Some(bounds);
        } else {
            self.last_provisional_bounds = None;
        }
    }

    fn compute_provisional_bounds(&self, current_x: i32, current_y: i32) -> Option<Rect> {
        if let DrawingState::Drawing {
            tool,
            start_x,
            start_y,
            points,
        } = &self.state
        {
            match tool {
                Tool::Pen => bounding_box_for_points(points, self.current_thickness),
                Tool::Line => bounding_box_for_line(
                    *start_x,
                    *start_y,
                    current_x,
                    current_y,
                    self.current_thickness,
                ),
                Tool::Rect => {
                    let (x, w) = if current_x >= *start_x {
                        (*start_x, current_x - start_x)
                    } else {
                        (current_x, start_x - current_x)
                    };
                    let (y, h) = if current_y >= *start_y {
                        (*start_y, current_y - start_y)
                    } else {
                        (current_y, start_y - current_y)
                    };
                    bounding_box_for_rect(x, y, w, h, self.current_thickness)
                }
                Tool::Ellipse => {
                    let (cx, cy, rx, ry) =
                        util::ellipse_bounds(*start_x, *start_y, current_x, current_y);
                    bounding_box_for_ellipse(cx, cy, rx, ry, self.current_thickness)
                }
                Tool::Arrow => bounding_box_for_arrow(
                    *start_x,
                    *start_y,
                    current_x,
                    current_y,
                    self.current_thickness,
                    self.arrow_length,
                    self.arrow_angle,
                ),
                Tool::Highlight => None,
            }
        } else {
            None
        }
    }

    /// Updates dirty tracking for the live text preview/caret overlay.
    pub(crate) fn update_text_preview_dirty(&mut self) {
        let new_bounds = self.compute_text_preview_bounds();
        let previous = self.last_text_preview_bounds;

        if new_bounds != previous {
            if let Some(prev) = previous {
                self.dirty_tracker.mark_rect(prev);
            }
        }

        if let Some(bounds) = new_bounds {
            self.dirty_tracker.mark_rect(bounds);
            self.last_text_preview_bounds = Some(bounds);
        } else {
            self.last_text_preview_bounds = None;
        }
    }

    /// Clears the cached text preview bounds.
    pub(crate) fn clear_text_preview_dirty(&mut self) {
        if let Some(prev) = self.last_text_preview_bounds.take() {
            self.dirty_tracker.mark_rect(prev);
        }
    }

    fn compute_text_preview_bounds(&self) -> Option<Rect> {
        if let DrawingState::TextInput { x, y, buffer } = &self.state {
            let mut preview = buffer.clone();
            preview.push('_');
            bounding_box_for_text(
                *x,
                *y,
                &preview,
                self.current_font_size,
                &self.font_descriptor,
                self.text_background_enabled,
            )
        } else {
            None
        }
    }

    /// Returns the current board mode.
    pub fn board_mode(&self) -> BoardMode {
        self.canvas_set.active_mode()
    }

    /// Look up an action for the given key and modifiers.
    pub(super) fn find_action(&self, key_str: &str) -> Option<Action> {
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
    pub(super) fn set_pending_capture_action(&mut self, action: Action) {
        self.pending_capture_action = Some(action);
    }

    /// Returns whether the click highlight feature is currently enabled.
    pub fn click_highlight_enabled(&self) -> bool {
        self.click_highlight.enabled()
    }

    /// Toggle the click highlight feature and mark the frame for redraw.
    pub fn toggle_click_highlight(&mut self) -> bool {
        let enabled = self.click_highlight.toggle(&mut self.dirty_tracker);
        self.needs_redraw = true;
        enabled
    }

    /// Clears any active highlights without changing the enabled flag.
    pub fn clear_click_highlights(&mut self) {
        if self.click_highlight.has_active() {
            self.click_highlight.clear_all(&mut self.dirty_tracker);
            self.needs_redraw = true;
        }
    }

    /// Spawns a highlight at the given position if the feature is enabled.
    pub fn trigger_click_highlight(&mut self, x: i32, y: i32) {
        if self.click_highlight.spawn(x, y, &mut self.dirty_tracker) {
            self.needs_redraw = true;
        }
    }

    pub fn sync_highlight_color(&mut self) {
        if self.click_highlight.apply_pen_color(self.current_color) {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Advances highlight animations; returns true if highlights remain active.
    pub fn advance_click_highlights(&mut self, now: Instant) -> bool {
        self.click_highlight.advance(now, &mut self.dirty_tracker)
    }

    /// Render active highlights to the cairo context.
    pub fn render_click_highlights(&self, ctx: &cairo::Context, now: Instant) {
        self.click_highlight.render(ctx, now);
    }

    /// Returns the active tool considering overrides and drawing state.
    pub fn active_tool(&self) -> Tool {
        if let DrawingState::Drawing { tool, .. } = &self.state {
            *tool
        } else if let Some(tool) = self.tool_override {
            tool
        } else {
            self.modifiers.current_tool()
        }
    }

    /// Returns whether the highlight tool is currently selected.
    pub fn highlight_tool_active(&self) -> bool {
        matches!(self.tool_override, Some(Tool::Highlight))
            || matches!(
                self.state,
                DrawingState::Drawing {
                    tool: Tool::Highlight,
                    ..
                }
            )
    }

    /// Toggles highlight-only tool mode.
    pub fn toggle_highlight_tool(&mut self) -> bool {
        let enable = !self.highlight_tool_active();

        if enable {
            self.tool_override = Some(Tool::Highlight);
            // Ensure we are not mid-drawing with another tool
            if !matches!(
                self.state,
                DrawingState::Idle | DrawingState::TextInput { .. }
            ) {
                self.state = DrawingState::Idle;
            }
        } else {
            self.tool_override = None;
        }

        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        enable
    }

    /// Switches to a different board mode with color auto-adjustment.
    ///
    /// Handles mode transitions with automatic color adjustment for contrast:
    /// - Entering board mode: saves current color, applies mode default
    /// - Exiting board mode: restores previous color
    /// - Switching between boards: applies new mode default
    ///
    /// Also resets drawing state to prevent partial shapes crossing modes.
    pub fn switch_board_mode(&mut self, new_mode: BoardMode) {
        let current_mode = self.canvas_set.active_mode();

        // Toggle behavior: if already in target mode, return to transparent
        let target_mode = if current_mode == new_mode && new_mode != BoardMode::Transparent {
            BoardMode::Transparent
        } else {
            new_mode
        };

        // No-op if we're already in the target mode
        if current_mode == target_mode {
            return;
        }

        // Handle color auto-adjustment based on transition type (if enabled)
        if self.board_config.auto_adjust_pen {
            match (current_mode, target_mode) {
                // Entering board mode from transparent
                (BoardMode::Transparent, BoardMode::Whiteboard | BoardMode::Blackboard) => {
                    // Save current color and apply board default
                    self.board_previous_color = Some(self.current_color);
                    if let Some(default_color) = target_mode.default_pen_color(&self.board_config) {
                        self.current_color = default_color;
                        self.sync_highlight_color();
                    }
                }
                // Exiting board mode to transparent
                (BoardMode::Whiteboard | BoardMode::Blackboard, BoardMode::Transparent) => {
                    // Restore previous color if we saved one
                    if let Some(prev_color) = self.board_previous_color {
                        self.current_color = prev_color;
                        self.board_previous_color = None;
                        self.sync_highlight_color();
                    }
                }
                // Switching between board modes
                (BoardMode::Whiteboard, BoardMode::Blackboard)
                | (BoardMode::Blackboard, BoardMode::Whiteboard) => {
                    // Apply new board's default color
                    if let Some(default_color) = target_mode.default_pen_color(&self.board_config) {
                        self.current_color = default_color;
                        self.sync_highlight_color();
                    }
                }
                // All other transitions (shouldn't happen, but handle gracefully)
                _ => {}
            }
        }

        // Switch the active frame
        self.canvas_set.switch_mode(target_mode);

        // Reset drawing state to prevent partial shapes crossing modes
        self.state = DrawingState::Idle;

        // Trigger redraw
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;

        log::info!("Switched from {:?} to {:?} mode", current_mode, target_mode);
    }
}
