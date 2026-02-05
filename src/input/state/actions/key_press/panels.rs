use crate::input::events::Key;
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
        } else {
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
