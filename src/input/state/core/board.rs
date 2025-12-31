use super::base::InputState;
use crate::input::board_mode::BoardMode;

impl InputState {
    /// Returns the current board mode.
    pub fn board_mode(&self) -> BoardMode {
        self.canvas_set.active_mode()
    }

    /// Switches to a different board mode with color auto-adjustment.
    ///
    /// Handles mode transitions with automatic color adjustment for contrast:
    /// - Entering board mode: saves current color, applies mode default
    /// - Exiting board mode: restores previous color
    /// - Switching between boards: applies new mode default
    ///
    /// Also resets drawing state to prevent partial shapes crossing modes.
    pub fn switch_board_mode(&mut self, new_mode: BoardMode) {
        let current_mode = self.canvas_set.active_mode();

        // Toggle behavior: if already in target mode, return to transparent
        let target_mode = if current_mode == new_mode && new_mode != BoardMode::Transparent {
            BoardMode::Transparent
        } else {
            new_mode
        };

        // No-op if we're already in the target mode
        if current_mode == target_mode {
            return;
        }

        // Handle color auto-adjustment based on transition type (if enabled)
        if self.board_config.auto_adjust_pen {
            match (current_mode, target_mode) {
                // Entering board mode from transparent
                (BoardMode::Transparent, BoardMode::Whiteboard | BoardMode::Blackboard) => {
                    // Save current color and apply board default
                    self.board_previous_color = Some(self.current_color);
                    if let Some(default_color) = target_mode.default_pen_color(&self.board_config) {
                        self.current_color = default_color;
                        self.sync_highlight_color();
                    }
                }
                // Exiting board mode to transparent
                (BoardMode::Whiteboard | BoardMode::Blackboard, BoardMode::Transparent) => {
                    // Restore previous color if we saved one
                    if let Some(prev_color) = self.board_previous_color {
                        self.current_color = prev_color;
                        self.board_previous_color = None;
                        self.sync_highlight_color();
                    }
                }
                // Switching between board modes
                (BoardMode::Whiteboard, BoardMode::Blackboard)
                | (BoardMode::Blackboard, BoardMode::Whiteboard) => {
                    // Apply new board's default color
                    if let Some(default_color) = target_mode.default_pen_color(&self.board_config) {
                        self.current_color = default_color;
                        self.sync_highlight_color();
                    }
                }
                // All other transitions (shouldn't happen, but handle gracefully)
                _ => {}
            }
        }

        // Switch the active frame
        self.canvas_set.switch_mode(target_mode);

        // Reset drawing state to prevent partial shapes crossing modes
        self.state = super::base::DrawingState::Idle;

        // Trigger redraw
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;

        log::info!("Switched from {:?} to {:?} mode", current_mode, target_mode);
    }

    fn prepare_page_switch(&mut self) {
        self.cancel_active_interaction();
        self.clear_selection();
        self.close_context_menu();
        self.invalidate_hit_cache();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub fn page_prev(&mut self) -> bool {
        let mode = self.canvas_set.active_mode();
        if self.canvas_set.prev_page(mode) {
            self.prepare_page_switch();
            true
        } else {
            false
        }
    }

    pub fn page_next(&mut self) -> bool {
        let mode = self.canvas_set.active_mode();
        if self.canvas_set.next_page(mode) {
            self.prepare_page_switch();
            true
        } else {
            false
        }
    }

    pub fn page_new(&mut self) {
        let mode = self.canvas_set.active_mode();
        self.canvas_set.new_page(mode);
        self.prepare_page_switch();
    }

    pub fn page_duplicate(&mut self) {
        let mode = self.canvas_set.active_mode();
        self.canvas_set.duplicate_page(mode);
        self.prepare_page_switch();
    }

    pub fn page_delete(&mut self) -> crate::draw::PageDeleteOutcome {
        let mode = self.canvas_set.active_mode();
        let outcome = self.canvas_set.delete_page(mode);
        self.prepare_page_switch();
        outcome
    }
}
