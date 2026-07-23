use super::super::base::InputState;
use crate::input::boards::{BoardConfigChange, PendingBoardRuntimeUiAction};

impl InputState {
    pub(super) fn mark_board_surface_dirty(&mut self) {
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(super) fn mark_board_surface_changed(&mut self) {
        self.mark_board_surface_dirty();
        self.mark_session_dirty();
    }

    pub(super) fn finish_active_board_transition(&mut self) {
        self.sync_canvas_pointer_to_current_transform();
        self.mark_board_surface_changed();
    }

    pub(crate) fn queue_board_config_save(&mut self, change: BoardConfigChange) {
        if !self.boards.persist_customizations() {
            return;
        }
        let snapshot = self.boards.to_config();
        if let Some(update) = &mut self.pending_board_config {
            update.merge(snapshot, change);
        } else {
            self.pending_board_config = Some(crate::input::boards::PendingBoardConfigUpdate::new(
                snapshot, change,
            ));
        }
    }

    pub(super) fn queue_board_runtime_ui_action(&mut self, action: PendingBoardRuntimeUiAction) {
        self.pending_board_runtime_ui.push(action);
    }

    pub(super) fn queue_board_identity_available(&mut self, board_id: &str) {
        let Some(board) = self
            .boards
            .board_states()
            .iter()
            .find(|board| board.spec.id == board_id)
        else {
            return;
        };
        let pinned = board.spec.pinned;
        let pin_seed = self.boards.pin_seed(board_id).unwrap_or(pinned);
        self.queue_board_runtime_ui_action(PendingBoardRuntimeUiAction::IdentityAvailable {
            board_id: board_id.to_string(),
            pin_seed,
            pinned,
        });
    }

    pub(super) fn prepare_active_page_content_change(&mut self) {
        self.cancel_active_interaction();
    }

    pub(super) fn finish_active_page_content_change(&mut self) {
        self.clear_selection();
        self.close_context_menu();
        self.invalidate_hit_cache();
        self.sync_canvas_pointer_to_current_transform();
        self.mark_board_surface_changed();
    }

    pub(super) fn finish_board_page_content_change(&mut self, board_index: usize) {
        if self.boards.active_index() == board_index {
            self.finish_active_page_content_change();
        } else {
            self.mark_board_surface_changed();
        }
    }
}
