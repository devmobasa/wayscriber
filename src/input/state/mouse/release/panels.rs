use crate::input::InputState;

pub(super) fn handle_board_picker_release(state: &mut InputState, x: i32, y: i32) -> bool {
    if !state.is_board_picker_open() {
        return false;
    }
    if state.board_picker_is_dragging() {
        state.board_picker_finish_drag();
        return true;
    }
    if let Some(index) = state.board_picker_pin_index_at(x, y) {
        state.board_picker_set_selected(index);
        state.board_picker_toggle_pin_selected();
        return true;
    }
    if let Some(color) = state.board_picker_palette_color_at(x, y) {
        state.board_picker_apply_palette_color(color);
        return true;
    }
    if let Some(index) = state.board_picker_swatch_index_at(x, y) {
        state.board_picker_set_selected(index);
        state.board_picker_edit_color_selected();
        state.needs_redraw = true;
        return true;
    }
    if let Some(index) = state.board_picker_index_at(x, y) {
        state.board_picker_set_selected(index);
        state.board_picker_activate_row(index);
    } else {
        state.close_board_picker();
    }
    state.needs_redraw = true;
    true
}

pub(super) fn handle_properties_panel_release(state: &mut InputState, x: i32, y: i32) -> bool {
    if !state.is_properties_panel_open() {
        return false;
    }
    if state.properties_panel_layout().is_none() {
        return true;
    }
    if let Some(index) = state.properties_panel_index_at(x, y) {
        state.set_properties_panel_focus(Some(index));
        state.activate_properties_panel_entry();
    } else {
        state.close_properties_panel();
    }
    state.needs_redraw = true;
    true
}

pub(super) fn handle_context_menu_release(state: &mut InputState, x: i32, y: i32) -> bool {
    if !state.is_context_menu_open() {
        return false;
    }
    if let Some(index) = state.context_menu_index_at(x, y) {
        let entries = state.context_menu_entries();
        if let Some(entry) = entries.get(index) {
            if !entry.disabled {
                if let Some(command) = entry.command {
                    state.execute_menu_command(command);
                } else {
                    state.close_context_menu();
                }
            } else {
                state.close_context_menu();
            }
        }
    } else {
        state.close_context_menu();
    }
    state.needs_redraw = true;
    true
}
