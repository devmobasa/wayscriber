use super::super::base::InputState;
use crate::draw::Color;
use crate::input::boards::BoardConfigChange;
use crate::input::state::{Toast, ToastPriority};
use crate::input::{BoardBackground, runtime_contrast_pen_color};

impl InputState {
    pub(crate) fn reset_active_canvas_position(&mut self) -> bool {
        if self.board_is_transparent() || !self.boards.pan_enabled() {
            return false;
        }
        if !self.boards.active_frame_mut().set_view_offset(0, 0) {
            return false;
        }
        self.sync_canvas_pointer_to_current_transform();
        self.mark_board_surface_changed();
        self.push_toast(
            ToastPriority::Info,
            "page.nav",
            Toast::info("Canvas position reset."),
        );
        true
    }

    pub(crate) fn set_board_name(&mut self, index: usize, name: String) -> bool {
        let Some(board) = self.boards.board_state_mut(index) else {
            return false;
        };
        let trimmed = name.trim();
        if trimmed.is_empty() {
            self.push_toast(
                ToastPriority::Info,
                "page.nav",
                Toast::warning("Board name cannot be empty."),
            );
            return false;
        }
        if board.spec.name == trimmed {
            return false;
        }
        board.spec.name = trimmed.to_string();
        let board_id = board.spec.id.clone();
        self.queue_board_config_save(BoardConfigChange::Name(board_id));
        self.mark_board_surface_dirty();
        true
    }

    pub(crate) fn set_board_background_color(&mut self, index: usize, color: Color) -> bool {
        let is_active = self.boards.active_index() == index;
        let Some(board) = self.boards.board_state_mut(index) else {
            return false;
        };
        if board.spec.background.is_transparent() {
            self.push_toast(
                ToastPriority::Info,
                "page.nav",
                Toast::info("Overlay board has no background color."),
            );
            return false;
        }
        if matches!(board.spec.background, BoardBackground::Solid(existing) if existing == color) {
            return false;
        }

        board.spec.background = BoardBackground::Solid(color);
        let active_pen_color = if board.spec.auto_adjust_pen {
            board.spec.default_pen_color = Some(runtime_contrast_pen_color(color));
            is_active.then(|| board.spec.effective_pen_color().unwrap_or(color))
        } else {
            None
        };
        if let Some(color) = active_pen_color {
            self.set_pen_color_from_board(color);
        }
        let board_id = self.boards.board_states()[index].spec.id.clone();
        self.queue_board_config_save(BoardConfigChange::Appearance(board_id));
        self.mark_board_surface_dirty();
        true
    }

    pub(crate) fn toggle_board_pinned(&mut self, index: usize) -> bool {
        let Some(board) = self.boards.board_state_mut(index) else {
            return false;
        };
        board.spec.pinned = !board.spec.pinned;
        let board_id = board.spec.id.clone();
        self.queue_board_config_save(BoardConfigChange::Pinned(board_id));
        self.mark_board_surface_dirty();
        true
    }

    pub(crate) fn reorder_board(&mut self, from: usize, to: usize) -> bool {
        if !self.boards.move_board(from, to) {
            return false;
        }
        self.queue_board_config_save(BoardConfigChange::Structure);
        self.mark_board_surface_dirty();
        true
    }

    pub(crate) fn reorder_page_in_board(
        &mut self,
        board_index: usize,
        from: usize,
        to: usize,
    ) -> bool {
        let is_active_board = self.boards.active_index() == board_index;
        let Some(board) = self.boards.board_states().get(board_index) else {
            return false;
        };
        let page_count = board.pages.page_count();
        if from >= page_count || to >= page_count {
            return false;
        }
        if is_active_board {
            self.prepare_active_page_content_change();
        }
        let Some(board) = self.boards.board_state_mut(board_index) else {
            return false;
        };
        let moved = board.pages.move_page(from, to);
        debug_assert!(moved, "preflighted page reorder failed on apply");
        self.finish_board_page_content_change(board_index);
        true
    }

    pub(crate) fn add_page_in_board(&mut self, board_index: usize) -> bool {
        let is_active_board = self.boards.active_index() == board_index;
        if self.boards.board_states().get(board_index).is_none() {
            return false;
        }
        if is_active_board {
            self.prepare_active_page_content_change();
        }
        let Some(board) = self.boards.board_state_mut(board_index) else {
            return false;
        };
        board.pages.new_page();
        let page_num = board.pages.active_index() + 1;
        let page_count = board.pages.page_count();
        let board_name = board.spec.name.clone();
        let board_id = board.spec.id.clone();
        self.finish_board_page_content_change(board_index);
        self.push_toast(
            ToastPriority::Info,
            "page.nav",
            Toast::info(format!(
                "Page added on '{board_name}' ({board_id}) ({page_num}/{page_count})"
            )),
        );
        true
    }

    pub(crate) fn duplicate_page_in_board(
        &mut self,
        board_index: usize,
        page_index: usize,
    ) -> bool {
        let is_active_board = self.boards.active_index() == board_index;
        let Some(board) = self.boards.board_states().get(board_index) else {
            return false;
        };
        if page_index >= board.pages.page_count() {
            return false;
        }
        if !self.session_allows_page_duplicate(board_index, page_index) {
            return false;
        }
        if is_active_board {
            self.prepare_active_page_content_change();
        }
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
        self.finish_board_page_content_change(board_index);
        self.push_toast(
            ToastPriority::Info,
            "page.nav",
            Toast::info(format!(
                "Page duplicated on '{board_name}' ({board_id}) ({page_num}/{page_count})"
            )),
        );
        true
    }

    pub(crate) fn rename_page_in_board(
        &mut self,
        board_index: usize,
        page_index: usize,
        name: Option<String>,
    ) -> bool {
        let is_active_board = self.boards.active_index() == board_index;
        let Some(board) = self.boards.board_states().get(board_index) else {
            return false;
        };
        if page_index >= board.pages.page_count() {
            return false;
        }
        if is_active_board {
            self.prepare_active_page_content_change();
        }
        let Some(board) = self.boards.board_state_mut(board_index) else {
            return false;
        };
        if !board.pages.set_page_name(page_index, name) {
            return false;
        }
        self.finish_board_page_content_change(board_index);
        self.push_toast(
            ToastPriority::Info,
            "page.nav",
            Toast::info("Page renamed."),
        );
        true
    }

    pub(crate) fn move_page_between_boards_with_activation(
        &mut self,
        source_board: usize,
        page_index: usize,
        target_board: usize,
        copy: bool,
        activate_target: bool,
    ) -> bool {
        if source_board == target_board {
            return false;
        }
        let board_count = self.boards.board_count();
        if source_board >= board_count || target_board >= board_count {
            return false;
        }
        let Some(source) = self.boards.board_states().get(source_board) else {
            return false;
        };
        if page_index >= source.pages.page_count() {
            return false;
        }
        if copy
            && !self.session_allows_page_copy_between_boards(source_board, page_index, target_board)
        {
            return false;
        }
        let active_board = self.boards.active_index();
        let active_involved = source_board == active_board || target_board == active_board;
        if active_involved {
            self.prepare_active_page_content_change();
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
        if active_involved {
            self.finish_active_page_content_change();
        } else {
            self.mark_board_surface_changed();
        }

        let action = if copy { "copied" } else { "moved" };
        self.push_toast(
            ToastPriority::Info,
            "page.nav",
            Toast::info(format!(
                "Page {action} to '{target_name}' ({target_id}) ({}/{})",
                new_index + 1,
                target_count
            )),
        );
        self.mark_session_dirty();
        if activate_target {
            self.switch_board_slot(target_board);
            if let Some(row) = self.board_picker_row_for_board(target_board) {
                self.board_picker_set_selected(row);
            }
        }
        true
    }

    pub fn page_prev(&mut self) -> bool {
        if self.boards.active_page_index() == 0 {
            return false;
        }
        self.prepare_active_page_content_change();
        let switched = self.boards.prev_page();
        debug_assert!(switched, "preflighted previous page failed on apply");
        self.finish_active_page_content_change();
        true
    }

    pub fn page_next(&mut self) -> bool {
        if self.boards.active_page_index() + 1 >= self.boards.page_count() {
            return false;
        }
        self.prepare_active_page_content_change();
        let switched = self.boards.next_page();
        debug_assert!(switched, "preflighted next page failed on apply");
        self.finish_active_page_content_change();
        true
    }

    pub fn switch_to_page(&mut self, index: usize) -> bool {
        if index >= self.boards.page_count() || index == self.boards.active_page_index() {
            return false;
        }
        self.prepare_active_page_content_change();
        let switched = self.boards.active_pages_mut().switch_to_page(index);
        debug_assert!(switched, "preflighted page switch failed on apply");
        self.finish_active_page_content_change();
        true
    }

    pub fn page_new(&mut self) {
        self.prepare_active_page_content_change();
        self.boards.new_page();
        self.finish_active_page_content_change();
        let page_num = self.boards.active_page_index() + 1;
        let page_count = self.boards.page_count();
        self.push_toast(
            ToastPriority::Info,
            "page.nav",
            Toast::info(format!("Page created ({page_num}/{page_count})")),
        );
    }

    pub fn page_duplicate(&mut self) {
        let before_page = self.boards.active_page_index();
        if !self.session_allows_page_duplicate(self.boards.active_index(), before_page) {
            return;
        }
        self.prepare_active_page_content_change();
        self.boards.duplicate_page();
        self.finish_active_page_content_change();
        let page_num = self.boards.active_page_index() + 1;
        let page_count = self.boards.page_count();
        self.push_toast(
            ToastPriority::Info,
            "page.nav",
            Toast::info(format!("Page duplicated ({page_num}/{page_count})")),
        );
    }
}
