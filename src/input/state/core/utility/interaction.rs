use super::super::base::{DrawingState, InputState};
use crate::util::Rect;

impl InputState {
    /// Updates the cached pointer location.
    pub fn update_pointer_position(&mut self, x: i32, y: i32) {
        self.last_pointer_position = (x, y);
        if self.click_highlight.update_tool_ring(
            self.highlight_tool_active(),
            x,
            y,
            &mut self.dirty_tracker,
        ) {
            self.needs_redraw = true;
        }
    }

    /// Updates the cached pointer location without triggering pointer-driven visuals.
    pub fn update_pointer_position_synthetic(&mut self, x: i32, y: i32) {
        self.last_pointer_position = (x, y);
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
