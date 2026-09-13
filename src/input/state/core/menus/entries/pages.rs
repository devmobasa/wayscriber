use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};
use crate::domain::Action;

/// Maximum number of pages to show in the submenu before truncating.
const MAX_VISIBLE_PAGES: usize = 8;

impl InputState {
    /// The active page position, shown on the parent row of the pages submenu.
    pub(super) fn pages_summary(&self) -> String {
        format!(
            "Page {}/{}",
            self.boards.active_page_index() + 1,
            self.boards.page_count().max(1)
        )
    }

    pub(super) fn pages_menu_entries(&self, with_header: bool) -> Vec<ContextMenuEntry> {
        let page_count = self.boards.page_count();
        let page_index = self.boards.active_page_index();
        let can_prev = page_index > 0;
        let can_next = page_index + 1 < page_count;

        let mut entries = Vec::new();

        // Current page indicator
        if with_header {
            entries.push(ContextMenuEntry::new(
                format!(
                    "{} - {}",
                    self.boards.active_board_name(),
                    self.pages_summary()
                ),
                None::<String>,
                true,
                None,
            ));
        }

        // List pages for quick switching (limited to MAX_VISIBLE_PAGES)
        // Window around the active page index
        let show_count = page_count.min(MAX_VISIBLE_PAGES);
        let start = if page_count > show_count {
            let half = show_count / 2;
            page_index.saturating_sub(half).min(page_count - show_count)
        } else {
            0
        };
        let end = start + show_count;

        // Pages outside the window are reachable through the board picker's
        // page panel, like the boards submenu's overflow rows.
        if start > 0 {
            entries.push(ContextMenuEntry::new(
                format!("  ... {} above (open picker)", start),
                self.shortcut_for_action(Action::BoardPicker),
                false,
                Some(MenuCommand::OpenBoardPicker),
            ));
        }

        for i in start..end {
            let is_active = i == page_index;
            let label = if is_active {
                format!("  Page {} (current)", i + 1)
            } else {
                format!("  Page {}", i + 1)
            };
            entries.push(ContextMenuEntry::new(
                label,
                None::<String>,
                is_active,
                Some(MenuCommand::SwitchToPage(i)),
            ));
        }

        if end < page_count {
            entries.push(ContextMenuEntry::new(
                format!("  ... {} below (open picker)", page_count - end),
                self.shortcut_for_action(Action::BoardPicker),
                false,
                Some(MenuCommand::OpenBoardPicker),
            ));
        }

        // Navigation
        entries.push(ContextMenuEntry::new(
            "Previous Page",
            self.shortcut_for_action(Action::PagePrev),
            !can_prev,
            Some(MenuCommand::PagePrev),
        ));
        entries.push(ContextMenuEntry::new(
            "Next Page",
            self.shortcut_for_action(Action::PageNext),
            !can_next,
            Some(MenuCommand::PageNext),
        ));

        // Management
        entries.push(ContextMenuEntry::new(
            "New Page",
            self.shortcut_for_action(Action::PageNew),
            false,
            Some(MenuCommand::PageNew),
        ));
        entries.push(ContextMenuEntry::new(
            "Duplicate Page",
            self.shortcut_for_action(Action::PageDuplicate),
            false,
            Some(MenuCommand::PageDuplicate),
        ));
        entries.push(ContextMenuEntry::new(
            "Delete Page",
            self.shortcut_for_action(Action::PageDelete),
            false,
            Some(MenuCommand::PageDelete),
        ));

        entries
    }
}
