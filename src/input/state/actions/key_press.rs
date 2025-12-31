use crate::config::Action;
use crate::draw::Shape;
use crate::input::events::Key;
use log::warn;

use super::super::{DrawingState, InputState, TextInputMode};

const PROPERTIES_PANEL_COARSE_STEP: i32 = 5;
const MAX_TEXT_LENGTH: usize = 10_000;

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
        if self.show_help && self.handle_help_overlay_key(key) {
            return;
        }

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

        if self.is_properties_panel_open() {
            let adjust_step = if self.modifiers.shift {
                PROPERTIES_PANEL_COARSE_STEP
            } else {
                1
            };
            let handled = match key {
                Key::Escape => {
                    self.close_properties_panel();
                    true
                }
                Key::Up => self.focus_previous_properties_entry(),
                Key::Down => self.focus_next_properties_entry(),
                Key::Home => self.focus_first_properties_entry(),
                Key::End => self.focus_last_properties_entry(),
                Key::Return | Key::Space => self.activate_properties_panel_entry(),
                Key::Left => self.adjust_properties_panel_entry(-adjust_step),
                Key::Right => self.adjust_properties_panel_entry(adjust_step),
                Key::Char('+') | Key::Char('=') => self.adjust_properties_panel_entry(adjust_step),
                Key::Char('-') | Key::Char('_') => self.adjust_properties_panel_entry(-adjust_step),
                _ => false,
            };
            if handled {
                return;
            }
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

        if matches!(key, Key::Escape)
            && matches!(self.state, DrawingState::Idle)
            && self.has_selection()
        {
            let bounds = self.selection_bounding_box(self.selected_shape_ids());
            self.clear_selection();
            self.mark_selection_dirty_region(bounds);
            self.needs_redraw = true;
            return;
        }

        // In text input mode, only check actions if modifiers are pressed or it's a special key
        // This allows plain letters to be typed without triggering color/tool actions
        if matches!(&self.state, DrawingState::TextInput { .. }) {
            // Only check for actions if:
            // 1. Modifiers are held (Ctrl, Alt, Shift for special commands)
            // 2. OR it's a special non-character key (Escape, F10, etc.)
            let should_check_actions = match key {
                // Special keys always check for actions
                Key::Escape
                | Key::F1
                | Key::F2
                | Key::F4
                | Key::F9
                | Key::F10
                | Key::F11
                | Key::F12
                | Key::Return
                | Key::Up
                | Key::Down
                | Key::Left
                | Key::Right
                | Key::Delete
                | Key::Home
                | Key::End
                | Key::PageUp
                | Key::PageDown => true,
                // Character keys only check if modifiers are held
                Key::Char(_) => self.modifiers.ctrl || self.modifiers.alt,
                // Other keys can check as well
                _ => self.modifiers.ctrl || self.modifiers.alt,
            };

            if should_check_actions && let Some(key_str) = key_to_action_label(key) {
                // Check if this key combination triggers an action
                if let Some(action) = self.find_action(&key_str) {
                    // Actions work in text mode
                    // Note: Exit action has special logic in handle_action - it cancels
                    // text mode if in TextInput state, or exits app if in Idle state
                    self.handle_action(action);
                    return;
                }
            }

            // No action triggered, handle as text input
            // Handle Return key for finalizing text input (only plain Return, not Shift+Return)
            if matches!(key, Key::Return) && !self.modifiers.shift {
                let (x, y, text) = if let DrawingState::TextInput { x, y, buffer } = &self.state {
                    (*x, *y, buffer.clone())
                } else {
                    (0, 0, String::new())
                };

                if text.is_empty() {
                    if self.text_edit_target.is_some() {
                        self.cancel_text_input();
                    } else {
                        self.clear_text_preview_dirty();
                        self.last_text_preview_bounds = None;
                        self.text_wrap_width = None;
                        self.state = DrawingState::Idle;
                        self.needs_redraw = true;
                    }
                    return;
                }

                let shape = match self.text_input_mode {
                    TextInputMode::Plain => Shape::Text {
                        x,
                        y,
                        text,
                        color: self.current_color,
                        size: self.current_font_size,
                        font_descriptor: self.font_descriptor.clone(),
                        background_enabled: self.text_background_enabled,
                        wrap_width: self.text_wrap_width,
                    },
                    TextInputMode::StickyNote => Shape::StickyNote {
                        x,
                        y,
                        text,
                        background: self.current_color,
                        size: self.current_font_size,
                        font_descriptor: self.font_descriptor.clone(),
                        wrap_width: self.text_wrap_width,
                    },
                };
                let bounds = shape.bounding_box();

                self.clear_text_preview_dirty();
                self.last_text_preview_bounds = None;

                if self.commit_text_edit(shape.clone()) {
                    self.text_wrap_width = None;
                    self.state = DrawingState::Idle;
                    return;
                }

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
                self.text_wrap_width = None;
                self.state = DrawingState::Idle;
                return;
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
        if matches!(key, Key::Escape)
            && let DrawingState::Drawing { .. } = &self.state
            && let Some(Action::Exit) = self.find_action("Escape")
        {
            self.state = DrawingState::Idle;
            self.needs_redraw = true;
            return;
        }

        // Convert key to string for action lookup
        let Some(key_str) = key_to_action_label(key) else {
            return;
        };

        // Look up action based on keybinding
        if let Some(action) = self.find_action(&key_str) {
            self.handle_action(action);
            return;
        }

        if matches!(key, Key::Return)
            && !self.modifiers.ctrl
            && !self.modifiers.shift
            && !self.modifiers.alt
            && matches!(self.state, DrawingState::Idle)
            && self.edit_selected_text()
        {}
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
}

fn key_to_action_label(key: Key) -> Option<String> {
    match key {
        Key::Char(c) => Some(c.to_string()),
        Key::Escape => Some("Escape".to_string()),
        Key::Return => Some("Return".to_string()),
        Key::Backspace => Some("Backspace".to_string()),
        Key::Space => Some("Space".to_string()),
        Key::F1 => Some("F1".to_string()),
        Key::F2 => Some("F2".to_string()),
        Key::F4 => Some("F4".to_string()),
        Key::F9 => Some("F9".to_string()),
        Key::F10 => Some("F10".to_string()),
        Key::F11 => Some("F11".to_string()),
        Key::F12 => Some("F12".to_string()),
        Key::Menu => Some("Menu".to_string()),
        Key::Up => Some("ArrowUp".to_string()),
        Key::Down => Some("ArrowDown".to_string()),
        Key::Left => Some("ArrowLeft".to_string()),
        Key::Right => Some("ArrowRight".to_string()),
        Key::Delete => Some("Delete".to_string()),
        Key::Home => Some("Home".to_string()),
        Key::End => Some("End".to_string()),
        Key::PageUp => Some("PageUp".to_string()),
        Key::PageDown => Some("PageDown".to_string()),
        _ => None,
    }
}
