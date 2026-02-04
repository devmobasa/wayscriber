use std::time::Instant;

use crate::input::InputState;
use crate::input::state::core::{BoardPickerClickState, MenuCommand};

use super::super::{BOARD_PICKER_DOUBLE_CLICK_DISTANCE, BOARD_PICKER_DOUBLE_CLICK_MS};

pub(super) fn handle_color_picker_popup_release(state: &mut InputState, x: i32, y: i32) -> bool {
    if !state.is_color_picker_popup_open() {
        return false;
    }

    // Stop dragging on release
    state.color_picker_popup_set_dragging(false);

    let layout = match state.color_picker_popup_layout() {
        Some(layout) => layout,
        None => {
            // No layout, close popup
            state.close_color_picker_popup(true);
            return true;
        }
    };

    let fx = x as f64;
    let fy = y as f64;

    // Check OK button
    if layout.point_in_ok_button(fx, fy) {
        state.apply_color_picker_popup();
        return true;
    }

    // Check Cancel button
    if layout.point_in_cancel_button(fx, fy) {
        state.close_color_picker_popup(true);
        return true;
    }

    // Check gradient click
    if layout.point_in_gradient(fx, fy) {
        let norm_x = (fx - layout.gradient_x) / layout.gradient_w;
        let norm_y = (fy - layout.gradient_y) / layout.gradient_h;
        state.color_picker_popup_set_from_gradient(norm_x, norm_y);
        // Unfocus hex input when clicking gradient
        state.color_picker_popup_set_hex_editing(false);
        return true;
    }

    // Check hex input click
    if layout.point_in_hex_input(fx, fy) {
        state.color_picker_popup_set_hex_editing(true);
        return true;
    }

    // Click outside panel closes popup
    if !layout.point_in_panel(fx, fy) {
        state.close_color_picker_popup(true);
        return true;
    }

    // Click somewhere else on panel - unfocus hex input
    state.color_picker_popup_set_hex_editing(false);
    state.needs_redraw = true;
    true
}

pub(super) fn handle_board_picker_release(state: &mut InputState, x: i32, y: i32) -> bool {
    if !state.is_board_picker_open() {
        return false;
    }
    if state.board_picker_is_page_dragging() {
        state.board_picker_finish_page_drag();
        return true;
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
    if state.board_picker_page_add_button_at(x, y) {
        state.board_picker_add_page();
        state.needs_redraw = true;
        return true;
    }
    if let Some(index) = state.board_picker_page_delete_index_at(x, y) {
        state.board_picker_delete_page(index);
        state.needs_redraw = true;
        return true;
    }
    if let Some(index) = state.board_picker_page_index_at(x, y) {
        state.board_picker_activate_page(index);
        state.needs_redraw = true;
        return true;
    }
    if state.board_picker_page_overflow_at(x, y) {
        state.update_pointer_position_synthetic(x, y);
        state.execute_menu_command(MenuCommand::OpenPagesMenu);
        return true;
    }
    if let Some(index) = state.board_picker_open_icon_index_at(x, y)
        && !state.board_picker_is_new_row(index)
    {
        state.board_picker_set_selected(index);
        state.board_picker_activate_row(index);
        state.needs_redraw = true;
        return true;
    }
    if let Some(index) = state.board_picker_index_at(x, y) {
        state.board_picker_set_selected(index);
        if state.board_picker_is_quick() || state.board_picker_is_new_row(index) {
            state.board_picker_activate_row(index);
            state.needs_redraw = true;
            return true;
        }
        let now = Instant::now();
        let is_double = state
            .last_board_picker_click
            .map(|last| {
                last.row == index
                    && now.duration_since(last.at).as_millis()
                        <= BOARD_PICKER_DOUBLE_CLICK_MS as u128
                    && (x - last.x).abs() <= BOARD_PICKER_DOUBLE_CLICK_DISTANCE
                    && (y - last.y).abs() <= BOARD_PICKER_DOUBLE_CLICK_DISTANCE
            })
            .unwrap_or(false);
        if is_double {
            state.last_board_picker_click = None;
            state.board_picker_activate_row(index);
        } else {
            state.last_board_picker_click = Some(BoardPickerClickState {
                row: index,
                x,
                y,
                at: now,
            });
        }
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
                if let Some(command) = entry.command.clone() {
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
