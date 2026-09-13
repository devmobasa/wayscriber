mod boards;
mod canvas;
mod page;
mod pages;
mod shape;
mod zoom;

use super::super::base::InputState;
use super::types::{
    ContextMenuEntry, ContextMenuKind, ContextMenuLevel, ContextMenuState, MenuCommand,
};
use crate::domain::Action;
use crate::draw::ShapeId;

impl InputState {
    /// Append the chrome recovery entries ("Show Toolbar"/"Show Status Bar")
    /// shared by the canvas and shape menus: right-clicking must offer the
    /// mouse-only way back regardless of what happens to be under the
    /// pointer. Each entry appears only while its surface is hidden and not
    /// presenter-owned (presenter mode hides chrome by design and restores
    /// it on exit).
    pub(super) fn push_chrome_recovery_entries(&self, entries: &mut Vec<ContextMenuEntry>) {
        if !(self.toolbar_visible() || self.presenter_hides_toolbars()) {
            entries.push(ContextMenuEntry::new(
                "Show Toolbar",
                self.shortcut_for_action(Action::ToggleToolbar),
                false,
                Some(MenuCommand::ShowToolbar),
            ));
        }
        if !(self.ui_visibility.show_status_bar
            || self.presenter_mode_active() && self.presenter_mode_config().hide_status_bar)
        {
            entries.push(ContextMenuEntry::new(
                "Show Status Bar",
                self.shortcut_for_action(Action::ToggleStatusBar),
                false,
                Some(MenuCommand::ShowStatusBar),
            ));
        }
    }

    /// Returns the entries to render for the currently open context menu.
    pub fn context_menu_entries(&self) -> Vec<ContextMenuEntry> {
        match &self.context_menu.state {
            ContextMenuState::Hidden => Vec::new(),
            ContextMenuState::Open {
                kind,
                shape_ids,
                hovered_shape_id,
                ..
            } => self.menu_entries(*kind, shape_ids, *hovered_shape_id, ContextMenuLevel::Root),
        }
    }

    /// Returns the entries of the open submenu, if any.
    pub fn context_submenu_entries(&self) -> Vec<ContextMenuEntry> {
        self.context_submenu().map_or_else(Vec::new, |submenu| {
            self.menu_entries(submenu.kind, &[], None, ContextMenuLevel::Submenu)
        })
    }

    pub(super) fn context_menu_level_entries(
        &self,
        level: ContextMenuLevel,
    ) -> Vec<ContextMenuEntry> {
        match level {
            ContextMenuLevel::Root => self.context_menu_entries(),
            ContextMenuLevel::Submenu => self.context_submenu_entries(),
        }
    }

    /// Builds one menu's rows. A menu that stands on its own starts with a
    /// header naming its current state; as a submenu that state sits in the
    /// parent row instead, so the first row lines up with the parent.
    fn menu_entries(
        &self,
        kind: ContextMenuKind,
        shape_ids: &[ShapeId],
        hovered_shape_id: Option<ShapeId>,
        level: ContextMenuLevel,
    ) -> Vec<ContextMenuEntry> {
        let with_header = level == ContextMenuLevel::Root;
        match kind {
            ContextMenuKind::Canvas => self.canvas_menu_entries(),
            ContextMenuKind::Shape => self.shape_menu_entries(shape_ids, hovered_shape_id),
            ContextMenuKind::Zoom => self.zoom_menu_entries(with_header),
            ContextMenuKind::Pages => self.pages_menu_entries(with_header),
            ContextMenuKind::Boards => self.boards_menu_entries(with_header),
            ContextMenuKind::Page => self.page_context_menu_entries(),
            ContextMenuKind::PageMove => self.page_move_menu_entries(),
            ContextMenuKind::Board => self.board_context_menu_entries(),
        }
    }
}
