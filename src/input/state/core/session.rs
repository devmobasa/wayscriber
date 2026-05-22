use super::base::{DrawingState, InputState, UiToastKind};
use crate::input::boards::BoardState;
use crate::session::{self, BoardPagesSnapshot, BoardSnapshot, SessionOptions, SessionSnapshot};

impl InputState {
    /// Updates the runtime session options used by size preflights for clone-heavy actions.
    #[allow(dead_code)]
    pub(crate) fn set_session_preflight_options(&mut self, options: Option<SessionOptions>) {
        self.session_preflight_options = options;
    }

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

    pub(crate) fn session_allows_page_duplicate(
        &mut self,
        board_index: usize,
        page_index: usize,
    ) -> bool {
        let Some(options) = self.session_preflight_options.as_ref() else {
            return true;
        };
        if !session_persistence_enabled(options) {
            return true;
        }
        let Some(mut snapshot) = session::snapshot_from_input(self, options) else {
            return true;
        };
        if !duplicate_page_in_snapshot(self, &mut snapshot, board_index, page_index) {
            return true;
        }
        self.session_allows_duplicate_snapshot(&snapshot, "Page")
    }

    pub(crate) fn session_allows_page_copy_between_boards(
        &mut self,
        source_board: usize,
        page_index: usize,
        target_board: usize,
    ) -> bool {
        let Some(options) = self.session_preflight_options.as_ref() else {
            return true;
        };
        if !session_persistence_enabled(options) {
            return true;
        }
        let mut snapshot =
            session::snapshot_from_input(self, options).unwrap_or_else(|| SessionSnapshot {
                active_board_id: self.board_id().to_string(),
                boards: Vec::new(),
                tool_state: None,
            });
        if !copy_page_between_boards_in_snapshot(
            self,
            options,
            &mut snapshot,
            source_board,
            page_index,
            target_board,
        ) {
            return true;
        }
        self.session_allows_copy_snapshot(&snapshot, "Page")
    }

    pub(crate) fn session_allows_board_duplicate(&mut self) -> bool {
        let Some(options) = self.session_preflight_options.as_ref() else {
            return true;
        };
        if !session_persistence_enabled(options) {
            return true;
        }
        let Some(mut snapshot) = session::snapshot_from_input(self, options) else {
            return true;
        };
        if !duplicate_active_board_in_snapshot(self, &mut snapshot) {
            return true;
        }
        self.session_allows_duplicate_snapshot(&snapshot, "Board")
    }

    fn session_allows_duplicate_snapshot(
        &mut self,
        snapshot: &SessionSnapshot,
        action_label: &str,
    ) -> bool {
        self.session_allows_clone_heavy_snapshot(snapshot, action_label, "duplicate")
    }

    fn session_allows_copy_snapshot(
        &mut self,
        snapshot: &SessionSnapshot,
        action_label: &str,
    ) -> bool {
        self.session_allows_clone_heavy_snapshot(snapshot, action_label, "copy")
    }

    fn session_allows_clone_heavy_snapshot(
        &mut self,
        snapshot: &SessionSnapshot,
        action_label: &str,
        action_name: &str,
    ) -> bool {
        let Some(options) = self.session_preflight_options.as_ref() else {
            return true;
        };
        let estimate = match session::estimate_snapshot_save(snapshot, options) {
            Ok(estimate) => estimate,
            Err(err) => {
                log::warn!(
                    "Blocking {} {} because session size preflight failed: {}",
                    action_label.to_lowercase(),
                    action_name,
                    err
                );
                self.set_ui_toast(
                    UiToastKind::Warning,
                    format!("{action_label} {action_name} blocked; session size check failed."),
                );
                self.trigger_blocked_feedback();
                return false;
            }
        };

        if estimate.visible_without_history.limit_exceeded.is_some() {
            let limit = format_session_limit(options.max_file_size_bytes);
            log::warn!(
                "Blocking {} {} because visible session payload would exceed configured cap: written={} raw={} max={}",
                action_label.to_lowercase(),
                action_name,
                estimate.visible_without_history.written_size,
                estimate.visible_without_history.raw_size,
                options.max_file_size_bytes
            );
            self.set_ui_toast(
                UiToastKind::Warning,
                format!(
                    "{action_label} {action_name} blocked; session would exceed {limit}. Remove images or raise session.max_file_size_mb."
                ),
            );
            self.trigger_blocked_feedback();
            return false;
        }

        if estimate.full.limit_exceeded.is_some() || estimate.full.is_near_limit() {
            log::warn!(
                "{} {} allowed but session is near/over full-history cap: full_written={} visible_written={} max={}",
                action_label,
                action_name,
                estimate.full.written_size,
                estimate.visible_without_history.written_size,
                options.max_file_size_bytes
            );
        }
        true
    }
}

fn session_persistence_enabled(options: &SessionOptions) -> bool {
    options.any_enabled() || options.restore_tool_state || options.persist_history
}

fn duplicate_page_in_snapshot(
    input: &InputState,
    snapshot: &mut SessionSnapshot,
    board_index: usize,
    page_index: usize,
) -> bool {
    let Some(source_board) = input.boards.board_states().get(board_index) else {
        return false;
    };
    let Some(board) = snapshot
        .boards
        .iter_mut()
        .find(|board| board.id == source_board.spec.id)
    else {
        return false;
    };
    if page_index >= board.pages.pages.len() {
        return false;
    }
    let cloned = board.pages.pages[page_index].clone_without_history();
    let insert_at = (page_index + 1).min(board.pages.pages.len());
    board.pages.pages.insert(insert_at, cloned);
    board.pages.active = insert_at;
    true
}

fn copy_page_between_boards_in_snapshot(
    input: &InputState,
    options: &SessionOptions,
    snapshot: &mut SessionSnapshot,
    source_board_index: usize,
    page_index: usize,
    target_board_index: usize,
) -> bool {
    if source_board_index == target_board_index {
        return false;
    }
    let Some(source_board) = input.boards.board_states().get(source_board_index) else {
        return false;
    };
    let Some(target_board) = input.boards.board_states().get(target_board_index) else {
        return false;
    };
    let Some(cloned_page) = source_board
        .pages
        .pages()
        .get(page_index)
        .map(|page| page.clone_without_history())
    else {
        return false;
    };
    if !board_should_persist_for_session(target_board, options) {
        return false;
    }

    if let Some(target_snapshot) = snapshot
        .boards
        .iter_mut()
        .find(|board| board.id == target_board.spec.id)
    {
        let new_index = target_snapshot.pages.pages.len();
        target_snapshot.pages.pages.push(cloned_page);
        target_snapshot.pages.active = new_index;
        return true;
    }

    let mut target_pages = target_board.pages.pages().to_vec();
    target_pages.push(cloned_page);
    snapshot.boards.push(BoardSnapshot {
        id: target_board.spec.id.clone(),
        pages: BoardPagesSnapshot {
            active: target_pages.len().saturating_sub(1),
            pages: target_pages,
        },
    });
    true
}

fn board_should_persist_for_session(board: &BoardState, options: &SessionOptions) -> bool {
    if board.spec.background.is_transparent() {
        options.persist_transparent
    } else {
        (options.persist_whiteboard || options.persist_blackboard) && board.spec.persist
    }
}

fn duplicate_active_board_in_snapshot(input: &InputState, snapshot: &mut SessionSnapshot) -> bool {
    let source_board = input.boards.active_board();
    let Some(source_index) = snapshot
        .boards
        .iter()
        .position(|board| board.id == source_board.spec.id)
    else {
        return false;
    };

    let mut cloned = BoardSnapshot {
        id: duplicate_board_id_for_snapshot(input, &source_board.spec.id),
        pages: snapshot.boards[source_index].pages.clone(),
    };
    cloned.pages.active = source_board.pages.active_index();
    let insert_at = (source_index + 1).min(snapshot.boards.len());
    snapshot.active_board_id = cloned.id.clone();
    snapshot.boards.insert(insert_at, cloned);
    true
}

fn duplicate_board_id_for_snapshot(input: &InputState, source_id: &str) -> String {
    let base = format!("{source_id}-copy");
    if !input
        .boards
        .board_states()
        .iter()
        .any(|board| board.spec.id == base)
    {
        return base;
    }

    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !input
            .boards
            .board_states()
            .iter()
            .any(|board| board.spec.id == candidate)
        {
            return candidate;
        }
        suffix += 1;
    }
}

fn format_session_limit(bytes: u64) -> String {
    let mib = bytes as f64 / 1024.0 / 1024.0;
    if mib >= 10.0 {
        format!("{mib:.0} MiB")
    } else {
        format!("{mib:.1} MiB")
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
