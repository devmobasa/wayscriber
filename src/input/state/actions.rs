use crate::config::Action;
use crate::draw::Shape;
use crate::input::{board_mode::BoardMode, events::Key};
use crate::util;
use log::{info, warn};

use super::{DrawingState, InputState};

pub(super) const MAX_TEXT_LENGTH: usize = 10_000;

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

        if matches!(key, Key::Escape) && self.properties_panel().is_some() {
            self.close_properties_panel();
            return;
        }

        if self.is_context_menu_open() {
            let handled = match key {
                Key::Escape => {
                    self.close_context_menu();
                    true
                }
                Key::Up => self.focus_previous_context_menu_entry(),
                Key::Down => self.focus_next_context_menu_entry(),
                Key::Home => self.focus_first_context_menu_entry(),
                Key::End => self.focus_last_context_menu_entry(),
                Key::Return | Key::Space => self.activate_context_menu_selection(),
                _ => false,
            };
            if handled {
                return;
            }
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
                    Key::Menu => "Menu".to_string(),
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

                        let shape = Shape::Text {
                            x,
                            y,
                            text,
                            color: self.current_color,
                            size: self.current_font_size,
                            font_descriptor: self.font_descriptor.clone(),
                            background_enabled: self.text_background_enabled,
                        };
                        let bounds = shape.bounding_box();

                        self.clear_text_preview_dirty();
                        self.last_text_preview_bounds = None;

                        let added = self
                            .canvas_set
                            .active_frame_mut()
                            .try_add_shape(shape, self.max_shapes_per_frame);
                        if added {
                            self.dirty_tracker.mark_optional_rect(bounds);
                            self.needs_redraw = true;
                        } else {
                            warn!(
                                "Shape limit ({}) reached; new text not added",
                                self.max_shapes_per_frame
                            );
                        }
                    }
                    self.state = DrawingState::Idle;
                    return;
                }
            }

            // Regular text input - add character to buffer
            if let DrawingState::TextInput { buffer, .. } = &mut self.state {
                match key {
                    Key::Char(c) => {
                        if Self::push_text_char(buffer, c) {
                            self.needs_redraw = true;
                            self.update_text_preview_dirty();
                        } else {
                            warn!(
                                "Text input reached maximum length of {} characters",
                                MAX_TEXT_LENGTH
                            );
                        }
                        return;
                    }
                    Key::Backspace => {
                        buffer.pop();
                        self.needs_redraw = true;
                        self.update_text_preview_dirty();
                        return;
                    }
                    Key::Space => {
                        if Self::push_text_char(buffer, ' ') {
                            self.needs_redraw = true;
                            self.update_text_preview_dirty();
                        } else {
                            warn!(
                                "Text input reached maximum length of {} characters",
                                MAX_TEXT_LENGTH
                            );
                        }
                        return;
                    }
                    Key::Return if self.modifiers.shift => {
                        // Shift+Enter: insert newline
                        if Self::push_text_char(buffer, '\n') {
                            self.needs_redraw = true;
                            self.update_text_preview_dirty();
                        } else {
                            warn!(
                                "Text input reached maximum length of {} characters",
                                MAX_TEXT_LENGTH
                            );
                        }
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
            Key::Menu => "Menu".to_string(),
            _ => return,
        };

        // Look up action based on keybinding
        if let Some(action) = self.find_action(&key_str) {
            self.handle_action(action);
        }
    }

    fn push_text_char(buffer: &mut String, ch: char) -> bool {
        let additional = ch.len_utf8();
        if buffer.len() + additional <= MAX_TEXT_LENGTH {
            buffer.push(ch);
            true
        } else {
            false
        }
    }

    /// Handle an action triggered by a keybinding.
    pub(super) fn handle_action(&mut self, action: Action) {
        if !matches!(action, Action::OpenContextMenu) {
            self.close_properties_panel();
        }
        match action {
            Action::Exit => {
                // Exit drawing mode or cancel current action
                match &self.state {
                    DrawingState::TextInput { .. } => {
                        self.clear_text_preview_dirty();
                        self.last_text_preview_bounds = None;
                        self.state = DrawingState::Idle;
                        self.needs_redraw = true;
                    }
                    DrawingState::Drawing { .. } => {
                        self.clear_provisional_dirty();
                        self.last_provisional_bounds = None;
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
                    self.last_text_preview_bounds = None;
                    self.update_text_preview_dirty();
                    self.needs_redraw = true;
                }
            }
            Action::ClearCanvas => {
                if self.clear_all() {
                    info!("Cleared canvas via keybinding");
                }
            }
            Action::Undo => {
                if let Some(action) = self.canvas_set.active_frame_mut().undo_last() {
                    self.apply_action_side_effects(&action);
                }
            }
            Action::Redo => {
                if let Some(action) = self.canvas_set.active_frame_mut().redo_last() {
                    self.apply_action_side_effects(&action);
                }
            }
            Action::IncreaseThickness => {
                self.current_thickness = (self.current_thickness + 1.0).min(20.0);
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
            Action::DecreaseThickness => {
                self.current_thickness = (self.current_thickness - 1.0).max(1.0);
                self.dirty_tracker.mark_full();
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
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
            Action::ToggleStatusBar => {
                self.show_status_bar = !self.show_status_bar;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
            Action::ToggleClickHighlight => {
                let enabled = self.toggle_click_highlight();
                info!(
                    "Click highlight {}",
                    if enabled { "enabled" } else { "disabled" }
                );
            }
            Action::ToggleHighlightTool => {
                let enabled = self.toggle_highlight_tool();
                info!(
                    "Highlight tool {}",
                    if enabled { "selected" } else { "released" }
                );
            }
            Action::OpenContextMenu => {
                self.toggle_context_menu_via_keyboard();
            }
            Action::OpenConfigurator => {
                self.launch_configurator();
            }
            Action::SetColorRed => {
                self.current_color = util::key_to_color('r').unwrap();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.sync_highlight_color();
            }
            Action::SetColorGreen => {
                self.current_color = util::key_to_color('g').unwrap();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.sync_highlight_color();
            }
            Action::SetColorBlue => {
                self.current_color = util::key_to_color('b').unwrap();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.sync_highlight_color();
            }
            Action::SetColorYellow => {
                self.current_color = util::key_to_color('y').unwrap();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.sync_highlight_color();
            }
            Action::SetColorOrange => {
                self.current_color = util::key_to_color('o').unwrap();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.sync_highlight_color();
            }
            Action::SetColorPink => {
                self.current_color = util::key_to_color('p').unwrap();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.sync_highlight_color();
            }
            Action::SetColorWhite => {
                self.current_color = util::key_to_color('w').unwrap();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.sync_highlight_color();
            }
            Action::SetColorBlack => {
                self.current_color = util::key_to_color('k').unwrap();
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.sync_highlight_color();
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
