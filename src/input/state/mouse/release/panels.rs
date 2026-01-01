use crate::input::InputState;

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
