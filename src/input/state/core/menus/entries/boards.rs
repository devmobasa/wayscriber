use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};
use crate::domain::Action;

/// Maximum number of boards to show in the submenu before truncating.
const MAX_VISIBLE_BOARDS: usize = 8;

impl InputState {
    pub(super) fn boards_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let board_count = self.boards.board_count();
        let board_index = self.boards.active_index();
        let can_prev = board_count > 1;
        let can_next = board_count > 1;

        let mut entries = Vec::new();

        // Current board indicator
        let current_name = self.boards.active_board_name();
        entries.push(ContextMenuEntry::new(
            format!("{} ({}/{})", current_name, board_index + 1, board_count),
            None::<String>,
            false,
            true,
            None,
        ));

        // List boards for quick switching (limited to MAX_VISIBLE_BOARDS)
        let boards = self.boards.board_states();
        let show_count = boards.len().min(MAX_VISIBLE_BOARDS);
        let start = if boards.len() > show_count {
            let half = show_count / 2;
            board_index
                .saturating_sub(half)
                .min(boards.len() - show_count)
        } else {
            0
        };
        let end = start + show_count;

        if start > 0 {
            entries.push(ContextMenuEntry::new(
                format!("  ... {} above (open picker)", start),
                self.shortcut_for_action(Action::BoardPicker),
                false,
                false,
                Some(MenuCommand::OpenBoardPicker),
            ));
        }

        for (index, board) in boards.iter().enumerate().take(end).skip(start) {
            let is_active = index == board_index;
            let label = if is_active {
                format!("  {} (current)", board.spec.name)
            } else {
                format!("  {}", board.spec.name)
            };
            entries.push(ContextMenuEntry::new(
                label,
                None::<String>,
                false,
                is_active,
                Some(MenuCommand::SwitchToBoard {
                    id: board.spec.id.clone(),
                }),
            ));
        }

        if end < board_count {
            entries.push(ContextMenuEntry::new(
                format!("  ... {} below (open picker)", board_count - end),
                self.shortcut_for_action(Action::BoardPicker),
                false,
                false,
                Some(MenuCommand::OpenBoardPicker),
            ));
        }

        // Navigation
        entries.push(ContextMenuEntry::new(
            "Previous Board",
            self.shortcut_for_action(Action::BoardPrev),
            false,
            !can_prev,
            Some(MenuCommand::BoardPrev),
        ));
        entries.push(ContextMenuEntry::new(
            "Next Board",
            self.shortcut_for_action(Action::BoardNext),
            false,
            !can_next,
            Some(MenuCommand::BoardNext),
        ));

        // Management
        entries.push(ContextMenuEntry::new(
            "New Board",
            self.shortcut_for_action(Action::BoardNew),
            false,
            false,
            Some(MenuCommand::BoardNew),
        ));
        entries.push(ContextMenuEntry::new(
            "Duplicate Board",
            self.shortcut_for_action(Action::BoardDuplicate),
            false,
            false,
            Some(MenuCommand::BoardDuplicate),
        ));
        // The overlay has no paper to edit.
        entries.push(ContextMenuEntry::new(
            "Edit Board Paper…",
            self.shortcut_for_action(Action::BoardPaperEdit),
            false,
            self.board_is_transparent(),
            Some(MenuCommand::BoardEditPaper),
        ));

        // Can't delete the transparent board or if only one board left
        let can_delete = !self.board_is_transparent() && board_count > 1;
        entries.push(ContextMenuEntry::new(
            "Delete Board",
            self.shortcut_for_action(Action::BoardDelete),
            false,
            !can_delete,
            Some(MenuCommand::BoardDelete),
        ));

        entries
    }

    /// Actions for the board row right-clicked in the board picker. Shortcut
    /// hints name the picker's own keys for the same actions.
    pub(super) fn board_context_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let Some(board) = self.context_menu.board_target.as_deref().and_then(|id| {
            self.boards
                .board_states()
                .iter()
                .find(|board| board.spec.id == id)
        }) else {
            return Vec::new();
        };
        let pin_label = if board.spec.pinned {
            "Unpin Board"
        } else {
            "Pin Board"
        };

        vec![
            ContextMenuEntry::new(board.spec.name.clone(), None::<String>, false, true, None),
            ContextMenuEntry::new(
                "Edit Paper…",
                Some("Ctrl+C"),
                false,
                board.spec.background.is_transparent(),
                Some(MenuCommand::BoardEditPaperFromContext),
            ),
            ContextMenuEntry::new(
                "Rename Board",
                Some("F2"),
                false,
                false,
                Some(MenuCommand::BoardRenameFromContext),
            ),
            ContextMenuEntry::new(
                pin_label,
                Some("Ctrl+P"),
                false,
                false,
                Some(MenuCommand::BoardTogglePinFromContext),
            ),
        ]
    }
}
