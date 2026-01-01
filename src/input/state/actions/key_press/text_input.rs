use log::warn;

use crate::draw::Shape;
use crate::input::events::Key;
use crate::input::state::{DrawingState, InputState, TextInputMode};

use super::bindings::key_to_action_label;

const MAX_TEXT_LENGTH: usize = 10_000;

impl InputState {
    pub(super) fn handle_text_input_key(&mut self, key: Key) {
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

        if should_check_actions
            && let Some(key_str) = key_to_action_label(key)
            && let Some(action) = self.find_action(&key_str)
        {
            // Actions work in text mode.
            // Exit action has special logic in handle_action.
            self.handle_action(action);
            return;
        }

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
                }
                Key::Backspace => {
                    buffer.pop();
                    self.needs_redraw = true;
                    self.update_text_preview_dirty();
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
                }
                _ => {
                    // Ignore other keys in text mode.
                }
            }
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
}
