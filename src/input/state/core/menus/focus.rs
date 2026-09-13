use super::super::base::InputState;
use super::types::ContextMenuLevel;

impl InputState {
    /// The menu keyboard navigation acts on: an open submenu once the pointer
    /// or keyboard focus is inside it, otherwise the parent menu.
    pub(crate) fn active_context_menu_level(&self) -> ContextMenuLevel {
        match self.context_submenu() {
            Some(submenu) if submenu.hover_index.is_some() || submenu.keyboard_focus.is_some() => {
                ContextMenuLevel::Submenu
            }
            _ => ContextMenuLevel::Root,
        }
    }

    /// Whether the open submenu holds the selection, so keys act on it.
    pub fn context_submenu_is_active(&self) -> bool {
        self.active_context_menu_level() == ContextMenuLevel::Submenu
    }

    pub(super) fn current_menu_focus_or_hover(&self, level: ContextMenuLevel) -> Option<usize> {
        let (hover, focus) = self.context_menu_level_selection(level)?;
        hover.or(focus)
    }

    fn select_edge_context_menu_entry(&mut self, start_front: bool) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }
        let level = self.active_context_menu_level();
        let entries = self.context_menu_level_entries(level);
        let index = if start_front {
            entries.iter().position(|entry| !entry.disabled)
        } else {
            entries.iter().rposition(|entry| !entry.disabled)
        };
        let Some(index) = index else {
            return false;
        };
        self.set_context_menu_level_focus(level, Some(index));
        true
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
        let level = self.active_context_menu_level();
        let entries = self.context_menu_level_entries(level);
        if entries.is_empty() {
            return false;
        }

        let len = entries.len();
        let mut index = self
            .current_menu_focus_or_hover(level)
            .unwrap_or_else(|| if forward { len - 1 } else { 0 });

        for _ in 0..len {
            index = if forward {
                (index + 1) % len
            } else {
                (index + len - 1) % len
            };
            if !entries[index].disabled {
                // Moving off a parent row collapses its submenu.
                if level == ContextMenuLevel::Root
                    && self
                        .context_submenu()
                        .is_some_and(|submenu| submenu.parent_index != index)
                {
                    self.close_context_submenu(false);
                }
                self.set_context_menu_level_focus(level, Some(index));
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

    pub(crate) fn activate_context_menu_selection_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
    ) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }
        let level = self.active_context_menu_level();
        let Some(index) = self.current_menu_focus_or_hover(level) else {
            return false;
        };
        self.activate_context_menu_row_with_resources(resources, level, index, true)
    }

    /// Clicks the menu row under the pointer. Returns false when no enabled row
    /// is there.
    pub(crate) fn activate_context_menu_row_at_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        x: i32,
        y: i32,
    ) -> bool {
        let Some(level) = self.context_menu_level_at(x, y) else {
            return false;
        };
        let Some(index) = self.context_menu_row_at(level, x, y) else {
            return false;
        };
        self.activate_context_menu_row_with_resources(resources, level, index, false)
    }

    /// Runs one enabled row. A parent row with a submenu opens it instead: from
    /// the keyboard with focus inside it, from a click as a toggle.
    fn activate_context_menu_row_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        level: ContextMenuLevel,
        index: usize,
        focus_submenu: bool,
    ) -> bool {
        let entries = self.context_menu_level_entries(level);
        let Some(entry) = entries.get(index).filter(|entry| !entry.disabled) else {
            return false;
        };
        if level == ContextMenuLevel::Root && entry.submenu.is_some() {
            return if focus_submenu {
                self.open_context_submenu(index, true)
            } else {
                self.toggle_context_submenu(index)
            };
        }
        match entry.command.clone() {
            Some(command) => self.execute_menu_command_with_resources(resources, command),
            None => self.close_context_menu(),
        }
        true
    }
}
