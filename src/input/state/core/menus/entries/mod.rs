mod boards;
mod canvas;
mod pages;
mod shape;

use super::super::base::InputState;
use super::types::{ContextMenuEntry, ContextMenuKind, ContextMenuState};

impl InputState {
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
                ContextMenuKind::Pages => self.pages_menu_entries(),
                ContextMenuKind::Boards => self.boards_menu_entries(),
            },
        }
    }
}
