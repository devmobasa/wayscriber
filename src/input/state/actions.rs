use crate::config::Action;
use crate::draw::Shape;
use crate::input::{board_mode::BoardMode, events::Key};
use crate::util;

use super::{DrawingState, InputState};

impl InputState {
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
                Key::Escape | Key::F10 | Key::F11 | Key::F12 | Key::Return => true,
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
                    Key::F10 => "F10".to_string(),
                    Key::F11 => "F11".to_string(),
                    Key::F12 => "F12".to_string(),
                    _ => String::new(),
                };

                // Check if this key combination triggers an action
                if !key_str.is_empty() {
                    if let Some(action) = self.find_action(&key_str) {
                        // Actions work in text mode
                        // Note: Exit action has special logic in handle_action - it cancels
                        // text mode if in TextInput state, or exits app if in Idle state
                        self.handle_action(action);
                        return;
                    }
                }
            }

            // No action triggered, handle as text input
            // Handle Return key for finalizing text input (only plain Return, not Shift+Return)
            if matches!(key, Key::Return) && !self.modifiers.shift {
                if let DrawingState::TextInput { x, y, buffer } = &self.state {
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
        if matches!(key, Key::Escape) {
            if let DrawingState::Drawing { .. } = &self.state {
                if let Some(Action::Exit) = self.find_action("Escape") {
                    self.state = DrawingState::Idle;
                    self.needs_redraw = true;
                    return;
                }
            }
        }

        // Convert key to string for action lookup
        let key_str = match key {
            Key::Char(c) => c.to_string(),
            Key::Escape => "Escape".to_string(),
            Key::Return => "Return".to_string(),
            Key::Backspace => "Backspace".to_string(),
            Key::Space => "Space".to_string(),
            Key::F10 => "F10".to_string(),
            Key::F11 => "F11".to_string(),
            Key::F12 => "F12".to_string(),
            _ => return,
        };

        // Look up action based on keybinding
        if let Some(action) = self.find_action(&key_str) {
            self.handle_action(action);
        }
    }

    /// Handle an action triggered by a keybinding.
    pub(super) fn handle_action(&mut self, action: Action) {
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
            Action::ToggleStatusBar => {
                self.show_status_bar = !self.show_status_bar;
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
                self.set_pending_capture_action(action);

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
}
