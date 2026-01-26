use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};
use crate::config::Action;

/// Maximum number of pages to show in the submenu before truncating.
const MAX_VISIBLE_PAGES: usize = 8;

impl InputState {
    pub(super) fn pages_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let page_count = self.boards.page_count();
        let page_index = self.boards.active_page_index();
        let can_prev = page_index > 0;
        let can_next = page_index + 1 < page_count;

        let mut entries = Vec::new();

        // Current page indicator
        let board_name = self.boards.active_board_name();
        entries.push(ContextMenuEntry::new(
            format!(
                "{} - Page {}/{}",
                board_name,
                page_index + 1,
                page_count.max(1)
            ),
            None::<String>,
            false,
            true,
            None,
        ));

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

        // Show "above" indicator if there are pages before the window
        if start > 0 {
            entries.push(ContextMenuEntry::new(
                format!("  ... {} above", start),
                None::<String>,
                false,
                true,
                None,
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
                false,
                is_active,
                Some(MenuCommand::SwitchToPage(i)),
            ));
        }

        // Show "below" indicator if there are pages after the window
        if end < page_count {
            entries.push(ContextMenuEntry::new(
                format!("  ... {} below", page_count - end),
                None::<String>,
                false,
                true,
                None,
            ));
        }

        // Navigation
        entries.push(ContextMenuEntry::new(
            "Previous Page",
            self.shortcut_for_action(Action::PagePrev),
            false,
            !can_prev,
            Some(MenuCommand::PagePrev),
        ));
        entries.push(ContextMenuEntry::new(
            "Next Page",
            self.shortcut_for_action(Action::PageNext),
            false,
            !can_next,
            Some(MenuCommand::PageNext),
        ));

        // Management
        entries.push(ContextMenuEntry::new(
            "New Page",
            self.shortcut_for_action(Action::PageNew),
            false,
            false,
            Some(MenuCommand::PageNew),
        ));
        entries.push(ContextMenuEntry::new(
            "Duplicate Page",
            self.shortcut_for_action(Action::PageDuplicate),
            false,
            false,
            Some(MenuCommand::PageDuplicate),
        ));
        entries.push(ContextMenuEntry::new(
            "Delete Page",
            self.shortcut_for_action(Action::PageDelete),
            false,
            false,
            Some(MenuCommand::PageDelete),
        ));

        entries
    }
}
