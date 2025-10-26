use crate::config::{Action, BoardConfig, KeyBinding};
use crate::draw::{CanvasSet, Color, FontDescriptor};
use crate::input::{board_mode::BoardMode, events::SystemCommand, modifiers::Modifiers};
use std::collections::HashMap;

use super::types::DrawingState;

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
    /// Screen width in pixels (set by backend after configuration)
    pub screen_width: u32,
    /// Screen height in pixels (set by backend after configuration)
    pub screen_height: u32,
    /// Previous color before entering board mode (for restoration)
    pub board_previous_color: Option<Color>,
    /// Board mode configuration
    pub board_config: BoardConfig,
    /// Keybinding action map for efficient lookup
    pub(super) action_map: HashMap<KeyBinding, Action>,
    /// Pending capture action (to be handled by WaylandState)
    pub(super) pending_capture_action: Option<Action>,
    /// Pending system command to be handled after the overlay exits
    pub(super) pending_system_command: Option<SystemCommand>,
    /// Whether exit requests are temporarily blocked while awaiting capture completion
    pub(super) capture_guard_active: bool,
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
        board_config: BoardConfig,
        action_map: HashMap<KeyBinding, Action>,
    ) -> Self {
        Self {
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
            screen_width: 0,
            screen_height: 0,
            board_previous_color: None,
            board_config,
            action_map,
            pending_capture_action: None,
            pending_system_command: None,
            capture_guard_active: false,
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

    /// Returns the current board mode.
    pub fn board_mode(&self) -> BoardMode {
        self.canvas_set.active_mode()
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

    /// Takes and clears any pending system command (e.g., launch configurator).
    pub fn take_pending_system_command(&mut self) -> Option<SystemCommand> {
        self.pending_system_command.take()
    }

    /// Marks that a capture is in progress so exit actions are deferred.
    pub fn begin_capture_guard(&mut self) {
        self.capture_guard_active = true;
        self.should_exit = false;
    }

    /// Clears the capture guard flag once capture completes or fails.
    pub fn end_capture_guard(&mut self) {
        self.capture_guard_active = false;
    }

    /// Returns true if exit requests are currently deferred by an in-progress capture.
    pub fn capture_guard_active(&self) -> bool {
        self.capture_guard_active
    }

    /// Requests an immediate exit regardless of capture guard state.
    pub fn force_exit(&mut self) {
        self.should_exit = true;
    }

    pub(super) fn handle_exit_request(&mut self, force: bool) {
        match &self.state {
            DrawingState::TextInput { .. } | DrawingState::Drawing { .. } => {
                self.state = DrawingState::Idle;
                self.needs_redraw = true;
            }
            DrawingState::Idle => {
                if force || !self.capture_guard_active {
                    self.should_exit = true;
                } else {
                    log::info!(
                        "Screenshot capture in progress - delay exit until capture completes"
                    );
                }
            }
        }
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
                    }
                }
                // Exiting board mode to transparent
                (BoardMode::Whiteboard | BoardMode::Blackboard, BoardMode::Transparent) => {
                    // Restore previous color if we saved one
                    if let Some(prev_color) = self.board_previous_color {
                        self.current_color = prev_color;
                        self.board_previous_color = None;
                    }
                }
                // Switching between board modes
                (BoardMode::Whiteboard, BoardMode::Blackboard)
                | (BoardMode::Blackboard, BoardMode::Whiteboard) => {
                    // Apply new board's default color
                    if let Some(default_color) = target_mode.default_pen_color(&self.board_config) {
                        self.current_color = default_color;
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
        self.needs_redraw = true;

        log::info!("Switched from {:?} to {:?} mode", current_mode, target_mode);
    }
}
