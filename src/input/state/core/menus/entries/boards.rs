use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};

/// Maximum number of boards to show in the submenu before truncating.
const MAX_VISIBLE_BOARDS: usize = 8;

impl InputState {
    pub(super) fn boards_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let board_count = self.boards.board_count();
        let board_index = self.boards.active_index();
        let can_prev = board_index > 0;
        let can_next = board_index + 1 < board_count;

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

        for (index, board) in boards.iter().enumerate().take(show_count) {
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

        // Show truncation indicator if there are more boards
        if board_count > MAX_VISIBLE_BOARDS {
            entries.push(ContextMenuEntry::new(
                format!("  ... and {} more", board_count - MAX_VISIBLE_BOARDS),
                None::<String>,
                false,
                true,
                None,
            ));
        }

        // Navigation
        entries.push(ContextMenuEntry::new(
            "Previous Board",
            Some("Ctrl+Shift+["),
            false,
            !can_prev,
            Some(MenuCommand::BoardPrev),
        ));
        entries.push(ContextMenuEntry::new(
            "Next Board",
            Some("Ctrl+Shift+]"),
            false,
            !can_next,
            Some(MenuCommand::BoardNext),
        ));

        // Management
        entries.push(ContextMenuEntry::new(
            "New Board",
            Some("Ctrl+Shift+N"),
            false,
            false,
            Some(MenuCommand::BoardNew),
        ));
        entries.push(ContextMenuEntry::new(
            "Duplicate Board",
            None::<String>,
            false,
            false,
            Some(MenuCommand::BoardDuplicate),
        ));

        // Can't delete the transparent board or if only one board left
        let can_delete = !self.board_is_transparent() && board_count > 1;
        entries.push(ContextMenuEntry::new(
            "Delete Board",
            None::<String>,
            false,
            !can_delete,
            Some(MenuCommand::BoardDelete),
        ));

        entries
    }
}
