use super::base::{DrawingState, InputState};

impl InputState {
    /// Marks session data as dirty for autosave tracking.
    pub(crate) fn mark_session_dirty(&mut self) {
        self.session_dirty = true;
    }

    /// Returns true if session data was marked dirty since the last check.
    #[allow(dead_code)]
    pub(crate) fn take_session_dirty(&mut self) -> bool {
        if self.session_dirty {
            self.session_dirty = false;
            true
        } else {
            false
        }
    }

    /// Clears session dirtiness after loading persisted state into memory.
    #[allow(dead_code)]
    pub(crate) fn clear_session_dirty(&mut self) {
        self.session_dirty = false;
    }

    /// Returns whether session data is dirty without clearing the dirty flag.
    #[allow(dead_code)]
    pub(crate) fn is_session_dirty(&self) -> bool {
        self.session_dirty
    }

    /// Returns true while pointer-driven work is in progress and autosave should wait.
    #[allow(dead_code)]
    pub(crate) fn has_active_pointer_interaction(&self) -> bool {
        self.active_drag_button.is_some()
            || matches!(
                self.state,
                DrawingState::Drawing { .. }
                    | DrawingState::BuildingPolygon { .. }
                    | DrawingState::PendingTextClick { .. }
                    | DrawingState::MovingSelection { .. }
                    | DrawingState::Selecting { .. }
                    | DrawingState::ResizingText { .. }
                    | DrawingState::ResizingSelection { .. }
            )
            || self.board_picker_is_dragging()
            || self.board_picker_is_page_dragging()
            || self.color_picker_popup_is_dragging()
    }
}

#[cfg(test)]
mod tests {
    use crate::draw::frame::ShapeSnapshot;
    use crate::draw::{BLACK, Shape};
    use crate::input::state::core::board_picker::{BoardPickerDrag, BoardPickerPageDrag};
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::{DrawingState, MouseButton, SelectionHandle, Tool};
    use crate::util::Rect;
    use std::sync::Arc;

    #[test]
    fn active_pointer_interaction_tracks_drag_button() {
        let mut state = make_test_input_state();
        assert!(!state.has_active_pointer_interaction());

        state.begin_pointer_drag(MouseButton::Left, None);
        assert!(state.has_active_pointer_interaction());

        state.end_pointer_drag();
        assert!(!state.has_active_pointer_interaction());
    }

    #[test]
    fn active_pointer_interaction_covers_drawing_states() {
        let states = vec![
            DrawingState::Drawing {
                tool: Tool::Pen,
                start_x: 10,
                start_y: 20,
                points: vec![(10, 20)],
                point_thicknesses: vec![2.0],
            },
            DrawingState::PendingTextClick {
                x: 10,
                y: 20,
                tool: Tool::Pen,
                shape_id: 1,
            },
            DrawingState::BuildingPolygon {
                points: vec![(10, 20)],
                preview: None,
                fill: false,
                color: BLACK,
                thick: 2.0,
            },
            DrawingState::MovingSelection {
                last_x: 10,
                last_y: 20,
                snapshots: Vec::new(),
                moved: false,
            },
            DrawingState::Selecting {
                start_x: 10,
                start_y: 20,
                additive: false,
            },
            DrawingState::ResizingText {
                shape_id: 1,
                snapshot: test_shape_snapshot(),
                base_x: 10,
                size: 12.0,
            },
            DrawingState::ResizingSelection {
                handle: SelectionHandle::BottomRight,
                original_bounds: Rect::new(0, 0, 20, 20).expect("valid rect"),
                start_x: 10,
                start_y: 20,
                snapshots: Arc::new(Vec::new()),
            },
        ];

        for drawing_state in states {
            let mut state = make_test_input_state();
            state.state = drawing_state;
            assert!(state.has_active_pointer_interaction());
        }
    }

    #[test]
    fn active_pointer_interaction_covers_picker_drags() {
        let mut state = make_test_input_state();
        state.board_picker_drag = Some(BoardPickerDrag {
            source_row: 0,
            source_board: 0,
            current_row: 0,
        });
        assert!(state.has_active_pointer_interaction());

        let mut state = make_test_input_state();
        state.board_picker_page_drag = Some(BoardPickerPageDrag {
            source_index: 0,
            current_index: 0,
            board_index: 0,
            target_board: Some(0),
        });
        assert!(state.has_active_pointer_interaction());

        let mut state = make_test_input_state();
        state.open_color_picker_popup();
        state.color_picker_popup_set_dragging(true);
        assert!(state.has_active_pointer_interaction());
    }

    fn test_shape_snapshot() -> ShapeSnapshot {
        ShapeSnapshot {
            shape: Shape::Freehand {
                points: vec![(0, 0), (1, 1)],
                color: BLACK,
                thick: 1.0,
            },
            locked: false,
        }
    }
}
