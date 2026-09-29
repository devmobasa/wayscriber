use super::super::{DrawingState, InputState, TextInputMode};
use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority};
use log::info;

impl InputState {
    pub(in crate::input::state) fn handle_core_action_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        action: Action,
    ) -> bool {
        match action {
            Action::Exit => {
                if self.try_cancel_active_interaction_with(measurer) {
                    true
                } else {
                    self.should_exit = true;
                    self.end_pointer_drag();
                    true
                }
            }
            Action::EnterTextMode => {
                self.start_text_draft_at_view_centre_with(measurer, TextInputMode::Plain);
                true
            }
            Action::EnterStickyNoteMode => {
                self.start_text_draft_at_view_centre_with(measurer, TextInputMode::StickyNote);
                true
            }
            Action::ClearCanvas => {
                // Laser ink is not canvas content, but a presenter clearing
                // the screen expects it gone too.
                self.clear_laser_ink();
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
                        self.push_toast(
                            ToastPriority::Info,
                            "core",
                            Toast::warning("Cleared unlocked shapes (locked shapes remain)."),
                        );
                        info!("Cleared unlocked shapes; locked shapes remain");
                    } else {
                        info!("Cleared canvas");
                    }
                } else if has_locked && !has_unlocked {
                    self.push_toast(
                        ToastPriority::Info,
                        "core",
                        Toast::warning("All shapes are locked."),
                    );
                }
                true
            }
            _ => false,
        }
    }

    /// Starts a new draft where the screen centre falls on the canvas. The
    /// keyboard entry points have no pointer position to place it at, and on
    /// a panned or zoomed board the screen centre is not the canvas point
    /// with the same numbers.
    fn start_text_draft_at_view_centre_with(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        mode: TextInputMode,
    ) {
        if !matches!(self.state, DrawingState::Idle) {
            return;
        }

        self.text_editing.prepare_new(mode);
        self.style.text_wrap_width = None;
        self.begin_text_input_session();
        let (screen_width, screen_height) = self.view.screen_size();
        let (x, y) =
            self.canvas_coords_for_screen((screen_width / 2) as i32, (screen_height / 2) as i32);
        self.state = DrawingState::text_input(x, y, String::new());

        self.update_text_preview_dirty_with(measurer);
        self.needs_redraw = true;
    }
}
