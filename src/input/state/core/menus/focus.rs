use super::super::base::InputState;
use super::types::{ContextMenuEntry, ContextMenuState};

impl InputState {
    fn current_menu_focus_or_hover(&self) -> Option<usize> {
        if let ContextMenuState::Open {
            hover_index,
            keyboard_focus,
            ..
        } = &self.context_menu_state
        {
            hover_index.or(*keyboard_focus)
        } else {
            None
        }
    }

    fn select_edge_context_menu_entry(&mut self, start_front: bool) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }
        let entries = self.context_menu_entries();
        let iter: Box<dyn Iterator<Item = (usize, &ContextMenuEntry)>> = if start_front {
            Box::new(entries.iter().enumerate())
        } else {
            Box::new(entries.iter().enumerate().rev())
        };
        for (index, entry) in iter {
            if !entry.disabled {
                self.set_context_menu_focus(Some(index));
                return true;
            }
        }
        false
    }

    pub(crate) fn focus_next_context_menu_entry(&mut self) -> bool {
        self.advance_context_menu_focus(true)
    }

    pub(crate) fn focus_previous_context_menu_entry(&mut self) -> bool {
        self.advance_context_menu_focus(false)
    }

    fn advance_context_menu_focus(&mut self, forward: bool) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }
        let entries = self.context_menu_entries();
        if entries.is_empty() {
            return false;
        }

        let len = entries.len();
        let mut index = self
            .current_menu_focus_or_hover()
            .unwrap_or_else(|| if forward { len - 1 } else { 0 });

        for _ in 0..len {
            index = if forward {
                (index + 1) % len
            } else {
                (index + len - 1) % len
            };
            if !entries[index].disabled {
                self.set_context_menu_focus(Some(index));
                return true;
            }
        }
        false
    }

    pub(crate) fn focus_first_context_menu_entry(&mut self) -> bool {
        self.select_edge_context_menu_entry(true)
    }

    pub(crate) fn focus_last_context_menu_entry(&mut self) -> bool {
        self.select_edge_context_menu_entry(false)
    }

    pub(crate) fn activate_context_menu_selection(&mut self) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }
        let entries = self.context_menu_entries();
        if entries.is_empty() {
            return false;
        }
        let index = match self.current_menu_focus_or_hover() {
            Some(idx) => idx,
            None => return false,
        };
        if let Some(entry) = entries.get(index) {
            if entry.disabled {
                return false;
            }
            if let Some(command) = entry.command {
                self.execute_menu_command(command);
            } else {
                self.close_context_menu();
            }
            true
        } else {
            false
        }
    }
}
