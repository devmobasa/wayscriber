use super::super::base::InputState;
use crate::input::boards::BoardConfigChange;
use crate::input::state::{Toast, ToastPriority};
use crate::input::{BOARD_ID_TRANSPARENT, BoardSpec};

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

        self.switch_board_with(
            |boards| boards.can_switch_to_id(&target_id),
            |boards| boards.switch_to_id(&target_id),
            &current_id,
        );
    }

    pub fn create_board(&mut self) -> bool {
        let current_id = self.boards.active_board_id().to_string();
        let created = self.switch_board_with(
            |boards| boards.board_count() < boards.max_count(),
            |boards| boards.create_board(),
            &current_id,
        );
        if created {
            let name = self.boards.active_board_name().to_string();
            self.push_toast(
                ToastPriority::Info,
                "board.switch",
                Toast::info(format!("Board created: {name}")),
            );
        }
        created
    }

    pub fn switch_board_slot(&mut self, slot: usize) {
        let current_id = self.boards.active_board_id().to_string();
        self.switch_board_with(
            |boards| boards.can_switch_to_slot(slot),
            |boards| boards.switch_to_slot(slot),
            &current_id,
        );
    }

    pub fn switch_board_next(&mut self) {
        let current_id = self.boards.active_board_id().to_string();
        self.switch_board_with(
            |boards| boards.board_count() > 1,
            |boards| boards.next_board(),
            &current_id,
        );
    }

    pub fn switch_board_prev(&mut self) {
        let current_id = self.boards.active_board_id().to_string();
        self.switch_board_with(
            |boards| boards.board_count() > 1,
            |boards| boards.prev_board(),
            &current_id,
        );
    }

    /// Duplicate the active board.
    pub fn duplicate_board(&mut self) {
        if self.board_is_transparent() {
            self.push_toast(
                ToastPriority::Info,
                "board.switch",
                Toast::info("Overlay board cannot be duplicated."),
            );
            return;
        }

        let current_id = self.boards.active_board_id().to_string();
        if self.boards.board_count() >= self.boards.max_count() {
            self.push_toast(
                ToastPriority::Info,
                "board.switch",
                Toast::info("Board limit reached."),
            );
            return;
        }
        if !self.session_allows_board_duplicate() {
            return;
        }

        self.cancel_active_interaction();
        let generation_before = self.boards.board_identity_generation();
        if let Some(new_id) = self.boards.duplicate_active_board() {
            self.clear_pending_deletes_after_board_generation_change(generation_before);
            self.record_board_recent(&new_id);
            self.queue_board_config_save(BoardConfigChange::IdentitiesCreated(vec![
                new_id.clone(),
            ]));
            let name = self.boards.active_board_name();
            self.push_toast(
                ToastPriority::Info,
                "board.switch",
                Toast::info(format!("Board duplicated: {name}")),
            );

            // Handle color auto-adjustment for the duplicated board
            let current_spec = self.boards.active_board().spec.clone();
            let target_auto =
                current_spec.auto_adjust_pen && !current_spec.background.is_transparent();
            if target_auto && let Some(default_color) = current_spec.effective_pen_color() {
                self.set_pen_color_from_board(default_color);
            }

            self.finish_active_board_transition();

            log::info!("Duplicated board '{}' to '{}'", current_id, new_id);
        } else {
            self.push_toast(
                ToastPriority::Info,
                "board.switch",
                Toast::info("Board limit reached."),
            );
        }
    }

    /// Switch to the most recently used board (other than the current one).
    pub fn switch_board_recent(&mut self) {
        // Find the first recent board that isn't the current one
        let current_id = self.boards.active_board_id();
        let target = self
            .board_recent
            .iter()
            .find(|id| id.as_str() != current_id && self.boards.has_board(id))
            .cloned();

        if let Some(target_id) = target {
            self.switch_board_force(&target_id);
        } else {
            self.push_toast(
                ToastPriority::Info,
                "board.switch",
                Toast::info("No recent board to switch to."),
            );
        }
    }

    pub(super) fn switch_board_with(
        &mut self,
        can_switch: impl FnOnce(&crate::input::BoardManager) -> bool,
        switch: impl FnOnce(&mut crate::input::BoardManager) -> bool,
        current_id: &str,
    ) -> bool {
        if !can_switch(&self.boards) {
            return false;
        }

        let current_spec = self.boards.active_board().spec.clone();
        let ids_before = self
            .boards
            .board_states()
            .iter()
            .map(|board| board.spec.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let generation_before = self.boards.board_identity_generation();
        self.cancel_active_interaction();
        let switched = switch(&mut self.boards);
        debug_assert!(switched, "preflighted board transition failed on apply");
        if !switched {
            return false;
        }

        let target_spec = self.boards.active_board().spec.clone();
        debug_assert_ne!(target_spec.id, current_id);
        if target_spec.id == current_id {
            return false;
        }
        let created_ids = self
            .boards
            .board_states()
            .iter()
            .filter(|board| !ids_before.contains(&board.spec.id))
            .map(|board| board.spec.id.clone())
            .collect::<Vec<_>>();
        if !created_ids.is_empty() {
            self.queue_board_config_save(BoardConfigChange::IdentitiesCreated(created_ids));
        }
        self.clear_pending_deletes_after_board_generation_change(generation_before);
        self.finish_board_transition_from(current_spec, current_id, true);
        true
    }

    pub(super) fn finish_board_transition_from(
        &mut self,
        current_spec: BoardSpec,
        current_id: &str,
        log_switch: bool,
    ) {
        let target_spec = self.boards.active_board().spec.clone();
        self.record_board_recent(&target_spec.id);

        let current_auto =
            current_spec.auto_adjust_pen && !current_spec.background.is_transparent();
        let target_auto = target_spec.auto_adjust_pen && !target_spec.background.is_transparent();

        match (current_auto, target_auto) {
            (false, true) => {
                self.board_previous_color = Some(self.color_for_tool(crate::input::Tool::Pen));
                if let Some(default_color) = target_spec.effective_pen_color() {
                    self.set_pen_color_from_board(default_color);
                }
            }
            (true, false) => {
                if let Some(prev_color) = self.board_previous_color.take() {
                    self.set_pen_color_from_board(prev_color);
                }
            }
            (true, true) => {
                if let Some(default_color) = target_spec.effective_pen_color() {
                    self.set_pen_color_from_board(default_color);
                }
            }
            _ => {}
        }

        if self.is_board_picker_open() {
            let active_index = self.boards.active_index();
            if let Some(row) = self.board_picker_row_for_board(active_index) {
                self.board_picker_set_selected(row);
            }
            if let super::super::board_picker::BoardPickerState::Open { hover_index, .. } =
                &mut self.board_picker_state
            {
                *hover_index = None;
            }
        }

        self.finish_active_board_transition();

        if log_switch {
            log::info!(
                "Switched from '{}' to '{}' board",
                current_id,
                target_spec.id
            );
        }
    }

    fn record_board_recent(&mut self, board_id: &str) {
        self.board_recent.retain(|id| id != board_id);
        self.board_recent.insert(0, board_id.to_string());
        if self.board_recent.len() > super::BOARD_RECENT_LIMIT {
            self.board_recent.truncate(super::BOARD_RECENT_LIMIT);
        }
    }

    pub(super) fn remove_board_recent(&mut self, board_id: &str) {
        self.board_recent.retain(|id| id != board_id);
    }
}
