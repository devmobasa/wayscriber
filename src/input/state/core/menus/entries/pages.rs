use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};

impl InputState {
    pub(super) fn pages_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let page_count = self.boards.page_count();
        let page_index = self.boards.active_page_index();
        let can_prev = page_index > 0;
        let can_next = page_index + 1 < page_count;

        vec![
            ContextMenuEntry::new(
                format!("Page {}/{}", page_index + 1, page_count.max(1)),
                None::<String>,
                false,
                true,
                None,
            ),
            ContextMenuEntry::new(
                "Previous Page",
                None::<String>,
                false,
                !can_prev,
                Some(MenuCommand::PagePrev),
            ),
            ContextMenuEntry::new(
                "Next Page",
                None::<String>,
                false,
                !can_next,
                Some(MenuCommand::PageNext),
            ),
            ContextMenuEntry::new(
                "New Page",
                Some("Ctrl+Alt+N"),
                false,
                false,
                Some(MenuCommand::PageNew),
            ),
            ContextMenuEntry::new(
                "Duplicate Page",
                Some("Ctrl+Alt+D"),
                false,
                false,
                Some(MenuCommand::PageDuplicate),
            ),
            ContextMenuEntry::new(
                "Delete Page",
                Some("Ctrl+Alt+Delete"),
                false,
                false,
                Some(MenuCommand::PageDelete),
            ),
        ]
    }
}
