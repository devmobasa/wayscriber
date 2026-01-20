use super::base::{
    BOARD_DELETE_CONFIRM_MS, BOARD_UNDO_EXPIRE_MS, InputState, PAGE_DELETE_CONFIRM_MS,
    PAGE_UNDO_EXPIRE_MS, PendingBoardDelete, PendingPageDelete, UiToastKind,
};
use crate::config::Action;
use crate::draw::Color;
use crate::input::{BOARD_ID_TRANSPARENT, BoardBackground};
use std::time::{Duration, Instant};

const BOARD_RECENT_LIMIT: usize = 5;

impl InputState {
    /// Returns true if there's a pending board deletion confirmation.
    pub fn has_pending_board_delete(&self) -> bool {
        self.pending_board_delete.is_some()
    }

    /// Cancels pending board deletion and clears the toast.
    pub fn cancel_pending_board_delete(&mut self) {
        if self.pending_board_delete.is_some() {
            self.pending_board_delete = None;
            self.ui_toast = None;
            self.set_ui_toast(UiToastKind::Info, "Board deletion cancelled.");
        }
    }

    /// Returns true if there's a pending page deletion confirmation.
    pub fn has_pending_page_delete(&self) -> bool {
        self.pending_page_delete.is_some()
    }

    /// Cancels pending page deletion and clears the toast.
    pub fn cancel_pending_page_delete(&mut self) {
        if self.pending_page_delete.is_some() {
            self.pending_page_delete = None;
            self.ui_toast = None;
            self.set_ui_toast(UiToastKind::Info, "Page deletion cancelled.");
        }
    }

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
            let name = self.boards.active_board_name().to_string();
            self.queue_board_config_save();
            self.set_ui_toast(UiToastKind::Info, format!("Board created: {name}"));
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
            self.pending_board_delete = None;
            return;
        }
        if self.boards.board_count() <= 1 {
            self.set_ui_toast(UiToastKind::Info, "At least one board must remain.");
            self.pending_board_delete = None;
            return;
        }

        let current_id = self.boards.active_board_id().to_string();
        let now = Instant::now();
        if self
            .pending_board_delete
            .as_ref()
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_board_delete = None;
        }
        let confirmed = self
            .pending_board_delete
            .as_ref()
            .is_some_and(|pending| pending.board_id == current_id && now <= pending.expires_at);
        if !confirmed {
            let name = self.boards.active_board_name();
            self.pending_board_delete = Some(PendingBoardDelete {
                board_id: current_id,
                expires_at: now + Duration::from_millis(BOARD_DELETE_CONFIRM_MS),
            });
            self.set_ui_toast_with_action_and_duration(
                UiToastKind::Warning,
                format!("Delete board '{name}'? Click to confirm."),
                "Delete",
                Action::BoardDelete,
                BOARD_DELETE_CONFIRM_MS,
            );
            return;
        }
        self.pending_board_delete = None;

        // Save board state before deletion for potential undo
        let deleted_board = self.boards.active_board().clone();
        let deleted_name = deleted_board.spec.name.clone();

        if self.switch_board_with(|boards| boards.remove_active_board(), &current_id) {
            self.remove_board_recent(&current_id);
            self.queue_board_config_save();

            // Store for undo
            self.deleted_boards.push((deleted_board, Instant::now()));

            // Show toast with undo action
            self.set_ui_toast_with_action(
                UiToastKind::Info,
                format!("Board deleted: {deleted_name}"),
                "Undo",
                Action::BoardRestoreDeleted,
            );
        }
    }

    /// Restore the most recently deleted board.
    pub fn restore_deleted_board(&mut self) {
        // Expire old entries first
        self.expire_deleted_boards();

        let Some((board, _timestamp)) = self.deleted_boards.pop() else {
            self.set_ui_toast(UiToastKind::Info, "No deleted board to restore.");
            return;
        };

        let name = board.spec.name.clone();
        let current_id = self.boards.active_board_id().to_string();

        if self.switch_board_with(
            |boards| boards.insert_board(boards.board_count(), board),
            &current_id,
        ) {
            self.queue_board_config_save();
            self.set_ui_toast(UiToastKind::Info, format!("Board restored: {name}"));
        } else {
            self.set_ui_toast(UiToastKind::Warning, "Board limit reached; cannot restore.");
        }
    }

    /// Remove deleted boards that have expired (older than BOARD_UNDO_EXPIRE_MS).
    fn expire_deleted_boards(&mut self) {
        let now = Instant::now();
        let expire_duration = std::time::Duration::from_millis(BOARD_UNDO_EXPIRE_MS);
        self.deleted_boards.retain(|(_board, timestamp)| {
            now.saturating_duration_since(*timestamp) < expire_duration
        });
    }

    /// Duplicate the active board.
    pub fn duplicate_board(&mut self) {
        if self.board_is_transparent() {
            self.set_ui_toast(UiToastKind::Info, "Overlay board cannot be duplicated.");
            return;
        }

        let current_id = self.boards.active_board_id().to_string();
        if let Some(new_id) = self.boards.duplicate_active_board() {
            self.record_board_recent(&new_id);
            self.queue_board_config_save();
            let name = self.boards.active_board_name();
            self.set_ui_toast(UiToastKind::Info, format!("Board duplicated: {name}"));

            // Handle color auto-adjustment for the duplicated board
            let current_spec = self.boards.active_board().spec.clone();
            let target_auto =
                current_spec.auto_adjust_pen && !current_spec.background.is_transparent();
            if target_auto && let Some(default_color) = current_spec.effective_pen_color() {
                self.current_color = default_color;
                self.sync_highlight_color();
            }

            // Reset drawing state
            self.state = super::base::DrawingState::Idle;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();

            log::info!("Duplicated board '{}' to '{}'", current_id, new_id);
        } else {
            self.set_ui_toast(UiToastKind::Info, "Board limit reached.");
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
            self.set_ui_toast(UiToastKind::Info, "No recent board to switch to.");
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
        self.pending_board_delete = None;
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
        self.mark_session_dirty();

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
        self.mark_session_dirty();
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

    pub fn page_delete(&mut self) -> crate::draw::PageDeleteOutcome {
        let page_count = self.boards.page_count();
        let page_index = self.boards.active_page_index();
        let board_id = self.boards.active_board_id().to_string();

        // If this is the last page, skip confirmation (just clears)
        if page_count <= 1 {
            let outcome = self.boards.delete_page();
            self.prepare_page_switch();
            self.set_ui_toast(UiToastKind::Info, "Page cleared (last page)");
            return outcome;
        }

        let now = Instant::now();

        // Expire old pending confirmation
        if self
            .pending_page_delete
            .as_ref()
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_page_delete = None;
        }

        // Check if we have a valid pending confirmation
        let confirmed = self.pending_page_delete.as_ref().is_some_and(|pending| {
            pending.board_id == board_id
                && pending.page_index == page_index
                && now <= pending.expires_at
        });

        if !confirmed {
            // First press: request confirmation
            self.pending_page_delete = Some(PendingPageDelete {
                board_id,
                page_index,
                expires_at: now + Duration::from_millis(PAGE_DELETE_CONFIRM_MS),
            });
            self.set_ui_toast_with_action_and_duration(
                UiToastKind::Warning,
                format!(
                    "Delete page {}/{}? Click to confirm.",
                    page_index + 1,
                    page_count
                ),
                "Delete",
                Action::PageDelete,
                PAGE_DELETE_CONFIRM_MS,
            );
            return crate::draw::PageDeleteOutcome::Pending;
        }

        // Confirmed: proceed with deletion
        self.pending_page_delete = None;

        // Save page before deletion for undo
        let deleted_page = self.boards.active_pages().active_frame().clone();

        let outcome = self.boards.delete_page();
        self.prepare_page_switch();
        let new_page_num = self.boards.active_page_index() + 1;
        let new_page_count = self.boards.page_count();

        if matches!(outcome, crate::draw::PageDeleteOutcome::Removed) {
            // Store for undo
            let board_id = self.boards.active_board_id().to_string();
            self.deleted_pages
                .push((board_id, deleted_page, Instant::now()));

            // Show toast with undo action
            self.set_ui_toast_with_action(
                UiToastKind::Info,
                format!("Page deleted ({new_page_num}/{new_page_count})"),
                "Undo",
                Action::PageRestoreDeleted,
            );
        }
        outcome
    }

    /// Restore the most recently deleted page.
    pub fn restore_deleted_page(&mut self) {
        // Expire old entries
        let now = Instant::now();
        let expire_duration = Duration::from_millis(PAGE_UNDO_EXPIRE_MS);
        self.deleted_pages.retain(|(_, _, deleted_at)| {
            now.saturating_duration_since(*deleted_at) < expire_duration
        });

        // Get the most recent deleted page for the current board
        let board_id = self.boards.active_board_id().to_string();
        let position = self
            .deleted_pages
            .iter()
            .rposition(|(id, _, _)| id == &board_id);

        if let Some(idx) = position {
            let (_, page, _) = self.deleted_pages.remove(idx);
            self.boards.insert_page(page);
            self.prepare_page_switch();
            let page_num = self.boards.active_page_index() + 1;
            let page_count = self.boards.page_count();
            self.set_ui_toast(
                UiToastKind::Info,
                format!("Page restored ({page_num}/{page_count})"),
            );
        } else {
            self.set_ui_toast(UiToastKind::Info, "No deleted page to restore.");
        }
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
