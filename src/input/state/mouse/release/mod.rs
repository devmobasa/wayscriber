use crate::input::events::MouseButton;

use super::super::{DrawingState, InputState};

mod drawing;
mod panels;
mod selection;
mod text;

impl InputState {
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
        self.update_pointer_position(x, y);
        if button == MouseButton::Left {
            if panels::handle_board_picker_release(self, x, y) {
                return;
            }
            if panels::handle_properties_panel_release(self, x, y) {
                return;
            }
            if panels::handle_context_menu_release(self, x, y) {
                return;
            }
        }

        if button != MouseButton::Left {
            return;
        }

        let state = std::mem::replace(&mut self.state, DrawingState::Idle);
        match state {
            DrawingState::MovingSelection {
                snapshots, moved, ..
            } => {
                selection::finish_moving_selection(self, snapshots, moved);
            }
            DrawingState::Selecting {
                start_x,
                start_y,
                additive,
            } => {
                selection::finish_selection_drag(self, start_x, start_y, x, y, additive);
            }
            DrawingState::ResizingText {
                shape_id, snapshot, ..
            } => {
                selection::finish_text_resize(self, shape_id, snapshot);
            }
            DrawingState::Drawing {
                tool,
                start_x,
                start_y,
                points,
                point_thicknesses,
            } => {
                drawing::finish_drawing(
                    self,
                    tool,
                    start_x,
                    start_y,
                    points,
                    point_thicknesses,
                    x,
                    y,
                );
            }
            DrawingState::PendingTextClick { x, y, shape_id, .. } => {
                text::handle_pending_text_click(self, x, y, shape_id);
            }
            other_state => {
                self.state = other_state;
            }
        }
    }
}
