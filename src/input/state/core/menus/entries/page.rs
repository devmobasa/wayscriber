use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};
use crate::config::Action;

impl InputState {
    pub(super) fn page_context_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let Some(target) = self.context_menu_page_target else {
            return Vec::new();
        };
        let Some(board) = self.boards.board_states().get(target.board_index) else {
            return Vec::new();
        };
        let page_count = board.pages.page_count();
        let page_number = target.page_index + 1;
        let page_name = board.pages.page_name(target.page_index);

        let mut entries = Vec::new();
        let header = if let Some(name) = page_name {
            format!(
                "{} — Page {} ({}/{})",
                name, page_number, page_number, page_count
            )
        } else {
            format!("Page {} ({}/{})", page_number, page_number, page_count)
        };
        entries.push(ContextMenuEntry::new(
            header,
            None::<String>,
            false,
            true,
            None,
        ));
        entries.push(ContextMenuEntry::new(
            "Rename Page",
            None::<String>,
            false,
            false,
            Some(MenuCommand::PageRename),
        ));
        entries.push(ContextMenuEntry::new(
            "Duplicate Page",
            self.shortcut_for_action(Action::PageDuplicate),
            false,
            false,
            Some(MenuCommand::PageDuplicateFromContext),
        ));
        entries.push(ContextMenuEntry::new(
            "Delete Page",
            self.shortcut_for_action(Action::PageDelete),
            false,
            false,
            Some(MenuCommand::PageDeleteFromContext),
        ));

        let can_move = self.boards.board_count() > 1;
        entries.push(ContextMenuEntry::new(
            "Move to Board",
            None::<String>,
            true,
            !can_move,
            Some(MenuCommand::OpenPageMoveMenu),
        ));
        entries
    }

    pub(super) fn page_move_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let Some(target) = self.context_menu_page_target else {
            return Vec::new();
        };
        let boards = self.boards.board_states();
        if boards.len() <= 1 {
            return vec![ContextMenuEntry::new(
                "No other boards",
                None::<String>,
                false,
                true,
                None,
            )];
        }

        let mut entries = Vec::new();
        for (index, board) in boards.iter().enumerate() {
            if index == target.board_index {
                continue;
            }
            entries.push(ContextMenuEntry::new(
                board.spec.name.clone(),
                None::<String>,
                false,
                false,
                Some(MenuCommand::PageMoveToBoard {
                    id: board.spec.id.clone(),
                }),
            ));
        }
        if entries.is_empty() {
            entries.push(ContextMenuEntry::new(
                "No other boards",
                None::<String>,
                false,
                true,
                None,
            ));
        }
        entries
    }
}
