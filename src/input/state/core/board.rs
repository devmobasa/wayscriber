use super::base::{InputState, UiToastKind};
use crate::draw::Color;
use crate::input::{BOARD_ID_TRANSPARENT, BoardBackground};

const BOARD_RECENT_LIMIT: usize = 5;

impl InputState {
    /// Returns the active board id.
    pub fn board_id(&self) -> &str {
        self.boards.active_board_id()
    }

    /// Returns the active board display name.
    pub fn board_name(&self) -> &str {
        self.boards.active_board_name()
    }

    /// Returns true if the active board is transparent.
    pub fn board_is_transparent(&self) -> bool {
        self.boards.active_background().is_transparent()
    }

    /// Switches to a different board with color auto-adjustment.
    ///
    /// Handles board transitions with automatic color adjustment for contrast:
    /// - Entering auto-adjust board: saves current color, applies board default
    /// - Exiting auto-adjust board: restores previous color
    /// - Switching between auto-adjust boards: applies new board default
    ///
    /// Also resets drawing state to prevent partial shapes crossing modes.
    pub fn switch_board(&mut self, target_id: &str) {
        self.switch_board_internal(target_id, true);
    }

    /// Switches to a different board without toggle semantics.
    pub fn switch_board_force(&mut self, target_id: &str) {
        self.switch_board_internal(target_id, false);
    }

    fn switch_board_internal(&mut self, target_id: &str, allow_toggle: bool) {
        let current_id = self.boards.active_board_id().to_string();

        // Toggle behavior: if already in target board, return to transparent.
        let mut target_id = target_id.to_string();
        if allow_toggle && current_id == target_id && !self.board_is_transparent() {
            target_id = BOARD_ID_TRANSPARENT.to_string();
        }

        if current_id == target_id {
            return;
        }

        self.switch_board_with(|boards| boards.switch_to_id(&target_id), &current_id);
    }

    pub fn create_board(&mut self) -> bool {
        let current_id = self.boards.active_board_id().to_string();
        let created = self.switch_board_with(|boards| boards.create_board(), &current_id);
        if created {
            self.queue_board_config_save();
        }
        created
    }

    pub fn switch_board_slot(&mut self, slot: usize) {
        let current_id = self.boards.active_board_id().to_string();
        self.switch_board_with(|boards| boards.switch_to_slot(slot), &current_id);
    }

    pub fn switch_board_next(&mut self) {
        let current_id = self.boards.active_board_id().to_string();
        self.switch_board_with(|boards| boards.next_board(), &current_id);
    }

    pub fn switch_board_prev(&mut self) {
        let current_id = self.boards.active_board_id().to_string();
        self.switch_board_with(|boards| boards.prev_board(), &current_id);
    }

    pub fn delete_active_board(&mut self) {
        if self.board_is_transparent() {
            self.set_ui_toast(UiToastKind::Info, "Overlay board cannot be deleted.");
            return;
        }
        if self.boards.active_pages().has_persistable_data() {
            self.set_ui_toast(
                UiToastKind::Warning,
                "Board has content; deletion requires clearing first.",
            );
            return;
        }
        if self.boards.board_count() <= 1 {
            self.set_ui_toast(UiToastKind::Info, "At least one board must remain.");
            return;
        }

        let current_id = self.boards.active_board_id().to_string();
        if self.switch_board_with(|boards| boards.remove_active_board(), &current_id) {
            self.remove_board_recent(&current_id);
            self.queue_board_config_save();
        }
    }

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
            board.spec.default_pen_color = Some(contrast_color(color));
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

    fn switch_board_with(
        &mut self,
        switch: impl FnOnce(&mut crate::input::BoardManager) -> bool,
        current_id: &str,
    ) -> bool {
        let current_spec = self.boards.active_board().spec.clone();
        let prev_count = self.boards.board_count();
        if !switch(&mut self.boards) {
            return false;
        }

        let target_spec = self.boards.active_board().spec.clone();
        if target_spec.id == current_id {
            return false;
        }
        if self.boards.board_count() > prev_count {
            self.queue_board_config_save();
        }
        self.record_board_recent(&target_spec.id);

        let current_auto =
            current_spec.auto_adjust_pen && !current_spec.background.is_transparent();
        let target_auto = target_spec.auto_adjust_pen && !target_spec.background.is_transparent();

        match (current_auto, target_auto) {
            (false, true) => {
                self.board_previous_color = Some(self.current_color);
                if let Some(default_color) = target_spec.effective_pen_color() {
                    self.current_color = default_color;
                    self.sync_highlight_color();
                }
            }
            (true, false) => {
                if let Some(prev_color) = self.board_previous_color.take() {
                    self.current_color = prev_color;
                    self.sync_highlight_color();
                }
            }
            (true, true) => {
                if let Some(default_color) = target_spec.effective_pen_color() {
                    self.current_color = default_color;
                    self.sync_highlight_color();
                }
            }
            _ => {}
        }

        // Reset drawing state to prevent partial shapes crossing modes
        self.state = super::base::DrawingState::Idle;

        // Trigger redraw
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;

        log::info!(
            "Switched from '{}' to '{}' board",
            current_id,
            target_spec.id
        );
        true
    }

    fn record_board_recent(&mut self, board_id: &str) {
        self.board_recent.retain(|id| id != board_id);
        self.board_recent.insert(0, board_id.to_string());
        if self.board_recent.len() > BOARD_RECENT_LIMIT {
            self.board_recent.truncate(BOARD_RECENT_LIMIT);
        }
    }

    fn remove_board_recent(&mut self, board_id: &str) {
        self.board_recent.retain(|id| id != board_id);
    }

    pub(crate) fn queue_board_config_save(&mut self) {
        if !self.boards.persist_customizations() {
            return;
        }
        self.pending_board_config = Some(self.boards.to_config());
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

    pub fn page_new(&mut self) {
        self.boards.new_page();
        self.prepare_page_switch();
    }

    pub fn page_duplicate(&mut self) {
        self.boards.duplicate_page();
        self.prepare_page_switch();
    }

    pub fn page_delete(&mut self) -> crate::draw::PageDeleteOutcome {
        let outcome = self.boards.delete_page();
        self.prepare_page_switch();
        outcome
    }
}

fn contrast_color(background: Color) -> Color {
    let luminance = 0.2126 * background.r + 0.7152 * background.g + 0.0722 * background.b;
    if luminance > 0.5 {
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }
    } else {
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }
}
