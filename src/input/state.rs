//! Drawing state machine and input state management.

use super::board_mode::BoardMode;
use super::events::{Key, MouseButton};
use super::modifiers::Modifiers;
use super::tool::Tool;
use crate::config::{Action, BoardConfig, KeyBinding};
use crate::draw::{CanvasSet, Color, FontDescriptor, Shape};
use crate::legacy;
use crate::util;
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
    action_map: HashMap<KeyBinding, Action>,
    /// Pending capture action (to be handled by WaylandState)
    pending_capture_action: Option<Action>,
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
        }
    }

    fn launch_configurator(&self) {
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

    /// Returns the current board mode.
    pub fn board_mode(&self) -> BoardMode {
        self.canvas_set.active_mode()
    }

    /// Look up an action for the given key and modifiers.
    fn find_action(&self, key_str: &str) -> Option<Action> {
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

    /// Processes a key press event.
    ///
    /// Handles all keyboard input including:
    /// - Drawing color selection (configurable keybindings)
    /// - Tool actions (text mode, clear, undo - configurable)
    /// - Text input (when in TextInput state)
    /// - Exit commands (configurable)
    /// - Thickness adjustment (configurable)
    /// - Help toggle (configurable)
    /// - Modifier key tracking
    pub fn on_key_press(&mut self, key: Key) {
        // Handle modifier keys first
        match key {
            Key::Shift => {
                self.modifiers.shift = true;
                return;
            }
            Key::Ctrl => {
                self.modifiers.ctrl = true;
                return;
            }
            Key::Alt => {
                self.modifiers.alt = true;
                return;
            }
            Key::Tab => {
                self.modifiers.tab = true;
                return;
            }
            _ => {}
        }

        // In text input mode, only check actions if modifiers are pressed or it's a special key
        // This allows plain letters to be typed without triggering color/tool actions
        if matches!(&self.state, DrawingState::TextInput { .. }) {
            // Only check for actions if:
            // 1. Modifiers are held (Ctrl, Alt, Shift for special commands)
            // 2. OR it's a special non-character key (Escape, F10, etc.)
            let should_check_actions = match key {
                // Special keys always check for actions
                Key::Escape | Key::F10 | Key::F11 | Key::Return => true,
                // Character keys only check if modifiers are held
                Key::Char(_) => self.modifiers.ctrl || self.modifiers.alt,
                // Other keys can check as well
                _ => self.modifiers.ctrl || self.modifiers.alt,
            };

            if should_check_actions {
                // Convert key to string for action lookup
                let key_str = match key {
                    Key::Char(c) => c.to_string(),
                    Key::Escape => "Escape".to_string(),
                    Key::Return => "Return".to_string(),
                    Key::Backspace => "Backspace".to_string(),
                    Key::Space => "Space".to_string(),
                    Key::Plus => "+".to_string(),
                    Key::Minus => "-".to_string(),
                    Key::Equals => "=".to_string(),
                    Key::Underscore => "_".to_string(),
                    Key::F10 => "F10".to_string(),
                    Key::F11 => "F11".to_string(),
                    _ => String::new(),
                };

                // Check if this key combination triggers an action
                if !key_str.is_empty()
                    && let Some(action) = self.find_action(&key_str)
                {
                    // Actions work in text mode
                    // Note: Exit action has special logic in handle_action - it cancels
                    // text mode if in TextInput state, or exits app if in Idle state
                    self.handle_action(action);
                    return;
                }
            }

            // No action triggered, handle as text input
            // Handle Return key for finalizing text input (only plain Return, not Shift+Return)
            if matches!(key, Key::Return)
                && !self.modifiers.shift
                && let DrawingState::TextInput { x, y, buffer } = &self.state
            {
                if !buffer.is_empty() {
                    let x = *x;
                    let y = *y;
                    let text = buffer.clone();

                    self.canvas_set.active_frame_mut().add_shape(Shape::Text {
                        x,
                        y,
                        text,
                        color: self.current_color,
                        size: self.current_font_size,
                        font_descriptor: self.font_descriptor.clone(),
                        background_enabled: self.text_background_enabled,
                    });
                    self.needs_redraw = true;
                }
                self.state = DrawingState::Idle;
                return;
            }

            // Regular text input - add character to buffer
            if let DrawingState::TextInput { buffer, .. } = &mut self.state {
                match key {
                    Key::Char(c) => {
                        buffer.push(c);
                        self.needs_redraw = true;
                        return;
                    }
                    Key::Backspace => {
                        buffer.pop();
                        self.needs_redraw = true;
                        return;
                    }
                    Key::Space => {
                        buffer.push(' ');
                        self.needs_redraw = true;
                        return;
                    }
                    Key::Return if self.modifiers.shift => {
                        // Shift+Enter: insert newline
                        buffer.push('\n');
                        self.needs_redraw = true;
                        return;
                    }
                    _ => {
                        // Ignore other keys in text mode
                        return;
                    }
                }
            }
        }

        // Handle Escape in Drawing state for canceling
        if matches!(key, Key::Escape)
            && let DrawingState::Drawing { .. } = &self.state
            && let Some(Action::Exit) = self.find_action("Escape")
        {
            self.state = DrawingState::Idle;
            self.needs_redraw = true;
            return;
        }

        // Convert key to string for action lookup
        let key_str = match key {
            Key::Char(c) => c.to_string(),
            Key::Escape => "Escape".to_string(),
            Key::Return => "Return".to_string(),
            Key::Backspace => "Backspace".to_string(),
            Key::Space => "Space".to_string(),
            Key::Plus => "+".to_string(),
            Key::Minus => "-".to_string(),
            Key::Equals => "=".to_string(),
            Key::Underscore => "_".to_string(),
            Key::F10 => "F10".to_string(),
            Key::F11 => "F11".to_string(),
            _ => return,
        };

        // Look up action based on keybinding
        if let Some(action) = self.find_action(&key_str) {
            self.handle_action(action);
        }
    }

    /// Handle an action triggered by a keybinding.
    fn handle_action(&mut self, action: Action) {
        match action {
            Action::Exit => {
                // Exit drawing mode or cancel current action
                match &self.state {
                    DrawingState::TextInput { .. } | DrawingState::Drawing { .. } => {
                        // Cancel current action
                        self.state = DrawingState::Idle;
                        self.needs_redraw = true;
                    }
                    DrawingState::Idle => {
                        // Exit application
                        self.should_exit = true;
                    }
                }
            }
            Action::EnterTextMode => {
                if matches!(self.state, DrawingState::Idle) {
                    self.state = DrawingState::TextInput {
                        x: (self.screen_width / 2) as i32,
                        y: (self.screen_height / 2) as i32,
                        buffer: String::new(),
                    };
                    self.needs_redraw = true;
                }
            }
            Action::ClearCanvas => {
                self.canvas_set.clear_active();
                self.needs_redraw = true;
            }
            Action::Undo => {
                if self.canvas_set.active_frame_mut().undo() {
                    self.needs_redraw = true;
                }
            }
            Action::IncreaseThickness => {
                self.current_thickness = (self.current_thickness + 1.0).min(20.0);
                self.needs_redraw = true;
            }
            Action::DecreaseThickness => {
                self.current_thickness = (self.current_thickness - 1.0).max(1.0);
                self.needs_redraw = true;
            }
            Action::IncreaseFontSize => {
                self.adjust_font_size(2.0);
            }
            Action::DecreaseFontSize => {
                self.adjust_font_size(-2.0);
            }
            Action::ToggleWhiteboard => {
                if self.board_config.enabled {
                    log::info!("Toggling whiteboard mode");
                    self.switch_board_mode(BoardMode::Whiteboard);
                }
            }
            Action::ToggleBlackboard => {
                if self.board_config.enabled {
                    log::info!("Toggling blackboard mode");
                    self.switch_board_mode(BoardMode::Blackboard);
                }
            }
            Action::ReturnToTransparent => {
                if self.board_config.enabled {
                    log::info!("Returning to transparent mode");
                    self.switch_board_mode(BoardMode::Transparent);
                }
            }
            Action::ToggleHelp => {
                self.show_help = !self.show_help;
                self.needs_redraw = true;
            }
            Action::OpenConfigurator => {
                self.launch_configurator();
            }
            Action::SetColorRed => {
                self.current_color = util::key_to_color('r').unwrap();
                self.needs_redraw = true;
            }
            Action::SetColorGreen => {
                self.current_color = util::key_to_color('g').unwrap();
                self.needs_redraw = true;
            }
            Action::SetColorBlue => {
                self.current_color = util::key_to_color('b').unwrap();
                self.needs_redraw = true;
            }
            Action::SetColorYellow => {
                self.current_color = util::key_to_color('y').unwrap();
                self.needs_redraw = true;
            }
            Action::SetColorOrange => {
                self.current_color = util::key_to_color('o').unwrap();
                self.needs_redraw = true;
            }
            Action::SetColorPink => {
                self.current_color = util::key_to_color('p').unwrap();
                self.needs_redraw = true;
            }
            Action::SetColorWhite => {
                self.current_color = util::key_to_color('w').unwrap();
                self.needs_redraw = true;
            }
            Action::SetColorBlack => {
                self.current_color = util::key_to_color('k').unwrap();
                self.needs_redraw = true;
            }
            Action::CaptureFullScreen
            | Action::CaptureActiveWindow
            | Action::CaptureSelection
            | Action::CaptureClipboardFull
            | Action::CaptureFileFull
            | Action::CaptureClipboardSelection
            | Action::CaptureFileSelection
            | Action::CaptureClipboardRegion
            | Action::CaptureFileRegion => {
                // Capture actions are handled externally by WaylandState
                // since they require access to CaptureManager
                // Store the action for later retrieval
                log::debug!("Capture action {:?} pending for backend", action);
                self.pending_capture_action = Some(action);

                // Clear modifiers to prevent them from being "stuck" after capture
                // (portal dialog causes key releases to be missed)
                self.modifiers.ctrl = false;
                self.modifiers.shift = false;
                self.modifiers.alt = false;
            }
        }
    }

    /// Processes a key release event.
    ///
    /// Currently only tracks modifier key releases to update the modifier state.
    pub fn on_key_release(&mut self, key: Key) {
        match key {
            Key::Shift => self.modifiers.shift = false,
            Key::Ctrl => self.modifiers.ctrl = false,
            Key::Alt => self.modifiers.alt = false,
            Key::Tab => self.modifiers.tab = false,
            _ => {}
        }
    }

    /// Processes a mouse button press event.
    ///
    /// # Arguments
    /// * `button` - Which mouse button was pressed
    /// * `x` - Mouse X coordinate
    /// * `y` - Mouse Y coordinate
    ///
    /// # Behavior
    /// - Left click while Idle: Starts drawing with the current tool (based on modifiers)
    /// - Left click during TextInput: Updates text position
    /// - Right click: Cancels current action
    pub fn on_mouse_press(&mut self, button: MouseButton, x: i32, y: i32) {
        match button {
            MouseButton::Left => {
                // Start drawing with current tool
                if matches!(self.state, DrawingState::Idle) {
                    let tool = self.modifiers.current_tool();
                    self.state = DrawingState::Drawing {
                        tool,
                        start_x: x,
                        start_y: y,
                        points: vec![(x, y)],
                    };
                    self.needs_redraw = true;
                } else if let DrawingState::TextInput { x: tx, y: ty, .. } = &mut self.state {
                    // Update text position if in text mode
                    *tx = x;
                    *ty = y;
                    self.needs_redraw = true;
                }
            }
            MouseButton::Right => {
                // Right-click could cancel or exit
                if !matches!(self.state, DrawingState::Idle) {
                    self.state = DrawingState::Idle;
                    self.needs_redraw = true;
                }
            }
            _ => {}
        }
    }

    /// Processes mouse motion (dragging) events.
    ///
    /// # Arguments
    /// * `x` - Current mouse X coordinate
    /// * `y` - Current mouse Y coordinate
    ///
    /// # Behavior
    /// - When drawing with Pen tool: Adds points to the freehand stroke
    /// - When drawing with other tools: Triggers redraw for live preview
    pub fn on_mouse_motion(&mut self, x: i32, y: i32) {
        if let DrawingState::Drawing { tool, points, .. } = &mut self.state {
            if *tool == Tool::Pen {
                // Add point to freehand stroke
                points.push((x, y));
            }
            // For other tools, we'll update the end point in release
            self.needs_redraw = true;
        }
    }

    /// Processes mouse button release events.
    ///
    /// # Arguments
    /// * `button` - Which mouse button was released
    /// * `x` - Mouse X coordinate at release
    /// * `y` - Mouse Y coordinate at release
    ///
    /// # Behavior
    /// When left button is released during drawing:
    /// - Finalizes the shape using start position and current position
    /// - Adds the completed shape to the frame
    /// - Returns to Idle state
    pub fn on_mouse_release(&mut self, button: MouseButton, x: i32, y: i32) {
        if button != MouseButton::Left {
            return;
        }

        if let DrawingState::Drawing {
            tool,
            start_x,
            start_y,
            points,
        } = &self.state
        {
            let shape = match tool {
                Tool::Pen => Shape::Freehand {
                    points: points.clone(),
                    color: self.current_color,
                    thick: self.current_thickness,
                },
                Tool::Line => Shape::Line {
                    x1: *start_x,
                    y1: *start_y,
                    x2: x,
                    y2: y,
                    color: self.current_color,
                    thick: self.current_thickness,
                },
                Tool::Rect => {
                    // Normalize rectangle to handle dragging in any direction
                    let (x, w) = if x >= *start_x {
                        (*start_x, x - start_x)
                    } else {
                        (x, start_x - x)
                    };
                    let (y, h) = if y >= *start_y {
                        (*start_y, y - start_y)
                    } else {
                        (y, start_y - y)
                    };
                    Shape::Rect {
                        x,
                        y,
                        w,
                        h,
                        color: self.current_color,
                        thick: self.current_thickness,
                    }
                }
                Tool::Ellipse => {
                    let (cx, cy, rx, ry) = util::ellipse_bounds(*start_x, *start_y, x, y);
                    Shape::Ellipse {
                        cx,
                        cy,
                        rx,
                        ry,
                        color: self.current_color,
                        thick: self.current_thickness,
                    }
                }
                Tool::Arrow => Shape::Arrow {
                    x1: *start_x,
                    y1: *start_y,
                    x2: x,
                    y2: y,
                    color: self.current_color,
                    thick: self.current_thickness,
                    arrow_length: self.arrow_length,
                    arrow_angle: self.arrow_angle,
                },
            };

            self.canvas_set.active_frame_mut().add_shape(shape);
            self.state = DrawingState::Idle;
            self.needs_redraw = true;
        }
    }

    /// Returns the shape currently being drawn for live preview.
    ///
    /// # Arguments
    /// * `current_x` - Current mouse X coordinate
    /// * `current_y` - Current mouse Y coordinate
    ///
    /// # Returns
    /// - `Some(Shape)` if actively drawing (for preview rendering)
    /// - `None` if idle or in text input mode
    ///
    /// # Note
    /// For Pen tool (freehand), this clones the points vector. For better performance
    /// with long strokes, consider using `render_provisional_shape` directly with a
    /// borrow instead of calling this method and rendering separately.
    ///
    /// This allows the backend to render a preview of the shape being drawn
    /// before the mouse button is released.
    pub fn get_provisional_shape(&self, current_x: i32, current_y: i32) -> Option<Shape> {
        if let DrawingState::Drawing {
            tool,
            start_x,
            start_y,
            points,
        } = &self.state
        {
            match tool {
                Tool::Pen => Some(Shape::Freehand {
                    points: points.clone(), // TODO: Consider using Cow or separate borrow API
                    color: self.current_color,
                    thick: self.current_thickness,
                }),
                Tool::Line => Some(Shape::Line {
                    x1: *start_x,
                    y1: *start_y,
                    x2: current_x,
                    y2: current_y,
                    color: self.current_color,
                    thick: self.current_thickness,
                }),
                Tool::Rect => {
                    // Normalize rectangle to handle dragging in any direction
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
                    Some(Shape::Rect {
                        x,
                        y,
                        w,
                        h,
                        color: self.current_color,
                        thick: self.current_thickness,
                    })
                }
                Tool::Ellipse => {
                    let (cx, cy, rx, ry) =
                        util::ellipse_bounds(*start_x, *start_y, current_x, current_y);
                    Some(Shape::Ellipse {
                        cx,
                        cy,
                        rx,
                        ry,
                        color: self.current_color,
                        thick: self.current_thickness,
                    })
                }
                Tool::Arrow => Some(Shape::Arrow {
                    x1: *start_x,
                    y1: *start_y,
                    x2: current_x,
                    y2: current_y,
                    color: self.current_color,
                    thick: self.current_thickness,
                    arrow_length: self.arrow_length,
                    arrow_angle: self.arrow_angle,
                }),
                // No provisional shape for other tools
            }
        } else {
            None
        }
    }

    /// Renders the provisional shape directly to a Cairo context without cloning.
    ///
    /// This is an optimized version for freehand drawing that avoids cloning
    /// the points vector on every render, preventing quadratic performance.
    ///
    /// # Arguments
    /// * `ctx` - Cairo context to render to
    /// * `current_x` - Current mouse X coordinate
    /// * `current_y` - Current mouse Y coordinate
    ///
    /// # Returns
    /// `true` if a provisional shape was rendered, `false` otherwise
    pub fn render_provisional_shape(
        &self,
        ctx: &cairo::Context,
        current_x: i32,
        current_y: i32,
    ) -> bool {
        if let DrawingState::Drawing {
            tool,
            start_x: _,
            start_y: _,
            points,
        } = &self.state
        {
            match tool {
                Tool::Pen => {
                    // Render freehand without cloning - just borrow the points
                    crate::draw::render_freehand_borrowed(
                        ctx,
                        points,
                        self.current_color,
                        self.current_thickness,
                    );
                    true
                }
                _ => {
                    // For other tools, use the normal path (no clone needed)
                    if let Some(shape) = self.get_provisional_shape(current_x, current_y) {
                        crate::draw::render_shape(ctx, &shape);
                        true
                    } else {
                        false
                    }
                }
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Action, BoardConfig};
    use crate::draw::{Color, FontDescriptor};

    fn create_test_input_state() -> InputState {
        use crate::config::KeybindingsConfig;

        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings.build_action_map().unwrap();

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }, // Red
            3.0,  // thickness
            32.0, // font_size
            FontDescriptor {
                family: "Sans".to_string(),
                weight: "bold".to_string(),
                style: "normal".to_string(),
            },
            false,                  // text_background_enabled
            20.0,                   // arrow_length
            30.0,                   // arrow_angle
            BoardConfig::default(), // board_config
            action_map,             // action_map
        )
    }

    #[test]
    fn test_adjust_font_size_increase() {
        let mut state = create_test_input_state();
        assert_eq!(state.current_font_size, 32.0);

        state.adjust_font_size(2.0);
        assert_eq!(state.current_font_size, 34.0);
        assert!(state.needs_redraw);
    }

    #[test]
    fn test_adjust_font_size_decrease() {
        let mut state = create_test_input_state();
        assert_eq!(state.current_font_size, 32.0);

        state.adjust_font_size(-2.0);
        assert_eq!(state.current_font_size, 30.0);
        assert!(state.needs_redraw);
    }

    #[test]
    fn test_adjust_font_size_clamp_min() {
        let mut state = create_test_input_state();
        state.current_font_size = 10.0;

        // Try to go below minimum (8.0)
        state.adjust_font_size(-5.0);
        assert_eq!(state.current_font_size, 8.0);
    }

    #[test]
    fn test_adjust_font_size_clamp_max() {
        let mut state = create_test_input_state();
        state.current_font_size = 70.0;

        // Try to go above maximum (72.0)
        state.adjust_font_size(5.0);
        assert_eq!(state.current_font_size, 72.0);
    }

    #[test]
    fn test_adjust_font_size_at_boundaries() {
        let mut state = create_test_input_state();

        // Test at minimum boundary
        state.current_font_size = 8.0;
        state.adjust_font_size(0.0);
        assert_eq!(state.current_font_size, 8.0);

        // Test at maximum boundary
        state.current_font_size = 72.0;
        state.adjust_font_size(0.0);
        assert_eq!(state.current_font_size, 72.0);
    }

    #[test]
    fn test_adjust_font_size_multiple_adjustments() {
        let mut state = create_test_input_state();
        assert_eq!(state.current_font_size, 32.0);

        // Simulate multiple Ctrl+Shift++ presses
        state.adjust_font_size(2.0);
        state.adjust_font_size(2.0);
        state.adjust_font_size(2.0);
        assert_eq!(state.current_font_size, 38.0);

        // Then decrease
        state.adjust_font_size(-2.0);
        state.adjust_font_size(-2.0);
        assert_eq!(state.current_font_size, 34.0);
    }

    #[test]
    fn test_text_mode_plain_letters_not_triggering_actions() {
        let mut state = create_test_input_state();

        // Enter text mode
        state.state = DrawingState::TextInput {
            x: 100,
            y: 100,
            buffer: String::new(),
        };

        // Type 'r' - should add to buffer, not change color
        let original_color = state.current_color;
        state.on_key_press(Key::Char('r'));

        // Check that 'r' was added to buffer
        if let DrawingState::TextInput { buffer, .. } = &state.state {
            assert_eq!(buffer, "r");
        } else {
            panic!("Should still be in text input mode");
        }

        // Color should NOT have changed
        assert_eq!(state.current_color, original_color);

        // Type more color keys
        state.on_key_press(Key::Char('g'));
        state.on_key_press(Key::Char('b'));
        state.on_key_press(Key::Char('t'));

        if let DrawingState::TextInput { buffer, .. } = &state.state {
            assert_eq!(buffer, "rgbt");
        } else {
            panic!("Should still be in text input mode");
        }

        // Color should still not have changed
        assert_eq!(state.current_color, original_color);
    }

    #[test]
    fn test_text_mode_ctrl_keys_trigger_actions() {
        let mut state = create_test_input_state();

        // Enter text mode
        state.state = DrawingState::TextInput {
            x: 100,
            y: 100,
            buffer: String::from("test"),
        };

        // Press Ctrl (modifier)
        state.on_key_press(Key::Ctrl);

        // Verify Ctrl is held
        assert!(state.modifiers.ctrl);

        // Press 'Z' while Ctrl is held (Ctrl+Z should undo - a non-Exit action)
        state.on_key_press(Key::Char('Z'));

        // Should still be in text mode (undo works but doesn't exit text mode)
        assert!(matches!(state.state, DrawingState::TextInput { .. }));

        // Now test Ctrl+Q for exit
        state.on_key_press(Key::Char('Q'));

        // Exit action from text mode goes to Idle (cancels text mode)
        assert!(matches!(state.state, DrawingState::Idle));

        // Now that we're in Idle, pressing Ctrl+Q again should exit the app
        state.on_key_press(Key::Char('Q'));
        assert!(state.should_exit);
    }

    #[test]
    fn test_text_mode_escape_exits() {
        let mut state = create_test_input_state();

        // Enter text mode
        state.state = DrawingState::TextInput {
            x: 100,
            y: 100,
            buffer: String::from("test"),
        };

        // Press Escape (should cancel text input)
        state.on_key_press(Key::Escape);

        // Should have exited text mode without adding text
        assert!(matches!(state.state, DrawingState::Idle));
        assert!(!state.should_exit); // Just cancel, don't exit app
    }

    #[test]
    fn test_text_mode_f10_shows_help() {
        let mut state = create_test_input_state();

        // Enter text mode
        state.state = DrawingState::TextInput {
            x: 100,
            y: 100,
            buffer: String::new(),
        };

        assert!(!state.show_help);

        // Press F10 (should toggle help even in text mode)
        state.on_key_press(Key::F10);

        // Help should be visible
        assert!(state.show_help);

        // Should still be in text mode
        assert!(matches!(state.state, DrawingState::TextInput { .. }));
    }

    #[test]
    fn test_idle_mode_plain_letters_trigger_color_actions() {
        let mut state = create_test_input_state();

        // Should be in Idle mode
        assert!(matches!(state.state, DrawingState::Idle));

        let original_color = state.current_color;

        // Press 'g' for green
        state.on_key_press(Key::Char('g'));

        // Color should have changed
        assert_ne!(state.current_color, original_color);
        assert_eq!(state.current_color, util::key_to_color('g').unwrap());
    }

    #[test]
    fn capture_action_sets_pending_and_clears_modifiers() {
        let mut state = create_test_input_state();
        state.modifiers.ctrl = true;
        state.modifiers.shift = true;
        state.modifiers.alt = true;

        state.handle_action(Action::CaptureClipboardFull);

        assert_eq!(
            state.pending_capture_action,
            Some(Action::CaptureClipboardFull)
        );
        assert!(!state.modifiers.ctrl);
        assert!(!state.modifiers.shift);
        assert!(!state.modifiers.alt);

        let pending = state.take_pending_capture_action();
        assert_eq!(pending, Some(Action::CaptureClipboardFull));
        assert!(state.pending_capture_action.is_none());
    }

    #[test]
    fn board_mode_toggle_restores_previous_color() {
        let mut state = create_test_input_state();
        let initial_color = state.current_color;
        assert_eq!(state.board_mode(), BoardMode::Transparent);

        state.switch_board_mode(BoardMode::Whiteboard);
        assert_eq!(state.board_mode(), BoardMode::Whiteboard);
        assert_eq!(state.board_previous_color, Some(initial_color));
        let expected_pen = BoardMode::Whiteboard
            .default_pen_color(&state.board_config)
            .expect("whiteboard should have default pen");
        assert_eq!(state.current_color, expected_pen);

        state.switch_board_mode(BoardMode::Whiteboard);
        assert_eq!(state.board_mode(), BoardMode::Transparent);
        assert_eq!(state.current_color, initial_color);
        assert!(state.board_previous_color.is_none());
    }

    #[test]
    fn mouse_drag_creates_shapes_for_each_tool() {
        let mut state = create_test_input_state();

        // Pen
        state.on_mouse_press(MouseButton::Left, 0, 0);
        state.on_mouse_motion(10, 10);
        state.on_mouse_release(MouseButton::Left, 10, 10);
        assert_eq!(state.canvas_set.active_frame().shapes.len(), 1);

        // Line (Shift)
        state.modifiers.shift = true;
        state.on_mouse_press(MouseButton::Left, 0, 0);
        state.on_mouse_release(MouseButton::Left, 5, 5);
        assert_eq!(state.canvas_set.active_frame().shapes.len(), 2);

        // Rectangle (Ctrl)
        state.modifiers.shift = false;
        state.modifiers.ctrl = true;
        state.on_mouse_press(MouseButton::Left, 0, 0);
        state.on_mouse_release(MouseButton::Left, 5, 5);
        assert_eq!(state.canvas_set.active_frame().shapes.len(), 3);

        // Ellipse (Tab)
        state.modifiers.ctrl = false;
        state.modifiers.tab = true;
        state.on_mouse_press(MouseButton::Left, 0, 0);
        state.on_mouse_release(MouseButton::Left, 4, 4);
        assert_eq!(state.canvas_set.active_frame().shapes.len(), 4);

        // Arrow (Ctrl+Shift)
        state.modifiers.tab = false;
        state.modifiers.ctrl = true;
        state.modifiers.shift = true;
        state.on_mouse_press(MouseButton::Left, 0, 0);
        state.on_mouse_release(MouseButton::Left, 6, 6);
        assert_eq!(state.canvas_set.active_frame().shapes.len(), 5);
    }
}
