use crate::input::events::MouseButton;

use super::super::{
    DrawingState, InputState,
    interaction::{CanvasPoint, PointerPoints, PointerRelease, ScreenPoint, route_pointer_release},
};

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
    #[allow(dead_code)] // Retained for older callers that only have canvas coordinates.
    pub fn on_mouse_release(&mut self, button: MouseButton, x: i32, y: i32) {
        let (screen_x, screen_y) = self.screen_coords_for_canvas(x, y);
        self.on_mouse_release_with_canvas(button, screen_x, screen_y, x, y);
    }

    pub fn on_mouse_release_with_canvas(
        &mut self,
        button: MouseButton,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
    ) {
        let points = PointerPoints::new(
            ScreenPoint::new(screen_x, screen_y),
            CanvasPoint::new(canvas_x, canvas_y),
        );
        let _ = route_pointer_release(self, PointerRelease::new(button, points));
    }

    pub(in crate::input::state) fn handle_color_picker_popup_release_at(
        &mut self,
        x: i32,
        y: i32,
    ) -> bool {
        panels::handle_color_picker_popup_release(self, x, y)
    }

    pub(in crate::input::state) fn handle_context_menu_release_at(
        &mut self,
        x: i32,
        y: i32,
    ) -> bool {
        panels::handle_context_menu_release(self, x, y)
    }

    pub(in crate::input::state) fn handle_board_picker_release_at(
        &mut self,
        x: i32,
        y: i32,
    ) -> bool {
        panels::handle_board_picker_release(self, x, y)
    }

    pub(in crate::input::state) fn handle_properties_panel_release_at(
        &mut self,
        x: i32,
        y: i32,
    ) -> bool {
        panels::handle_properties_panel_release(self, x, y)
    }

    pub(in crate::input::state) fn finish_pointer_interaction_at(
        &mut self,
        canvas_x: i32,
        canvas_y: i32,
    ) {
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
                selection::finish_selection_drag(
                    self, start_x, start_y, canvas_x, canvas_y, additive,
                );
            }
            DrawingState::ResizingText {
                shape_id, snapshot, ..
            } => {
                selection::finish_text_resize(self, shape_id, snapshot);
            }
            DrawingState::ResizingSelection { snapshots, .. } => {
                selection::finish_selection_resize(self, snapshots.as_ref());
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
                    drawing::DrawingRelease {
                        start: (start_x, start_y),
                        end: (canvas_x, canvas_y),
                        points,
                        point_thicknesses,
                    },
                );
            }
            DrawingState::PendingTextClick { x, y, shape_id, .. } => {
                text::handle_pending_text_click(self, x, y, shape_id);
            }
            other_state => {
                self.state = other_state;
            }
        }
        if matches!(self.state, DrawingState::Idle) {
            self.end_pointer_drag();
            self.sync_current_settings_from_active_tool();
        }
    }
}
