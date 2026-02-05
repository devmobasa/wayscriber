use crate::input::events::Key;
use crate::input::state::BoardPickerFocus;
use crate::input::state::InputState;

const PROPERTIES_PANEL_COARSE_STEP: i32 = 5;

impl InputState {
    pub(super) fn handle_color_picker_popup_key(&mut self, key: Key) -> bool {
        if !self.is_color_picker_popup_open() {
            return false;
        }

        if self.color_picker_popup_is_hex_editing() {
            match key {
                Key::Escape => {
                    // Unfocus hex input
                    self.color_picker_popup_set_hex_editing(false);
                    true
                }
                Key::Return => {
                    // Commit hex value
                    self.color_picker_popup_commit_hex();
                    true
                }
                Key::Backspace | Key::Delete => {
                    self.color_picker_popup_hex_backspace();
                    true
                }
                Key::Char(ch) => {
                    self.color_picker_popup_hex_append(ch);
                    true
                }
                _ => true,
            }
        } else {
            match key {
                Key::Escape => {
                    // Cancel and close popup
                    self.close_color_picker_popup(true);
                    true
                }
                Key::Return => {
                    // Apply and close popup
                    self.apply_color_picker_popup();
                    true
                }
                _ => true,
            }
        }
    }

    pub(super) fn handle_board_picker_key(&mut self, key: Key) -> bool {
        if !self.is_board_picker_open() {
            return false;
        }

        if self.board_picker_page_edit_state().is_some() {
            match key {
                Key::Escape => {
                    self.board_picker_cancel_page_edit();
                    true
                }
                Key::Return => {
                    self.board_picker_commit_page_edit();
                    true
                }
                Key::Backspace | Key::Delete => {
                    self.board_picker_page_edit_backspace();
                    true
                }
                Key::Space => {
                    self.board_picker_page_edit_append(' ');
                    true
                }
                Key::Char(ch) => {
                    self.board_picker_page_edit_append(ch);
                    true
                }
                _ => true,
            }
        } else if self.board_picker_edit_state().is_some() {
            match key {
                Key::Escape => {
                    self.board_picker_cancel_edit();
                    true
                }
                Key::Return => {
                    self.board_picker_commit_edit();
                    true
                }
                Key::Backspace | Key::Delete => {
                    self.board_picker_edit_backspace();
                    true
                }
                Key::Space => {
                    self.board_picker_edit_append(' ');
                    true
                }
                Key::Char(ch) => {
                    self.board_picker_edit_append(ch);
                    true
                }
                _ => true,
            }
        } else if self.board_picker_focus() == BoardPickerFocus::PagePanel {
            self.handle_board_picker_page_panel_key(key)
        } else {
            self.handle_board_picker_board_list_key(key)
        }
    }

    fn handle_board_picker_board_list_key(&mut self, key: Key) -> bool {
        if self.board_picker_is_quick() {
            match key {
                Key::Delete | Key::F2 => return true,
                Key::Char('n') | Key::Char('N') if self.modifiers.ctrl => return true,
                Key::Char('r') | Key::Char('R') if self.modifiers.ctrl => return true,
                Key::Char('c') | Key::Char('C') if self.modifiers.ctrl => return true,
                Key::Char('p') | Key::Char('P') if self.modifiers.ctrl => return true,
                _ => {}
            }
        }
        match key {
            Key::Escape => {
                if !self.board_picker_clear_search() {
                    self.close_board_picker();
                }
                true
            }
            Key::Backspace => {
                self.board_picker_backspace_search();
                true
            }
            Key::Up => {
                self.board_picker_clear_search();
                let next = self
                    .board_picker_selected_index()
                    .unwrap_or(0)
                    .saturating_sub(1);
                self.board_picker_set_selected(next);
                self.needs_redraw = true;
                true
            }
            Key::Down => {
                self.board_picker_clear_search();
                let next = self
                    .board_picker_selected_index()
                    .unwrap_or(0)
                    .saturating_add(1);
                self.board_picker_set_selected(next);
                self.needs_redraw = true;
                true
            }
            Key::Home => {
                self.board_picker_clear_search();
                self.board_picker_set_selected(0);
                self.needs_redraw = true;
                true
            }
            Key::End => {
                self.board_picker_clear_search();
                let last = self.board_picker_row_count().saturating_sub(1);
                self.board_picker_set_selected(last);
                self.needs_redraw = true;
                true
            }
            Key::Return | Key::Space => {
                if let Some(index) = self.board_picker_selected_index() {
                    self.board_picker_activate_row(index);
                }
                true
            }
            Key::Delete => {
                self.board_picker_delete_selected();
                true
            }
            Key::Tab | Key::Right => {
                let page_panel_enabled = self
                    .board_picker_layout
                    .is_some_and(|l| l.page_panel_enabled);
                if !self.board_picker_is_quick() && page_panel_enabled {
                    self.board_picker_set_focus(BoardPickerFocus::PagePanel);
                }
                true
            }
            Key::Char('n') | Key::Char('N') if self.modifiers.ctrl => {
                self.board_picker_create_new();
                true
            }
            Key::Char('r') | Key::Char('R') if self.modifiers.ctrl => {
                self.board_picker_rename_selected();
                true
            }
            Key::Char('c') | Key::Char('C') if self.modifiers.ctrl => {
                self.board_picker_edit_color_selected();
                true
            }
            Key::Char('p') | Key::Char('P') if self.modifiers.ctrl => {
                self.board_picker_toggle_pin_selected();
                true
            }
            Key::Char(ch) => {
                if !ch.is_control() {
                    self.board_picker_append_search(ch);
                }
                true
            }
            Key::F2 => {
                self.board_picker_rename_selected();
                true
            }
            _ => true,
        }
    }

    fn handle_board_picker_page_panel_key(&mut self, key: Key) -> bool {
        let layout = self.board_picker_layout;
        let page_cols = layout.map(|l| l.page_cols.max(1)).unwrap_or(1);
        let visible = layout.map(|l| l.page_visible_count).unwrap_or(0);
        let current = self.board_picker_page_focus_index().unwrap_or(0);

        match key {
            Key::Escape => {
                if !self.board_picker_clear_search() {
                    self.board_picker_set_focus(BoardPickerFocus::BoardList);
                }
                true
            }
            Key::Tab => {
                self.board_picker_set_focus(BoardPickerFocus::BoardList);
                true
            }
            Key::Left => {
                let col = current % page_cols;
                if col == 0 {
                    self.board_picker_set_focus(BoardPickerFocus::BoardList);
                } else if visible > 0 {
                    self.board_picker_set_page_focus_index(current.saturating_sub(1));
                }
                true
            }
            Key::Right => {
                if visible > 0 {
                    let next = current.saturating_add(1).min(visible.saturating_sub(1));
                    self.board_picker_set_page_focus_index(next);
                }
                true
            }
            Key::Up => {
                if visible > 0 {
                    let next = current.saturating_sub(page_cols);
                    self.board_picker_set_page_focus_index(next);
                }
                true
            }
            Key::Down => {
                if visible > 0 {
                    let next = current
                        .saturating_add(page_cols)
                        .min(visible.saturating_sub(1));
                    self.board_picker_set_page_focus_index(next);
                }
                true
            }
            Key::Return | Key::Space => {
                if visible > 0 {
                    self.board_picker_activate_page(current);
                }
                true
            }
            Key::Delete => {
                if visible > 0 {
                    self.board_picker_delete_page(current);
                    // Clamp focus using post-delete page count from actual board state,
                    // since the layout's page_visible_count is stale after deletion.
                    let remaining = self
                        .board_picker_page_panel_board_index()
                        .and_then(|bi| self.boards.board_states().get(bi))
                        .map(|b| b.pages.page_count())
                        .unwrap_or(0);
                    if remaining == 0 {
                        self.board_picker_set_focus(BoardPickerFocus::BoardList);
                    } else {
                        let max_visible = layout
                            .map(|l| l.page_cols * l.page_max_rows)
                            .unwrap_or(remaining);
                        let new_visible = remaining.min(max_visible);
                        let clamped = current.min(new_visible.saturating_sub(1));
                        self.board_picker_set_page_focus_index(clamped);
                    }
                }
                true
            }
            Key::F2 => {
                if visible > 0
                    && let Some(board_index) = self.board_picker_page_panel_board_index()
                {
                    self.board_picker_start_page_rename(board_index, current);
                }
                true
            }
            Key::Backspace => {
                self.board_picker_backspace_search();
                true
            }
            Key::Char(ch) => {
                if !ch.is_control() {
                    // Switch focus back to board list and append to search
                    self.board_picker_append_search(ch);
                }
                true
            }
            _ => true,
        }
    }

    pub(super) fn handle_properties_panel_key(&mut self, key: Key) -> bool {
        let adjust_step = if self.modifiers.shift {
            PROPERTIES_PANEL_COARSE_STEP
        } else {
            1
        };
        match key {
            Key::Escape => {
                self.close_properties_panel();
                true
            }
            Key::Up => self.focus_previous_properties_entry(),
            Key::Down => self.focus_next_properties_entry(),
            Key::Home => self.focus_first_properties_entry(),
            Key::End => self.focus_last_properties_entry(),
            Key::Return | Key::Space => self.activate_properties_panel_entry(),
            Key::Left => self.adjust_properties_panel_entry(-adjust_step),
            Key::Right => self.adjust_properties_panel_entry(adjust_step),
            Key::Char('+') | Key::Char('=') => self.adjust_properties_panel_entry(adjust_step),
            Key::Char('-') | Key::Char('_') => self.adjust_properties_panel_entry(-adjust_step),
            _ => false,
        }
    }

    pub(super) fn handle_context_menu_key(&mut self, key: Key) -> bool {
        match key {
            Key::Escape => {
                self.close_context_menu();
                true
            }
            Key::Up => self.focus_previous_context_menu_entry(),
            Key::Down => self.focus_next_context_menu_entry(),
            Key::Home => self.focus_first_context_menu_entry(),
            Key::End => self.focus_last_context_menu_entry(),
            Key::Return | Key::Space => self.activate_context_menu_selection(),
            _ => false,
        }
    }
}
