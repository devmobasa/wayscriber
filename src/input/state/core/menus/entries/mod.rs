mod boards;
mod canvas;
mod page;
mod pages;
mod shape;
mod zoom;

use super::super::base::InputState;
use super::types::{ContextMenuEntry, ContextMenuKind, ContextMenuState, MenuCommand};
use crate::domain::Action;

impl InputState {
    /// Append the chrome recovery entries ("Show Toolbar"/"Show Status Bar")
    /// shared by the canvas and shape menus: right-clicking must offer the
    /// mouse-only way back regardless of what happens to be under the
    /// pointer. Each entry appears only while its surface is hidden and not
    /// presenter-owned (presenter mode hides chrome by design and restores
    /// it on exit).
    pub(super) fn push_chrome_recovery_entries(&self, entries: &mut Vec<ContextMenuEntry>) {
        if !self.toolbar_visible()
            && !(self.presenter_mode && self.presenter_mode_config.hide_toolbars)
        {
            entries.push(ContextMenuEntry::new(
                "Show Toolbar",
                self.shortcut_for_action(Action::ToggleToolbar),
                false,
                false,
                Some(MenuCommand::ShowToolbar),
            ));
        }
        if !self.show_status_bar
            && !(self.presenter_mode && self.presenter_mode_config.hide_status_bar)
        {
            entries.push(ContextMenuEntry::new(
                "Show Status Bar",
                self.shortcut_for_action(Action::ToggleStatusBar),
                false,
                false,
                Some(MenuCommand::ShowStatusBar),
            ));
        }
    }

    /// Returns the entries to render for the currently open context menu.
    pub fn context_menu_entries(&self) -> Vec<ContextMenuEntry> {
        match &self.context_menu_state {
            ContextMenuState::Hidden => Vec::new(),
            ContextMenuState::Open {
                kind,
                shape_ids,
                hovered_shape_id,
                ..
            } => match kind {
                ContextMenuKind::Canvas => self.canvas_menu_entries(),
                ContextMenuKind::Shape => self.shape_menu_entries(shape_ids, *hovered_shape_id),
                ContextMenuKind::Zoom => self.zoom_menu_entries(),
                ContextMenuKind::Pages => self.pages_menu_entries(),
                ContextMenuKind::Boards => self.boards_menu_entries(),
                ContextMenuKind::Page => self.page_context_menu_entries(),
                ContextMenuKind::PageMove => self.page_move_menu_entries(),
            },
        }
    }
}
