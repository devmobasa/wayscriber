use crate::draw::Color;
use crate::input::BoardBackground;

use super::super::base::{InputState, UiToastKind};
use super::{
    BOARD_PICKER_RECENT_LABEL_MAX_CHARS, BOARD_PICKER_RECENT_MAX_NAMES,
    BOARD_PICKER_SEARCH_MAX_LEN, BoardPickerDrag, BoardPickerEdit, BoardPickerEditMode,
    BoardPickerMode, BoardPickerPageDrag, BoardPickerState, MAX_BOARD_NAME_LEN, MAX_PAGE_NAME_LEN,
    color_to_hex, parse_hex_color, truncate_search_label,
};

impl InputState {
    pub(crate) fn is_board_picker_open(&self) -> bool {
        matches!(self.board_picker_state, BoardPickerState::Open { .. })
    }

    pub(crate) fn board_picker_mode(&self) -> BoardPickerMode {
        match &self.board_picker_state {
            BoardPickerState::Open { mode, .. } => *mode,
            BoardPickerState::Hidden => BoardPickerMode::Full,
        }
    }

    pub(crate) fn board_picker_is_quick(&self) -> bool {
        self.board_picker_mode() == BoardPickerMode::Quick
    }

    pub(crate) fn open_board_picker(&mut self) {
        if self.show_help {
            self.toggle_help_overlay();
        }
        self.cancel_active_interaction();
        self.close_context_menu();
        self.close_properties_panel();
        self.board_picker_clear_search();
        self.board_picker_drag = None;
        self.board_picker_page_drag = None;
        self.board_picker_page_edit = None;
        let active_index = self.boards.active_index();
        self.board_picker_state = BoardPickerState::Open {
            selected: active_index,
            hover_index: None,
            edit: None,
            mode: BoardPickerMode::Full,
        };
        let selected_row = self.board_picker_row_for_board(active_index);
        if let (Some(selected), BoardPickerState::Open { selected: row, .. }) =
            (selected_row, &mut self.board_picker_state)
        {
            *row = selected;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn open_board_picker_quick(&mut self) {
        if self.show_help {
            self.toggle_help_overlay();
        }
        self.cancel_active_interaction();
        self.close_context_menu();
        self.close_properties_panel();
        self.board_picker_clear_search();
        self.board_picker_drag = None;
        self.board_picker_page_drag = None;
        self.board_picker_page_edit = None;
        let active_index = self.boards.active_index();
        self.board_picker_state = BoardPickerState::Open {
            selected: active_index,
            hover_index: None,
            edit: None,
            mode: BoardPickerMode::Quick,
        };
        let selected_row = self.board_picker_row_for_board(active_index);
        if let (Some(selected), BoardPickerState::Open { selected: row, .. }) =
            (selected_row, &mut self.board_picker_state)
        {
            *row = selected;
        }
        self.board_picker_select_recent();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn close_board_picker(&mut self) {
        if let Some(layout) = self.board_picker_layout {
            self.mark_board_picker_region(layout);
        }
        // Board picker dims the entire screen; ensure full redraw when closing.
        self.dirty_tracker.mark_full();
        self.board_picker_state = BoardPickerState::Hidden;
        self.board_picker_drag = None;
        self.board_picker_page_drag = None;
        self.board_picker_page_edit = None;
        self.board_picker_layout = None;
        self.last_board_picker_click = None;
        self.board_picker_clear_search();
        self.needs_redraw = true;
    }

    pub(crate) fn toggle_board_picker(&mut self) {
        if self.is_board_picker_open() {
            self.close_board_picker();
        } else {
            self.open_board_picker();
        }
    }

    pub(crate) fn toggle_board_picker_quick(&mut self) {
        if self.is_board_picker_open() {
            self.close_board_picker();
        } else {
            self.open_board_picker_quick();
        }
    }

    pub(crate) fn board_picker_active_index(&self) -> Option<usize> {
        match &self.board_picker_state {
            BoardPickerState::Open {
                hover_index,
                selected,
                ..
            } => hover_index.or(Some(*selected)),
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn board_picker_selected_index(&self) -> Option<usize> {
        match &self.board_picker_state {
            BoardPickerState::Open { selected, .. } => Some(*selected),
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn board_picker_page_panel_board_index(&self) -> Option<usize> {
        let selected = self.board_picker_selected_index()?;
        if let Some(board_index) = self.board_picker_board_index_for_row(selected) {
            Some(board_index)
        } else {
            Some(self.boards.active_index())
        }
    }

    pub(crate) fn board_picker_set_selected(&mut self, index: usize) {
        let row_count = self.board_picker_row_count().max(1);
        let next = index.min(row_count.saturating_sub(1));
        if let BoardPickerState::Open { selected, .. } = &mut self.board_picker_state {
            *selected = next;
            self.needs_redraw = true;
            self.dirty_tracker.mark_full();
        }
    }

    pub(crate) fn board_picker_is_dragging(&self) -> bool {
        self.board_picker_drag.is_some()
    }

    pub(crate) fn board_picker_is_page_dragging(&self) -> bool {
        self.board_picker_page_drag.is_some()
    }

    pub(crate) fn board_picker_start_drag(&mut self, row: usize) -> bool {
        if self.board_picker_is_quick() || self.board_picker_is_new_row(row) {
            return false;
        }
        let Some(board_index) = self.board_picker_board_index_for_row(row) else {
            return false;
        };
        self.board_picker_drag = Some(BoardPickerDrag {
            source_row: row,
            source_board: board_index,
            current_row: row,
        });
        self.board_picker_set_selected(row);
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_picker_start_page_drag(&mut self, page_index: usize) -> bool {
        if self.board_picker_is_quick() {
            return false;
        }
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return false;
        };
        let page_count = self
            .boards
            .board_states()
            .get(board_index)
            .map(|board| board.pages.page_count())
            .unwrap_or(0);
        if page_index >= page_count {
            return false;
        }
        self.board_picker_page_drag = Some(BoardPickerPageDrag {
            source_index: page_index,
            current_index: page_index,
            board_index,
            target_board: Some(board_index),
        });
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_picker_update_drag_from_pointer(&mut self, _x: i32, y: i32) {
        let Some(layout) = self.board_picker_layout else {
            return;
        };
        let Some(source_board) = self
            .board_picker_drag
            .as_ref()
            .map(|drag| drag.source_board)
        else {
            return;
        };
        let board_count = self.boards.board_count();
        if board_count == 0 {
            return;
        }
        let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
        let row = ((y as f64 - rows_top) / layout.row_height).floor() as isize;
        let max_row = board_count.saturating_sub(1) as isize;
        let clamped = row.clamp(0, max_row) as usize;
        let target_row = self.board_picker_clamp_drag_row(clamped, source_board);
        if let Some(drag) = &mut self.board_picker_drag
            && drag.current_row != target_row
        {
            drag.current_row = target_row;
            self.board_picker_set_selected(target_row);
            self.needs_redraw = true;
        }
    }

    pub(crate) fn board_picker_update_page_drag_from_pointer(&mut self, x: i32, y: i32) {
        let Some(layout) = self.board_picker_layout else {
            return;
        };
        let Some(drag) = self.board_picker_page_drag else {
            return;
        };
        if !layout.page_panel_enabled {
            return;
        }
        let mut next_target_board = Some(drag.board_index);
        let mut next_current_index = drag.current_index;
        let mut next_hover_row = None;

        if let Some(row) = self.board_picker_index_at(x, y)
            && !self.board_picker_is_new_row(row)
            && let Some(board_index) = self.board_picker_board_index_for_row(row)
        {
            next_target_board = Some(board_index);
            next_hover_row = Some(row);
        } else if let Some(index) = self.board_picker_page_index_at(x, y) {
            next_current_index = index.min(layout.page_count.saturating_sub(1));
        }

        let mut updated = false;
        if let Some(drag) = &mut self.board_picker_page_drag {
            if drag.target_board != next_target_board {
                drag.target_board = next_target_board;
                updated = true;
            }
            if drag.current_index != next_current_index {
                drag.current_index = next_current_index;
                updated = true;
            }
        }

        if let BoardPickerState::Open { hover_index, .. } = &mut self.board_picker_state
            && *hover_index != next_hover_row
        {
            *hover_index = next_hover_row;
            updated = true;
        }

        if updated {
            self.needs_redraw = true;
        }
    }

    pub(crate) fn board_picker_finish_drag(&mut self) -> bool {
        let Some(drag) = self.board_picker_drag.take() else {
            return false;
        };
        let target_row = self.board_picker_clamp_drag_row(drag.current_row, drag.source_board);
        if target_row == drag.source_row {
            self.needs_redraw = true;
            return true;
        }
        let Some(target_board) = self.board_picker_board_index_for_row(target_row) else {
            return true;
        };
        let source_id = self
            .boards
            .board_states()
            .get(drag.source_board)
            .map(|board| board.spec.id.clone());
        if !self.reorder_board(drag.source_board, target_board) {
            return true;
        }
        let Some(source_id) = source_id else {
            return true;
        };
        let Some(new_index) = self
            .boards
            .board_states()
            .iter()
            .position(|board| board.spec.id == source_id)
        else {
            return true;
        };
        if let Some(row) = self.board_picker_row_for_board(new_index) {
            self.board_picker_set_selected(row);
        }
        true
    }

    pub(crate) fn board_picker_finish_page_drag(&mut self) -> bool {
        let Some(drag) = self.board_picker_page_drag.take() else {
            return false;
        };
        let target_board = drag.target_board.unwrap_or(drag.board_index);
        if target_board != drag.board_index {
            let copy = self.modifiers.alt;
            if self.move_page_between_boards(
                drag.board_index,
                drag.source_index,
                target_board,
                copy,
            ) {
                self.switch_board_slot(target_board);
                if let Some(row) = self.board_picker_row_for_board(target_board) {
                    self.board_picker_set_selected(row);
                }
            }
            self.needs_redraw = true;
            return true;
        }
        if drag.source_index == drag.current_index {
            self.needs_redraw = true;
            return true;
        }
        self.reorder_page_in_board(drag.board_index, drag.source_index, drag.current_index);
        self.needs_redraw = true;
        true
    }

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
        self.board_picker_page_edit = Some(super::BoardPickerPageEdit {
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
        let search_label = if search.is_empty() {
            "Type: jump".to_string()
        } else {
            format!(
                "Search: {}",
                truncate_search_label(search, BOARD_PICKER_SEARCH_MAX_LEN)
            )
        };
        let esc_label = if search.is_empty() {
            "Esc: close"
        } else {
            "Esc: clear"
        };
        if self.board_picker_is_quick() {
            format!("Enter: switch  {search_label}  {esc_label}")
        } else {
            format!(
                "Click: preview  Enter/Dbl-click: open  Ctrl+N: new  Ctrl+R: rename  Ctrl+C: color  Ctrl+P: pin  {search_label}  Del: delete  {esc_label}"
            )
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

    fn board_picker_promote_to_full(&mut self) {
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
        if let Some(row) = selected_row_full {
            *selected = row;
        }
        self.board_picker_layout = None;
        self.needs_redraw = true;
    }

    pub(crate) fn board_picker_row_count(&self) -> usize {
        let board_count = self.boards.board_count();
        if self.board_picker_is_quick() {
            board_count
        } else {
            board_count + 1
        }
    }

    pub(crate) fn board_picker_is_new_row(&self, index: usize) -> bool {
        !self.board_picker_is_quick() && index >= self.boards.board_count()
    }

    pub(crate) fn board_picker_activate_row(&mut self, index: usize) {
        let board_count = self.boards.board_count();
        if index < board_count {
            if let Some(board_index) = self.board_picker_board_index_for_row(index) {
                self.switch_board_slot(board_index);
                self.close_board_picker();
            }
        } else {
            self.board_picker_create_new();
        }
    }

    pub(crate) fn board_picker_activate_page(&mut self, page_index: usize) {
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return;
        };
        let page_count = self
            .boards
            .board_states()
            .get(board_index)
            .map(|board| board.pages.page_count())
            .unwrap_or(0);
        if page_index >= page_count {
            return;
        }
        if self.boards.active_index() != board_index {
            self.switch_board_slot(board_index);
        }
        self.switch_to_page(page_index);
        self.close_board_picker();
    }

    pub(crate) fn board_picker_add_page(&mut self) {
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return;
        };
        self.add_page_in_board(board_index);
    }

    pub(crate) fn board_picker_delete_page(&mut self, page_index: usize) {
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return;
        };
        self.delete_page_in_board(board_index, page_index);
    }

    pub(crate) fn board_picker_duplicate_page(&mut self, page_index: usize) {
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return;
        };
        let _ = self.duplicate_page_in_board(board_index, page_index);
    }

    pub(crate) fn board_picker_create_new(&mut self) {
        if self.board_picker_is_quick() {
            self.board_picker_promote_to_full();
        }
        if !self.create_board() {
            self.set_ui_toast(UiToastKind::Warning, "Board limit reached.");
            return;
        }
        let index = self.boards.active_index();
        if let Some(row) = self.board_picker_row_for_board(index) {
            self.board_picker_set_selected(row);
        }
        let name = self.boards.active_board_name().to_string();
        self.board_picker_start_edit(BoardPickerEditMode::Name, name);
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

    pub(crate) fn board_picker_delete_selected(&mut self) {
        let Some(index) = self.board_picker_selected_index() else {
            return;
        };
        if self.board_picker_is_new_row(index) {
            return;
        }
        let Some(board_index) = self.board_picker_board_index_for_row(index) else {
            return;
        };
        if self.boards.active_index() != board_index {
            self.switch_board_slot(board_index);
        }
        self.delete_active_board();
        if let Some(row) = self.board_picker_row_for_board(self.boards.active_index()) {
            self.board_picker_set_selected(row);
        }
    }

    pub(crate) fn board_picker_toggle_pin_selected(&mut self) {
        let Some(index) = self.board_picker_selected_index() else {
            return;
        };
        if self.board_picker_is_new_row(index) {
            return;
        }
        let Some(board_index) = self.board_picker_board_index_for_row(index) else {
            return;
        };
        if !self.toggle_board_pinned(board_index) {
            return;
        }
        let selected_row = self.board_picker_row_for_board(board_index);
        if let (Some(row), BoardPickerState::Open { selected, .. }) =
            (selected_row, &mut self.board_picker_state)
        {
            *selected = row;
        }
        self.board_picker_layout = None;
        self.needs_redraw = true;
    }

    fn board_picker_board_order(&self) -> Vec<usize> {
        self.board_picker_board_order_for_mode(self.board_picker_mode())
    }

    fn board_picker_board_order_for_mode(&self, _mode: BoardPickerMode) -> Vec<usize> {
        let board_count = self.boards.board_count();
        let mut order = Vec::with_capacity(board_count);
        for (index, board) in self.boards.board_states().iter().enumerate() {
            if board.spec.pinned {
                order.push(index);
            }
        }
        for (index, board) in self.boards.board_states().iter().enumerate() {
            if !board.spec.pinned {
                order.push(index);
            }
        }
        order
    }

    fn board_picker_clamp_drag_row(&self, row: usize, source_board: usize) -> usize {
        let board_count = self.boards.board_count();
        if board_count == 0 {
            return row;
        }
        let pinned_count = self
            .boards
            .board_states()
            .iter()
            .filter(|board| board.spec.pinned)
            .count();
        let is_pinned = self
            .boards
            .board_states()
            .get(source_board)
            .map(|board| board.spec.pinned)
            .unwrap_or(false);
        let min = if is_pinned { 0 } else { pinned_count };
        let max = if is_pinned {
            pinned_count.saturating_sub(1)
        } else {
            board_count.saturating_sub(1)
        };
        row.clamp(min, max)
    }

    pub(crate) fn board_picker_board_index_for_row(&self, row: usize) -> Option<usize> {
        self.board_picker_board_index_for_row_in_mode(row, self.board_picker_mode())
    }

    /// Returns the count of pinned boards.
    pub fn board_picker_pinned_count(&self) -> usize {
        self.boards
            .board_states()
            .iter()
            .filter(|b| b.spec.pinned)
            .count()
    }

    pub(crate) fn board_picker_row_for_board(&self, board_index: usize) -> Option<usize> {
        self.board_picker_row_for_board_in_mode(board_index, self.board_picker_mode())
    }

    fn board_picker_board_index_for_row_in_mode(
        &self,
        row: usize,
        mode: BoardPickerMode,
    ) -> Option<usize> {
        if row >= self.boards.board_count() {
            return None;
        }
        let order = self.board_picker_board_order_for_mode(mode);
        order.get(row).copied()
    }

    fn board_picker_row_for_board_in_mode(
        &self,
        board_index: usize,
        mode: BoardPickerMode,
    ) -> Option<usize> {
        if board_index >= self.boards.board_count() {
            return None;
        }
        let order = self.board_picker_board_order_for_mode(mode);
        order.iter().position(|&index| index == board_index)
    }
}
