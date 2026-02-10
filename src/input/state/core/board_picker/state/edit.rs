use crate::draw::Color;
use crate::input::BoardBackground;

use super::super::super::base::{InputState, UiToastKind};
use super::super::{
    BOARD_PICKER_RECENT_LABEL_MAX_CHARS, BOARD_PICKER_RECENT_MAX_NAMES,
    BOARD_PICKER_SEARCH_MAX_LEN, BoardPickerEdit, BoardPickerEditMode, BoardPickerFocus,
    BoardPickerMode, BoardPickerPageEdit, BoardPickerState, MAX_BOARD_NAME_LEN, MAX_PAGE_NAME_LEN,
    color_to_hex, parse_hex_color, truncate_search_label,
};

impl InputState {
    pub(crate) fn board_picker_clear_edit(&mut self) {
        if let BoardPickerState::Open { edit, .. } = &mut self.board_picker_state {
            *edit = None;
        }
    }

    pub(crate) fn board_picker_start_edit(&mut self, mode: BoardPickerEditMode, buffer: String) {
        if let BoardPickerState::Open { edit, .. } = &mut self.board_picker_state {
            *edit = Some(BoardPickerEdit { mode, buffer });
        }
        self.board_picker_clear_search();
    }

    pub(crate) fn board_picker_edit_state(&self) -> Option<(BoardPickerEditMode, usize, &str)> {
        let BoardPickerState::Open { selected, edit, .. } = &self.board_picker_state else {
            return None;
        };
        let edit = edit.as_ref()?;
        Some((edit.mode, *selected, edit.buffer.as_str()))
    }

    pub(crate) fn board_picker_edit_buffer_mut(&mut self) -> Option<&mut BoardPickerEdit> {
        let BoardPickerState::Open { edit, .. } = &mut self.board_picker_state else {
            return None;
        };
        edit.as_mut()
    }

    pub(crate) fn board_picker_page_edit_state(&self) -> Option<(usize, usize, &str)> {
        self.board_picker_page_edit
            .as_ref()
            .map(|edit| (edit.board_index, edit.page_index, edit.buffer.as_str()))
    }

    pub(crate) fn board_picker_start_page_rename(&mut self, board_index: usize, page_index: usize) {
        let Some(board) = self.boards.board_states().get(board_index) else {
            return;
        };
        if page_index >= board.pages.page_count() {
            return;
        }
        let buffer = board
            .pages
            .page_name(page_index)
            .unwrap_or_default()
            .to_string();
        self.board_picker_clear_edit();
        self.board_picker_page_edit = Some(BoardPickerPageEdit {
            board_index,
            page_index,
            buffer,
        });
        self.board_picker_clear_search();
        self.needs_redraw = true;
    }

    pub(crate) fn board_picker_commit_page_edit(&mut self) -> bool {
        let Some(edit) = self.board_picker_page_edit.take() else {
            return false;
        };
        let name = edit.buffer.trim().to_string();
        let name = if name.is_empty() { None } else { Some(name) };
        let _ = self.rename_page_in_board(edit.board_index, edit.page_index, name);
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_picker_cancel_page_edit(&mut self) {
        if self.board_picker_page_edit.is_some() {
            self.board_picker_page_edit = None;
            self.needs_redraw = true;
        }
    }

    pub(crate) fn board_picker_page_edit_backspace(&mut self) {
        if let Some(edit) = &mut self.board_picker_page_edit {
            edit.buffer.pop();
            self.needs_redraw = true;
        }
    }

    pub(crate) fn board_picker_page_edit_append(&mut self, ch: char) {
        let Some(edit) = &mut self.board_picker_page_edit else {
            return;
        };
        if edit.buffer.len() >= MAX_PAGE_NAME_LEN {
            return;
        }
        if !ch.is_control() {
            edit.buffer.push(ch);
            self.needs_redraw = true;
        }
    }

    pub(crate) fn board_picker_footer_text(&self) -> String {
        let search = self.board_picker_search.trim();
        if !search.is_empty() {
            return format!(
                "Search: {}  (Esc: clear)",
                truncate_search_label(search, BOARD_PICKER_SEARCH_MAX_LEN)
            );
        }
        if self.board_picker_is_quick() {
            "Enter: switch  Type: jump  Esc: close".to_string()
        } else if self.board_picker_focus() == BoardPickerFocus::PagePanel {
            "Enter: open  F2: rename  Del: delete  Tab: back  Esc: close".to_string()
        } else {
            let page_panel_enabled = self
                .board_picker_layout
                .is_some_and(|layout| layout.page_panel_enabled);
            if page_panel_enabled {
                "Enter: open  F2: rename  Del: delete  Tab: pages  Esc: close".to_string()
            } else {
                "Enter: open  F2: rename  Ctrl+N: new  Del: delete  Esc: close".to_string()
            }
        }
    }

    pub(crate) fn board_picker_title(&self, board_count: usize, max_count: usize) -> String {
        if self.board_picker_is_quick() {
            "Switch board".to_string()
        } else {
            format!("Boards ({}/{})", board_count, max_count)
        }
    }

    pub(crate) fn board_picker_recent_label(&self) -> Option<String> {
        let active_id = self.boards.active_board_id();
        let mut names = Vec::new();
        for id in &self.board_recent {
            if id == active_id {
                continue;
            }
            if let Some(board) = self.boards.board_states().iter().find(|b| b.spec.id == *id) {
                names.push(board.spec.name.clone());
            }
            if names.len() >= BOARD_PICKER_RECENT_MAX_NAMES {
                break;
            }
        }
        if names.is_empty() {
            None
        } else {
            let label = format!("Recent: {}", names.join(", "));
            Some(truncate_search_label(
                &label,
                BOARD_PICKER_RECENT_LABEL_MAX_CHARS,
            ))
        }
    }

    pub(crate) fn board_picker_select_recent(&mut self) {
        let active_id = self.boards.active_board_id();
        let mut recent_index = None;
        for id in &self.board_recent {
            if id == active_id {
                continue;
            }
            if let Some(idx) = self
                .boards
                .board_states()
                .iter()
                .position(|board| board.spec.id == *id)
            {
                recent_index = self.board_picker_row_for_board(idx);
                break;
            }
        }
        if let Some(idx) = recent_index {
            self.board_picker_set_selected(idx);
        }
    }

    pub(super) fn board_picker_promote_to_full(&mut self) {
        let selected_board = match &self.board_picker_state {
            BoardPickerState::Open { selected, mode, .. } => {
                self.board_picker_board_index_for_row_in_mode(*selected, *mode)
            }
            BoardPickerState::Hidden => None,
        };
        let selected_row_full = selected_board.and_then(|board_index| {
            self.board_picker_row_for_board_in_mode(board_index, BoardPickerMode::Full)
        });

        let BoardPickerState::Open {
            selected,
            hover_index,
            edit,
            mode,
            focus,
            page_focus_index,
        } = &mut self.board_picker_state
        else {
            return;
        };
        if *mode == BoardPickerMode::Full {
            return;
        }
        *mode = BoardPickerMode::Full;
        *hover_index = None;
        *edit = None;
        *focus = BoardPickerFocus::BoardList;
        *page_focus_index = None;
        if let Some(row) = selected_row_full {
            *selected = row;
        }
        self.board_picker_layout = None;
        self.needs_redraw = true;
    }

    pub(crate) fn board_picker_rename_selected(&mut self) {
        let Some(index) = self.board_picker_selected_index() else {
            return;
        };
        if self.board_picker_is_quick() {
            self.board_picker_promote_to_full();
        }
        if self.board_picker_is_new_row(index) {
            self.board_picker_create_new();
            return;
        }
        let Some(board_index) = self.board_picker_board_index_for_row(index) else {
            return;
        };
        if let Some(board) = self.boards.board_states().get(board_index) {
            self.board_picker_start_edit(BoardPickerEditMode::Name, board.spec.name.clone());
        }
    }

    pub(crate) fn board_picker_edit_color_selected(&mut self) {
        let Some(index) = self.board_picker_selected_index() else {
            return;
        };
        if self.board_picker_is_quick() {
            self.board_picker_promote_to_full();
        }
        if self.board_picker_is_new_row(index) {
            self.board_picker_create_new();
            return;
        }
        let Some(board_index) = self.board_picker_board_index_for_row(index) else {
            return;
        };
        let Some(board) = self.boards.board_states().get(board_index) else {
            return;
        };
        if board.spec.background.is_transparent() {
            self.set_ui_toast(UiToastKind::Info, "Overlay board has no background color.");
            return;
        }
        let buffer = match &board.spec.background {
            BoardBackground::Solid(color) => color_to_hex(*color),
            BoardBackground::Transparent => String::new(),
        };
        self.board_picker_start_edit(BoardPickerEditMode::Color, buffer);
        self.needs_redraw = true;
    }

    pub(crate) fn board_picker_commit_edit(&mut self) -> bool {
        let Some((mode, index, buffer)) = self.board_picker_edit_state() else {
            return false;
        };
        if self.board_picker_is_new_row(index) {
            self.board_picker_clear_edit();
            return false;
        }
        let Some(board_index) = self.board_picker_board_index_for_row(index) else {
            self.board_picker_clear_edit();
            return false;
        };

        let buffer = buffer.to_string();
        let trimmed = buffer.trim();
        match mode {
            BoardPickerEditMode::Name => {
                if !self.set_board_name(board_index, trimmed.to_string()) {
                    return false;
                }
            }
            BoardPickerEditMode::Color => {
                let Some(color) = parse_hex_color(trimmed) else {
                    self.set_ui_toast(
                        UiToastKind::Warning,
                        "Invalid color. Use #RRGGBB or RRGGBB.",
                    );
                    return false;
                };
                if !self.set_board_background_color(board_index, color) {
                    return false;
                }
            }
        }

        self.board_picker_clear_edit();
        true
    }

    pub(crate) fn board_picker_cancel_edit(&mut self) {
        self.board_picker_clear_edit();
        self.needs_redraw = true;
    }

    pub(crate) fn board_picker_edit_backspace(&mut self) {
        if let Some(edit) = self.board_picker_edit_buffer_mut() {
            edit.buffer.pop();
            self.needs_redraw = true;
        }
    }

    pub(crate) fn board_picker_edit_append(&mut self, ch: char) {
        let Some(edit) = self.board_picker_edit_buffer_mut() else {
            return;
        };
        match edit.mode {
            BoardPickerEditMode::Name => {
                if edit.buffer.len() >= MAX_BOARD_NAME_LEN {
                    return;
                }
                if !ch.is_control() {
                    edit.buffer.push(ch);
                    self.needs_redraw = true;
                }
            }
            BoardPickerEditMode::Color => {
                let max_len = if edit.buffer.starts_with('#') { 7 } else { 6 };
                if edit.buffer.len() >= max_len {
                    return;
                }
                if ch == '#' && edit.buffer.is_empty() {
                    edit.buffer.push(ch);
                    self.needs_redraw = true;
                    return;
                }
                if ch.is_ascii_hexdigit() {
                    edit.buffer.push(ch.to_ascii_uppercase());
                    self.needs_redraw = true;
                }
            }
        }
    }

    pub(crate) fn board_picker_apply_palette_color(&mut self, color: Color) -> bool {
        let Some(index) = self.board_picker_selected_index() else {
            return false;
        };
        if self.board_picker_is_new_row(index) {
            return false;
        }
        let Some(board_index) = self.board_picker_board_index_for_row(index) else {
            return false;
        };
        if !self.set_board_background_color(board_index, color) {
            return false;
        }
        if let Some(edit) = self.board_picker_edit_buffer_mut()
            && edit.mode == BoardPickerEditMode::Color
        {
            edit.buffer = color_to_hex(color);
        }
        self.needs_redraw = true;
        true
    }
}
