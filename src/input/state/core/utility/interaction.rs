use super::super::base::{DrawingState, InputState};
use crate::util::Rect;

impl InputState {
    fn board_view_offset(&self) -> (f64, f64) {
        if self.board_is_transparent() || !self.boards.pan_enabled() {
            (0.0, 0.0)
        } else {
            let (x, y) = self.boards.active_frame().view_offset();
            (x as f64, y as f64)
        }
    }

    fn current_canvas_scale(&self) -> f64 {
        if self.zoom_active {
            self.zoom_scale.max(f64::MIN_POSITIVE)
        } else {
            1.0
        }
    }

    fn current_canvas_origin(&self) -> (f64, f64) {
        let (board_x, board_y) = self.board_view_offset();
        if self.zoom_active {
            (
                board_x + self.zoom_view_offset.0,
                board_y + self.zoom_view_offset.1,
            )
        } else {
            (board_x, board_y)
        }
    }

    fn canvas_coords_for_screen(&self, screen_x: i32, screen_y: i32) -> (i32, i32) {
        let scale = self.current_canvas_scale();
        let (origin_x, origin_y) = self.current_canvas_origin();
        (
            (origin_x + screen_x as f64 / scale).round() as i32,
            (origin_y + screen_y as f64 / scale).round() as i32,
        )
    }

    pub(crate) fn sync_canvas_pointer_to_current_transform(&mut self) {
        let (screen_x, screen_y) = self.last_pointer_position;
        self.last_canvas_pointer_position = self.canvas_coords_for_screen(screen_x, screen_y);
    }

    #[allow(dead_code)] // Kept for legacy input wrappers that translate canvas coords back to UI space.
    pub(crate) fn screen_coords_for_canvas(&self, canvas_x: i32, canvas_y: i32) -> (i32, i32) {
        let scale = self.current_canvas_scale();
        let (origin_x, origin_y) = self.current_canvas_origin();
        (
            ((canvas_x as f64 - origin_x) * scale).round() as i32,
            ((canvas_y as f64 - origin_y) * scale).round() as i32,
        )
    }

    pub(crate) fn screen_rect_for_canvas(&self, rect: Rect) -> Option<Rect> {
        let scale = self.current_canvas_scale();
        let (origin_x, origin_y) = self.current_canvas_origin();
        let min_x = ((rect.x as f64 - origin_x) * scale).floor() as i32;
        let min_y = ((rect.y as f64 - origin_y) * scale).floor() as i32;
        let max_x = (((rect.x + rect.width) as f64 - origin_x) * scale).ceil() as i32;
        let max_y = (((rect.y + rect.height) as f64 - origin_y) * scale).ceil() as i32;
        Rect::from_min_max(min_x, min_y, max_x, max_y)
    }

    /// Returns the last known pointer position.
    pub(crate) fn pointer_position(&self) -> (i32, i32) {
        self.last_pointer_position
    }

    /// Returns the last known pointer position in canvas/world coordinates.
    #[allow(dead_code)]
    pub(crate) fn canvas_pointer_position(&self) -> (i32, i32) {
        self.last_canvas_pointer_position
    }

    /// Updates the cached pointer location.
    pub fn update_pointer_position(&mut self, x: i32, y: i32) {
        let (canvas_x, canvas_y) = self.canvas_coords_for_screen(x, y);
        self.update_pointer_positions(x, y, canvas_x, canvas_y);
    }

    /// Updates cached screen and canvas pointer locations together.
    pub fn update_pointer_positions(
        &mut self,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
    ) {
        self.last_pointer_position = (screen_x, screen_y);
        self.last_canvas_pointer_position = (canvas_x, canvas_y);
        if self.click_highlight.update_tool_ring(
            self.highlight_tool_active(),
            canvas_x,
            canvas_y,
            &mut self.dirty_tracker,
        ) {
            self.needs_redraw = true;
        }
    }

    /// Updates the cached pointer location without triggering pointer-driven visuals.
    pub fn update_pointer_position_synthetic(&mut self, x: i32, y: i32) {
        self.last_pointer_position = (x, y);
        self.last_canvas_pointer_position = self.canvas_coords_for_screen(x, y);
    }

    /// Updates the undo stack limit for subsequent actions.
    pub fn set_undo_stack_limit(&mut self, limit: usize) {
        self.undo_stack_limit = limit.max(1);
    }

    /// Updates screen dimensions after backend configuration.
    ///
    /// This should be called by the backend when it receives the actual
    /// screen dimensions from the display server.
    pub fn update_screen_dimensions(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Cancels the current text input session and restores any edited shape.
    pub(crate) fn cancel_text_input(&mut self) {
        self.cancel_text_edit();
        self.clear_text_preview_dirty();
        self.last_text_preview_bounds = None;
        self.text_wrap_width = None;
        self.state = DrawingState::Idle;
        self.needs_redraw = true;
    }

    /// Cancels any in-progress interaction without exiting the application.
    pub(crate) fn cancel_active_interaction(&mut self) {
        match &self.state {
            DrawingState::TextInput { .. } => {
                self.cancel_text_input();
            }
            DrawingState::PendingTextClick { .. } => {
                self.state = DrawingState::Idle;
            }
            DrawingState::Drawing { .. } => {
                self.clear_provisional_dirty();
                self.last_provisional_bounds = None;
                self.state = DrawingState::Idle;
                self.needs_redraw = true;
            }
            DrawingState::MovingSelection { snapshots, .. } => {
                self.restore_selection_from_snapshots(snapshots.clone());
                self.state = DrawingState::Idle;
            }
            DrawingState::Selecting { .. } => {
                self.clear_provisional_dirty();
                self.last_provisional_bounds = None;
                self.state = DrawingState::Idle;
                self.needs_redraw = true;
            }
            DrawingState::ResizingText {
                shape_id, snapshot, ..
            } => {
                self.restore_selection_from_snapshots(vec![(*shape_id, snapshot.clone())]);
                self.state = DrawingState::Idle;
            }
            DrawingState::ResizingSelection { snapshots, .. } => {
                let snapshots = snapshots.clone();
                self.restore_resize_from_snapshots(snapshots.as_ref());
                self.state = DrawingState::Idle;
            }
            DrawingState::Idle => {}
        }
    }

    /// Drains pending dirty rectangles for the current surface size.
    #[allow(dead_code)]
    pub fn take_dirty_regions(&mut self) -> Vec<Rect> {
        let width = self.screen_width.min(i32::MAX as u32) as i32;
        let height = self.screen_height.min(i32::MAX as u32) as i32;
        self.dirty_tracker.take_regions(width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::BOARD_ID_WHITEBOARD;
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn update_pointer_position_synthetic_updates_pointer_without_redraw() {
        let mut state = make_test_input_state();
        state.needs_redraw = false;

        state.update_pointer_position_synthetic(12, 34);

        assert_eq!(state.pointer_position(), (12, 34));
        assert_eq!(state.canvas_pointer_position(), (12, 34));
        assert!(!state.needs_redraw);
    }

    #[test]
    fn update_pointer_position_synthetic_preserves_canvas_transform() {
        let mut state = make_test_input_state();
        state.set_zoom_status(true, false, 2.0, (100.0, 200.0));

        state.update_pointer_position_synthetic(30, 40);

        assert_eq!(state.pointer_position(), (30, 40));
        assert_eq!(state.canvas_pointer_position(), (115, 220));
    }

    #[test]
    fn update_pointer_position_uses_canvas_transform_for_screen_space_updates() {
        let mut state = make_test_input_state();
        state.switch_board(BOARD_ID_WHITEBOARD);
        assert!(state.boards.active_frame_mut().set_view_offset(100, 50));

        state.update_pointer_position(30, 40);

        assert_eq!(state.pointer_position(), (30, 40));
        assert_eq!(state.canvas_pointer_position(), (130, 90));
    }

    #[test]
    fn screen_rect_for_canvas_tracks_board_offset_after_pointer_cache_changes() {
        let mut state = make_test_input_state();
        state.switch_board(BOARD_ID_WHITEBOARD);
        assert!(state.boards.active_frame_mut().set_view_offset(100, 50));
        state.update_pointer_position(400, 300);
        let rect = Rect::new(138, 88, 24, 24).expect("valid rect");

        assert_eq!(
            state.screen_rect_for_canvas(rect),
            Rect::new(38, 38, 24, 24)
        );

        assert!(state.reset_active_canvas_position());
        assert_eq!(
            state.screen_rect_for_canvas(rect),
            Rect::new(138, 88, 24, 24)
        );
    }

    #[test]
    fn set_undo_stack_limit_clamps_to_at_least_one() {
        let mut state = make_test_input_state();
        state.set_undo_stack_limit(0);
        assert_eq!(state.undo_stack_limit, 1);

        state.set_undo_stack_limit(25);
        assert_eq!(state.undo_stack_limit, 25);
    }

    #[test]
    fn cancel_text_input_clears_wrap_width_and_returns_to_idle() {
        let mut state = make_test_input_state();
        state.text_wrap_width = Some(240);
        state.state = DrawingState::TextInput {
            x: 10,
            y: 20,
            buffer: "hello".to_string(),
        };
        state.needs_redraw = false;

        state.cancel_text_input();

        assert!(matches!(state.state, DrawingState::Idle));
        assert!(state.text_wrap_width.is_none());
        assert!(state.needs_redraw);
    }

    #[test]
    fn take_dirty_regions_returns_full_surface_and_drains_tracker() {
        let mut state = make_test_input_state();
        state.update_screen_dimensions(100, 50);
        state.dirty_tracker.mark_full();

        assert_eq!(
            state.take_dirty_regions(),
            vec![Rect::new(0, 0, 100, 50).unwrap()]
        );
        assert!(state.take_dirty_regions().is_empty());
    }
}
