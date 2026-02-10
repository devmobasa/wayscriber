use super::super::base::{InputState, UiToastKind};
use crate::draw::Color;
use crate::input::BoardBackground;

impl InputState {
    pub(crate) fn set_board_name(&mut self, index: usize, name: String) -> bool {
        let Some(board) = self.boards.board_state_mut(index) else {
            return false;
        };
        let trimmed = name.trim();
        if trimmed.is_empty() {
            self.set_ui_toast(UiToastKind::Warning, "Board name cannot be empty.");
            return false;
        }
        if board.spec.name == trimmed {
            return false;
        }
        board.spec.name = trimmed.to_string();
        self.queue_board_config_save();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn set_board_background_color(&mut self, index: usize, color: Color) -> bool {
        let is_active = self.boards.active_index() == index;
        let Some(board) = self.boards.board_state_mut(index) else {
            return false;
        };
        if board.spec.background.is_transparent() {
            self.set_ui_toast(UiToastKind::Info, "Overlay board has no background color.");
            return false;
        }
        if matches!(board.spec.background, BoardBackground::Solid(existing) if existing == color) {
            return false;
        }

        board.spec.background = BoardBackground::Solid(color);
        if board.spec.auto_adjust_pen {
            board.spec.default_pen_color = Some(super::contrast_color(color));
            if is_active {
                self.current_color = board.spec.effective_pen_color().unwrap_or(color);
                self.sync_highlight_color();
            }
        }
        self.queue_board_config_save();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn toggle_board_pinned(&mut self, index: usize) -> bool {
        let Some(board) = self.boards.board_state_mut(index) else {
            return false;
        };
        board.spec.pinned = !board.spec.pinned;
        self.queue_board_config_save();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn reorder_board(&mut self, from: usize, to: usize) -> bool {
        if !self.boards.move_board(from, to) {
            return false;
        }
        self.queue_board_config_save();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn reorder_page_in_board(
        &mut self,
        board_index: usize,
        from: usize,
        to: usize,
    ) -> bool {
        let Some(board) = self.boards.board_state_mut(board_index) else {
            return false;
        };
        if !board.pages.move_page(from, to) {
            return false;
        }
        if self.boards.active_index() == board_index {
            self.prepare_page_switch();
        } else {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
        }
        true
    }

    pub(crate) fn add_page_in_board(&mut self, board_index: usize) -> bool {
        let Some(board) = self.boards.board_state_mut(board_index) else {
            return false;
        };
        board.pages.new_page();
        let page_num = board.pages.active_index() + 1;
        let page_count = board.pages.page_count();
        let board_name = board.spec.name.clone();
        let board_id = board.spec.id.clone();
        if self.boards.active_index() == board_index {
            self.prepare_page_switch();
        } else {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
        }
        self.set_ui_toast(
            UiToastKind::Info,
            format!("Page added on '{board_name}' ({board_id}) ({page_num}/{page_count})"),
        );
        true
    }

    pub(crate) fn duplicate_page_in_board(
        &mut self,
        board_index: usize,
        page_index: usize,
    ) -> bool {
        let Some(board) = self.boards.board_state_mut(board_index) else {
            return false;
        };
        let Some(new_index) = board.pages.duplicate_page_at(page_index) else {
            return false;
        };
        let page_num = new_index + 1;
        let page_count = board.pages.page_count();
        let board_name = board.spec.name.clone();
        let board_id = board.spec.id.clone();
        if self.boards.active_index() == board_index {
            self.prepare_page_switch();
        } else {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
        }
        self.set_ui_toast(
            UiToastKind::Info,
            format!("Page duplicated on '{board_name}' ({board_id}) ({page_num}/{page_count})"),
        );
        true
    }

    pub(crate) fn rename_page_in_board(
        &mut self,
        board_index: usize,
        page_index: usize,
        name: Option<String>,
    ) -> bool {
        let Some(board) = self.boards.board_state_mut(board_index) else {
            return false;
        };
        if !board.pages.set_page_name(page_index, name) {
            return false;
        }
        if self.boards.active_index() == board_index {
            self.prepare_page_switch();
        } else {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
        }
        self.set_ui_toast(UiToastKind::Info, "Page renamed.");
        true
    }

    pub(crate) fn move_page_between_boards(
        &mut self,
        source_board: usize,
        page_index: usize,
        target_board: usize,
        copy: bool,
    ) -> bool {
        if source_board == target_board {
            return false;
        }
        let board_count = self.boards.board_count();
        if source_board >= board_count || target_board >= board_count {
            return false;
        }
        let (new_index, target_name, target_id, target_count) = {
            let (source, target) = if source_board < target_board {
                let (left, right) = self.boards.board_states_mut().split_at_mut(target_board);
                (&mut left[source_board], &mut right[0])
            } else {
                let (left, right) = self.boards.board_states_mut().split_at_mut(source_board);
                (&mut right[0], &mut left[target_board])
            };

            let page = if copy {
                source
                    .pages
                    .pages()
                    .get(page_index)
                    .map(|frame| frame.clone_without_history())
            } else {
                source.pages.take_page(page_index)
            };
            let Some(page) = page else {
                return false;
            };

            let new_index = target.pages.push_page(page);
            let target_name = target.spec.name.clone();
            let target_id = target.spec.id.clone();
            let target_count = target.pages.page_count();
            (new_index, target_name, target_id, target_count)
        };

        let action = if copy { "copied" } else { "moved" };
        self.set_ui_toast(
            UiToastKind::Info,
            format!(
                "Page {action} to '{target_name}' ({target_id}) ({}/{})",
                new_index + 1,
                target_count
            ),
        );
        self.mark_session_dirty();
        true
    }

    pub fn page_prev(&mut self) -> bool {
        if self.boards.prev_page() {
            self.prepare_page_switch();
            true
        } else {
            false
        }
    }

    pub fn page_next(&mut self) -> bool {
        if self.boards.next_page() {
            self.prepare_page_switch();
            true
        } else {
            false
        }
    }

    pub fn switch_to_page(&mut self, index: usize) -> bool {
        if self.boards.active_pages_mut().switch_to_page(index) {
            self.prepare_page_switch();
            true
        } else {
            false
        }
    }

    pub fn page_new(&mut self) {
        self.boards.new_page();
        self.prepare_page_switch();
        let page_num = self.boards.active_page_index() + 1;
        let page_count = self.boards.page_count();
        self.set_ui_toast(
            UiToastKind::Info,
            format!("Page created ({page_num}/{page_count})"),
        );
    }

    pub fn page_duplicate(&mut self) {
        self.boards.duplicate_page();
        self.prepare_page_switch();
        let page_num = self.boards.active_page_index() + 1;
        let page_count = self.boards.page_count();
        self.set_ui_toast(
            UiToastKind::Info,
            format!("Page duplicated ({page_num}/{page_count})"),
        );
    }
}
