// Default keybinding lists.
pub(super) fn default_exit() -> Vec<String> {
    vec!["Escape".to_string(), "Ctrl+Q".to_string()]
}

pub(super) fn default_enter_text_mode() -> Vec<String> {
    vec!["T".to_string()]
}

pub(super) fn default_enter_sticky_note_mode() -> Vec<String> {
    vec!["N".to_string()]
}

pub(super) fn default_clear_canvas() -> Vec<String> {
    vec!["E".to_string()]
}

pub(super) fn default_undo() -> Vec<String> {
    vec!["Ctrl+Z".to_string()]
}

pub(super) fn default_redo() -> Vec<String> {
    vec!["Ctrl+Shift+Z".to_string(), "Ctrl+Y".to_string()]
}

pub(super) fn default_duplicate_selection() -> Vec<String> {
    vec!["Ctrl+D".to_string()]
}

pub(super) fn default_copy_selection() -> Vec<String> {
    vec!["Ctrl+Alt+C".to_string()]
}

pub(super) fn default_paste_selection() -> Vec<String> {
    vec!["Ctrl+Alt+V".to_string()]
}

pub(super) fn default_select_all() -> Vec<String> {
    vec!["Ctrl+A".to_string()]
}

pub(super) fn default_move_selection_to_front() -> Vec<String> {
    vec!["]".to_string()]
}

pub(super) fn default_move_selection_to_back() -> Vec<String> {
    vec!["[".to_string()]
}

pub(super) fn default_nudge_selection_up() -> Vec<String> {
    vec!["ArrowUp".to_string()]
}

pub(super) fn default_nudge_selection_down() -> Vec<String> {
    vec!["ArrowDown".to_string()]
}

pub(super) fn default_nudge_selection_left() -> Vec<String> {
    vec!["ArrowLeft".to_string(), "Shift+PageUp".to_string()]
}

pub(super) fn default_nudge_selection_right() -> Vec<String> {
    vec!["ArrowRight".to_string(), "Shift+PageDown".to_string()]
}

pub(super) fn default_nudge_selection_up_large() -> Vec<String> {
    vec!["PageUp".to_string()]
}

pub(super) fn default_nudge_selection_down_large() -> Vec<String> {
    vec!["PageDown".to_string()]
}

pub(super) fn default_move_selection_to_start() -> Vec<String> {
    vec!["Home".to_string()]
}

pub(super) fn default_move_selection_to_end() -> Vec<String> {
    vec!["End".to_string()]
}

pub(super) fn default_move_selection_to_top() -> Vec<String> {
    vec!["Ctrl+Home".to_string()]
}

pub(super) fn default_move_selection_to_bottom() -> Vec<String> {
    vec!["Ctrl+End".to_string()]
}

pub(super) fn default_delete_selection() -> Vec<String> {
    vec!["Delete".to_string()]
}

pub(super) fn default_increase_thickness() -> Vec<String> {
    vec!["+".to_string(), "=".to_string()]
}

pub(super) fn default_decrease_thickness() -> Vec<String> {
    vec!["-".to_string(), "_".to_string()]
}

pub(super) fn default_increase_marker_opacity() -> Vec<String> {
    vec!["Ctrl+Alt+ArrowUp".to_string()]
}

pub(super) fn default_decrease_marker_opacity() -> Vec<String> {
    vec!["Ctrl+Alt+ArrowDown".to_string()]
}

pub(super) fn default_select_marker_tool() -> Vec<String> {
    vec!["H".to_string()]
}

pub(super) fn default_select_eraser_tool() -> Vec<String> {
    vec!["D".to_string()]
}

pub(super) fn default_toggle_eraser_mode() -> Vec<String> {
    vec!["Ctrl+Shift+E".to_string()]
}

pub(super) fn default_select_pen_tool() -> Vec<String> {
    vec!["F".to_string()]
}

pub(super) fn default_select_line_tool() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_select_rect_tool() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_select_ellipse_tool() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_select_arrow_tool() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_select_highlight_tool() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_increase_font_size() -> Vec<String> {
    vec!["Ctrl+Shift++".to_string(), "Ctrl+Shift+=".to_string()]
}

pub(super) fn default_decrease_font_size() -> Vec<String> {
    vec!["Ctrl+Shift+-".to_string(), "Ctrl+Shift+_".to_string()]
}

pub(super) fn default_toggle_whiteboard() -> Vec<String> {
    vec!["Ctrl+W".to_string()]
}

pub(super) fn default_toggle_blackboard() -> Vec<String> {
    vec!["Ctrl+B".to_string()]
}

pub(super) fn default_return_to_transparent() -> Vec<String> {
    vec!["Ctrl+Shift+T".to_string()]
}

pub(super) fn default_page_prev() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_page_next() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_page_new() -> Vec<String> {
    vec!["Ctrl+Alt+N".to_string()]
}

pub(super) fn default_page_duplicate() -> Vec<String> {
    vec!["Ctrl+Alt+D".to_string()]
}

pub(super) fn default_page_delete() -> Vec<String> {
    vec!["Ctrl+Alt+Delete".to_string()]
}

pub(super) fn default_toggle_help() -> Vec<String> {
    vec!["F10".to_string(), "F1".to_string()]
}

pub(super) fn default_toggle_status_bar() -> Vec<String> {
    vec!["F12".to_string(), "F4".to_string()]
}

pub(super) fn default_toggle_click_highlight() -> Vec<String> {
    vec!["Ctrl+Shift+H".to_string()]
}

pub(super) fn default_toggle_toolbar() -> Vec<String> {
    vec!["F2".to_string(), "F9".to_string()]
}

pub(super) fn default_toggle_fill() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_toggle_highlight_tool() -> Vec<String> {
    vec!["Ctrl+Alt+H".to_string()]
}

pub(super) fn default_toggle_selection_properties() -> Vec<String> {
    vec!["Ctrl+Alt+P".to_string()]
}

pub(super) fn default_open_context_menu() -> Vec<String> {
    vec!["Shift+F10".to_string(), "Menu".to_string()]
}

pub(super) fn default_open_configurator() -> Vec<String> {
    vec!["F11".to_string()]
}

pub(super) fn default_set_color_red() -> Vec<String> {
    vec!["R".to_string()]
}

pub(super) fn default_set_color_green() -> Vec<String> {
    vec!["G".to_string()]
}

pub(super) fn default_set_color_blue() -> Vec<String> {
    vec!["B".to_string()]
}

pub(super) fn default_set_color_yellow() -> Vec<String> {
    vec!["Y".to_string()]
}

pub(super) fn default_set_color_orange() -> Vec<String> {
    vec!["O".to_string()]
}

pub(super) fn default_set_color_pink() -> Vec<String> {
    vec!["P".to_string()]
}

pub(super) fn default_set_color_white() -> Vec<String> {
    vec!["W".to_string()]
}

pub(super) fn default_set_color_black() -> Vec<String> {
    vec!["K".to_string()]
}

pub(super) fn default_capture_full_screen() -> Vec<String> {
    vec!["Ctrl+Shift+P".to_string()]
}

pub(super) fn default_capture_active_window() -> Vec<String> {
    vec!["Ctrl+Shift+O".to_string()]
}

pub(super) fn default_capture_selection() -> Vec<String> {
    vec!["Ctrl+Shift+I".to_string()]
}

pub(super) fn default_capture_clipboard_full() -> Vec<String> {
    vec!["Ctrl+C".to_string()]
}

pub(super) fn default_capture_file_full() -> Vec<String> {
    vec!["Ctrl+S".to_string()]
}

pub(super) fn default_capture_clipboard_selection() -> Vec<String> {
    vec!["Ctrl+Shift+C".to_string()]
}

pub(super) fn default_capture_file_selection() -> Vec<String> {
    vec!["Ctrl+Shift+S".to_string()]
}

pub(super) fn default_capture_clipboard_region() -> Vec<String> {
    vec!["Ctrl+6".to_string()]
}

pub(super) fn default_capture_file_region() -> Vec<String> {
    vec!["Ctrl+Shift+6".to_string()]
}

pub(super) fn default_open_capture_folder() -> Vec<String> {
    vec!["Ctrl+Alt+O".to_string()]
}

pub(super) fn default_toggle_frozen_mode() -> Vec<String> {
    vec!["Ctrl+Shift+F".to_string()]
}

pub(super) fn default_zoom_in() -> Vec<String> {
    vec!["Ctrl+Alt++".to_string(), "Ctrl+Alt+=".to_string()]
}

pub(super) fn default_zoom_out() -> Vec<String> {
    vec!["Ctrl+Alt+-".to_string(), "Ctrl+Alt+_".to_string()]
}

pub(super) fn default_reset_zoom() -> Vec<String> {
    vec!["Ctrl+Alt+0".to_string()]
}

pub(super) fn default_toggle_zoom_lock() -> Vec<String> {
    vec!["Ctrl+Alt+L".to_string()]
}

pub(super) fn default_refresh_zoom_capture() -> Vec<String> {
    vec!["Ctrl+Alt+R".to_string()]
}

pub(super) fn default_apply_preset_1() -> Vec<String> {
    vec!["1".to_string()]
}

pub(super) fn default_apply_preset_2() -> Vec<String> {
    vec!["2".to_string()]
}

pub(super) fn default_apply_preset_3() -> Vec<String> {
    vec!["3".to_string()]
}

pub(super) fn default_apply_preset_4() -> Vec<String> {
    vec!["4".to_string()]
}

pub(super) fn default_apply_preset_5() -> Vec<String> {
    vec!["5".to_string()]
}

pub(super) fn default_save_preset_1() -> Vec<String> {
    vec!["Shift+1".to_string()]
}

pub(super) fn default_save_preset_2() -> Vec<String> {
    vec!["Shift+2".to_string()]
}

pub(super) fn default_save_preset_3() -> Vec<String> {
    vec!["Shift+3".to_string()]
}

pub(super) fn default_save_preset_4() -> Vec<String> {
    vec!["Shift+4".to_string()]
}

pub(super) fn default_save_preset_5() -> Vec<String> {
    vec!["Shift+5".to_string()]
}

pub(super) fn default_clear_preset_1() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_clear_preset_2() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_clear_preset_3() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_clear_preset_4() -> Vec<String> {
    Vec::new()
}

pub(super) fn default_clear_preset_5() -> Vec<String> {
    Vec::new()
}
