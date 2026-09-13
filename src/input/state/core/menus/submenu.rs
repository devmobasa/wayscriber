//! Submenus open beside the parent row that holds them, and the parent menu
//! stays open underneath.

use super::super::base::InputState;
use super::types::{
    ContextMenuKind, ContextMenuLevel, ContextMenuState, ContextSubmenu, MenuCommand,
};

/// The menu a command asks for. Opened as a submenu when the open menu has a
/// row for it, otherwise on its own at the pointer.
pub(super) fn menu_kind_for_command(command: &MenuCommand) -> Option<ContextMenuKind> {
    match command {
        MenuCommand::OpenZoomMenu => Some(ContextMenuKind::Zoom),
        MenuCommand::OpenPagesMenu => Some(ContextMenuKind::Pages),
        MenuCommand::OpenBoardsMenu => Some(ContextMenuKind::Boards),
        MenuCommand::OpenPageMoveMenu => Some(ContextMenuKind::PageMove),
        _ => None,
    }
}

impl InputState {
    /// The open submenu, if any.
    pub fn context_submenu(&self) -> Option<ContextSubmenu> {
        match &self.context_menu.state {
            ContextMenuState::Open { submenu, .. } => *submenu,
            ContextMenuState::Hidden => None,
        }
    }

    /// The submenu an enabled row of the parent menu opens.
    pub(super) fn context_menu_row_submenu(&self, index: usize) -> Option<ContextMenuKind> {
        self.context_menu_entries()
            .get(index)
            .filter(|entry| !entry.disabled)?
            .submenu
    }

    /// Opens the menu `command` asks for: beside the parent row that holds it
    /// when the open menu has one, otherwise as its own menu at the pointer.
    pub(super) fn open_menu_for_command(&mut self, command: &MenuCommand) {
        let Some(kind) = menu_kind_for_command(command) else {
            return;
        };
        let row = self
            .context_menu_entries()
            .iter()
            .position(|entry| entry.submenu == Some(kind) && !entry.disabled);
        if let Some(row) = row {
            self.open_context_submenu_kind(row, kind, true);
            return;
        }
        // The board picker's page overflow link opens the pages menu with no
        // parent menu open.
        let page_target = self.context_menu.page_target;
        self.open_context_menu(self.pointer.screen(), Vec::new(), kind, None);
        self.context_menu.page_target = page_target;
        self.needs_redraw = true;
    }

    /// Opens the submenu of a parent row beside it. `focus` moves keyboard focus
    /// to its first enabled entry. Returns whether the row opens a submenu.
    pub(crate) fn open_context_submenu(&mut self, parent_index: usize, focus: bool) -> bool {
        match self.context_menu_row_submenu(parent_index) {
            Some(kind) => self.open_context_submenu_kind(parent_index, kind, focus),
            None => false,
        }
    }

    pub(super) fn open_context_submenu_kind(
        &mut self,
        parent_index: usize,
        kind: ContextMenuKind,
        focus: bool,
    ) -> bool {
        let ContextMenuState::Open { submenu, .. } = &mut self.context_menu.state else {
            return false;
        };
        if submenu.is_none_or(|open| open.parent_index != parent_index) {
            *submenu = Some(ContextSubmenu {
                kind,
                parent_index,
                hover_index: None,
                keyboard_focus: None,
            });
            // The replaced pane is laid out again before the next paint.
            self.context_menu.submenu_layout = None;
        }
        self.context_menu.pending_hover = None;
        if focus {
            let first = self
                .context_submenu_entries()
                .iter()
                .position(|entry| !entry.disabled);
            self.set_context_menu_level_focus(ContextMenuLevel::Submenu, first);
        }
        self.needs_redraw = true;
        true
    }

    /// Closes the open submenu. `focus_parent` moves keyboard focus back to the
    /// row that opened it. Returns whether a submenu was open.
    pub(crate) fn close_context_submenu(&mut self, focus_parent: bool) -> bool {
        let ContextMenuState::Open {
            submenu,
            hover_index,
            keyboard_focus,
            ..
        } = &mut self.context_menu.state
        else {
            return false;
        };
        let Some(closed) = submenu.take() else {
            return false;
        };
        if focus_parent {
            *keyboard_focus = Some(closed.parent_index);
            *hover_index = None;
        }
        self.context_menu.submenu_layout = None;
        self.context_menu.pending_hover = None;
        self.needs_redraw = true;
        true
    }

    /// A click on a parent row: collapses its open submenu, otherwise opens
    /// it. A collapsed submenu stays shut until the pointer leaves the row.
    pub(super) fn toggle_context_submenu(&mut self, parent_index: usize) -> bool {
        let expanded = self
            .context_submenu()
            .is_some_and(|submenu| submenu.parent_index == parent_index);
        if expanded {
            self.context_menu.hover_open_suppressed = Some(parent_index);
            self.close_context_submenu(false)
        } else {
            self.context_menu.hover_open_suppressed = None;
            self.open_context_submenu(parent_index, false)
        }
    }

    /// Opens the submenu of the focused or hovered parent row, with keyboard
    /// focus inside it.
    pub(crate) fn open_focused_context_submenu(&mut self) -> bool {
        if self.active_context_menu_level() != ContextMenuLevel::Root {
            return false;
        }
        self.current_menu_focus_or_hover(ContextMenuLevel::Root)
            .is_some_and(|row| self.open_context_submenu(row, true))
    }
}
