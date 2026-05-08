use super::super::{DrawingState, InputState, TextInputMode, UiToastKind};
use crate::config::Action;
use log::info;

impl InputState {
    pub(super) fn handle_core_action(&mut self, action: Action) -> bool {
        match action {
            Action::Exit => {
                if self.try_cancel_active_interaction() {
                    true
                } else {
                    self.should_exit = true;
                    self.end_pointer_drag();
                    true
                }
            }
            Action::EnterTextMode => {
                if matches!(self.state, DrawingState::Idle) {
                    self.text_input_mode = TextInputMode::Plain;
                    self.text_edit_target = None;
                    self.text_wrap_width = None;
                    self.state = DrawingState::TextInput {
                        x: (self.screen_width / 2) as i32,
                        y: (self.screen_height / 2) as i32,
                        buffer: String::new(),
                    };
                    self.last_text_preview_bounds = None;
                    self.update_text_preview_dirty();
                    self.needs_redraw = true;
                }
                true
            }
            Action::EnterStickyNoteMode => {
                if matches!(self.state, DrawingState::Idle) {
                    self.text_input_mode = TextInputMode::StickyNote;
                    self.text_edit_target = None;
                    self.text_wrap_width = None;
                    self.state = DrawingState::TextInput {
                        x: (self.screen_width / 2) as i32,
                        y: (self.screen_height / 2) as i32,
                        buffer: String::new(),
                    };
                    self.last_text_preview_bounds = None;
                    self.update_text_preview_dirty();
                    self.needs_redraw = true;
                }
                true
            }
            Action::ClearCanvas => {
                let (has_locked, has_unlocked) = {
                    let frame = self.boards.active_frame();
                    let mut has_locked = false;
                    let mut has_unlocked = false;
                    for shape in &frame.shapes {
                        if shape.locked {
                            has_locked = true;
                        } else {
                            has_unlocked = true;
                        }
                        if has_locked && has_unlocked {
                            break;
                        }
                    }
                    (has_locked, has_unlocked)
                };

                if self.clear_all() {
                    if has_locked {
                        self.set_ui_toast(
                            UiToastKind::Warning,
                            "Cleared unlocked shapes (locked shapes remain).",
                        );
                        info!("Cleared unlocked shapes; locked shapes remain");
                    } else {
                        info!("Cleared canvas");
                    }
                } else if has_locked && !has_unlocked {
                    self.set_ui_toast(UiToastKind::Warning, "All shapes are locked.");
                }
                true
            }
            _ => false,
        }
    }
}
