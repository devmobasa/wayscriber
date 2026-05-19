use super::super::{
    InputState,
    interaction::{CanvasPoint, PointerMotion, PointerPoints, ScreenPoint, route_pointer_motion},
};

impl InputState {
    /// Processes mouse motion (dragging) events.
    ///
    /// # Arguments
    /// * `x` - Current mouse X coordinate
    /// * `y` - Mouse Y coordinate
    ///
    /// # Behavior
    /// - When drawing with Pen tool: Adds points to the freehand stroke
    /// - When drawing with other tools: Triggers redraw for live preview
    #[allow(dead_code)] // Retained for older callers that only have canvas coordinates.
    pub fn on_mouse_motion(&mut self, x: i32, y: i32) {
        let (screen_x, screen_y) = self.screen_coords_for_canvas(x, y);
        self.on_mouse_motion_with_canvas(screen_x, screen_y, x, y);
    }

    pub fn on_mouse_motion_with_canvas(
        &mut self,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
    ) {
        let points = PointerPoints::new(
            ScreenPoint::new(screen_x, screen_y),
            CanvasPoint::new(canvas_x, canvas_y),
        );
        let _ = route_pointer_motion(self, PointerMotion::new(points));
    }
}

#[cfg(test)]
mod tests {
    use crate::input::BOARD_ID_WHITEBOARD;
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn on_mouse_motion_wrapper_preserves_screen_coords_from_canvas_transform() {
        let mut state = make_test_input_state();
        state.switch_board(BOARD_ID_WHITEBOARD);
        assert!(state.boards.active_frame_mut().set_view_offset(100, 200));

        state.on_mouse_motion(130, 240);

        assert_eq!(state.pointer_position(), (30, 40));
        assert_eq!(state.canvas_pointer_position(), (130, 240));
    }
}
