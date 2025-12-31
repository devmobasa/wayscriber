use super::super::base::InputState;
use super::types::{ContextMenuState, MenuCommand};

impl InputState {
    pub(super) fn update_context_menu_hover_from_pointer_internal(
        &mut self,
        x: i32,
        y: i32,
        trigger_redraw: bool,
    ) {
        if !self.is_context_menu_open() {
            return;
        }
        let new_hover = self.context_menu_index_at(x, y);
        if let ContextMenuState::Open {
            ref mut hover_index,
            ref mut keyboard_focus,
            ..
        } = self.context_menu_state
            && *hover_index != new_hover
        {
            *hover_index = new_hover;
            if new_hover.is_some() {
                *keyboard_focus = None;
            }
            if trigger_redraw {
                self.needs_redraw = true;
            }
        }
    }

    /// Updates hover state based on the provided pointer position.
    pub fn update_context_menu_hover_from_pointer(&mut self, x: i32, y: i32) {
        self.update_context_menu_hover_from_pointer_internal(x, y, true);
    }

    /// Updates cached hover information without forcing a redraw.
    /// Updates the keyboard focus entry for the context menu.
    pub fn set_context_menu_focus(&mut self, focus: Option<usize>) {
        if let ContextMenuState::Open {
            ref mut keyboard_focus,
            ref mut hover_index,
            ..
        } = self.context_menu_state
        {
            let changed = *keyboard_focus != focus;
            *keyboard_focus = focus;
            if focus.is_some() {
                *hover_index = None;
            }
            if changed {
                self.needs_redraw = true;
            }
        }
    }

    pub(crate) fn focus_context_menu_command(&mut self, command: MenuCommand) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }
        let entries = self.context_menu_entries();
        for (index, entry) in entries.iter().enumerate() {
            if entry.disabled {
                continue;
            }
            if entry.command == Some(command) {
                self.set_context_menu_focus(Some(index));
                return true;
            }
        }
        false
    }
}
