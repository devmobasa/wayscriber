use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::super::super::defaults::*;

/// Configuration for all keybindings.
///
/// Each action can have multiple keybindings. Users specify them in config.toml as:
/// ```toml
/// [keybindings]
/// exit = ["Escape", "Ctrl+Q"]
/// undo = ["Ctrl+Z"]
/// clear_canvas = ["E"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KeybindingsConfig {
    #[serde(default = "default_exit")]
    pub exit: Vec<String>,

    #[serde(default = "default_enter_text_mode")]
    pub enter_text_mode: Vec<String>,

    #[serde(default = "default_enter_sticky_note_mode")]
    pub enter_sticky_note_mode: Vec<String>,

    #[serde(default = "default_clear_canvas")]
    pub clear_canvas: Vec<String>,

    #[serde(default = "default_undo")]
    pub undo: Vec<String>,

    #[serde(default = "default_redo")]
    pub redo: Vec<String>,

    #[serde(default)]
    pub undo_all: Vec<String>,

    #[serde(default)]
    pub redo_all: Vec<String>,

    #[serde(default)]
    pub undo_all_delayed: Vec<String>,

    #[serde(default)]
    pub redo_all_delayed: Vec<String>,

    #[serde(default = "default_duplicate_selection")]
    pub duplicate_selection: Vec<String>,

    #[serde(default = "default_copy_selection")]
    pub copy_selection: Vec<String>,

    #[serde(default = "default_paste_selection")]
    pub paste_selection: Vec<String>,

    #[serde(default = "default_select_all")]
    pub select_all: Vec<String>,

    #[serde(default = "default_move_selection_to_front")]
    pub move_selection_to_front: Vec<String>,

    #[serde(default = "default_move_selection_to_back")]
    pub move_selection_to_back: Vec<String>,

    #[serde(default = "default_nudge_selection_up")]
    pub nudge_selection_up: Vec<String>,

    #[serde(default = "default_nudge_selection_down")]
    pub nudge_selection_down: Vec<String>,

    #[serde(default = "default_nudge_selection_left")]
    pub nudge_selection_left: Vec<String>,

    #[serde(default = "default_nudge_selection_right")]
    pub nudge_selection_right: Vec<String>,

    #[serde(default = "default_nudge_selection_up_large")]
    pub nudge_selection_up_large: Vec<String>,

    #[serde(default = "default_nudge_selection_down_large")]
    pub nudge_selection_down_large: Vec<String>,

    #[serde(default = "default_move_selection_to_start")]
    pub move_selection_to_start: Vec<String>,

    #[serde(default = "default_move_selection_to_end")]
    pub move_selection_to_end: Vec<String>,

    #[serde(default = "default_move_selection_to_top")]
    pub move_selection_to_top: Vec<String>,

    #[serde(default = "default_move_selection_to_bottom")]
    pub move_selection_to_bottom: Vec<String>,

    #[serde(default = "default_delete_selection")]
    pub delete_selection: Vec<String>,

    #[serde(default = "default_increase_thickness")]
    pub increase_thickness: Vec<String>,

    #[serde(default = "default_decrease_thickness")]
    pub decrease_thickness: Vec<String>,

    #[serde(default = "default_increase_marker_opacity")]
    pub increase_marker_opacity: Vec<String>,

    #[serde(default = "default_decrease_marker_opacity")]
    pub decrease_marker_opacity: Vec<String>,

    #[serde(default = "default_select_marker_tool")]
    pub select_marker_tool: Vec<String>,

    #[serde(default = "default_select_eraser_tool")]
    pub select_eraser_tool: Vec<String>,

    #[serde(default = "default_toggle_eraser_mode")]
    pub toggle_eraser_mode: Vec<String>,

    #[serde(default = "default_select_pen_tool")]
    pub select_pen_tool: Vec<String>,

    #[serde(default = "default_select_line_tool")]
    pub select_line_tool: Vec<String>,

    #[serde(default = "default_select_rect_tool")]
    pub select_rect_tool: Vec<String>,

    #[serde(default = "default_select_ellipse_tool")]
    pub select_ellipse_tool: Vec<String>,

    #[serde(default = "default_select_arrow_tool")]
    pub select_arrow_tool: Vec<String>,

    #[serde(default = "default_select_highlight_tool")]
    pub select_highlight_tool: Vec<String>,

    #[serde(default = "default_increase_font_size")]
    pub increase_font_size: Vec<String>,

    #[serde(default = "default_decrease_font_size")]
    pub decrease_font_size: Vec<String>,

    #[serde(default = "default_toggle_whiteboard")]
    pub toggle_whiteboard: Vec<String>,

    #[serde(default = "default_toggle_blackboard")]
    pub toggle_blackboard: Vec<String>,

    #[serde(default = "default_return_to_transparent")]
    pub return_to_transparent: Vec<String>,

    #[serde(default = "default_page_prev")]
    pub page_prev: Vec<String>,

    #[serde(default = "default_page_next")]
    pub page_next: Vec<String>,

    #[serde(default = "default_page_new")]
    pub page_new: Vec<String>,

    #[serde(default = "default_page_duplicate")]
    pub page_duplicate: Vec<String>,

    #[serde(default = "default_page_delete")]
    pub page_delete: Vec<String>,

    #[serde(default = "default_toggle_help")]
    pub toggle_help: Vec<String>,

    #[serde(default = "default_toggle_status_bar")]
    pub toggle_status_bar: Vec<String>,

    #[serde(default = "default_toggle_click_highlight")]
    pub toggle_click_highlight: Vec<String>,

    #[serde(default = "default_toggle_toolbar")]
    pub toggle_toolbar: Vec<String>,

    #[serde(default = "default_toggle_fill")]
    pub toggle_fill: Vec<String>,

    #[serde(default = "default_toggle_highlight_tool")]
    pub toggle_highlight_tool: Vec<String>,

    #[serde(default = "default_toggle_selection_properties")]
    pub toggle_selection_properties: Vec<String>,

    #[serde(default = "default_open_context_menu")]
    pub open_context_menu: Vec<String>,

    #[serde(default = "default_open_configurator")]
    pub open_configurator: Vec<String>,

    #[serde(default = "default_set_color_red")]
    pub set_color_red: Vec<String>,

    #[serde(default = "default_set_color_green")]
    pub set_color_green: Vec<String>,

    #[serde(default = "default_set_color_blue")]
    pub set_color_blue: Vec<String>,

    #[serde(default = "default_set_color_yellow")]
    pub set_color_yellow: Vec<String>,

    #[serde(default = "default_set_color_orange")]
    pub set_color_orange: Vec<String>,

    #[serde(default = "default_set_color_pink")]
    pub set_color_pink: Vec<String>,

    #[serde(default = "default_set_color_white")]
    pub set_color_white: Vec<String>,

    #[serde(default = "default_set_color_black")]
    pub set_color_black: Vec<String>,

    #[serde(default = "default_capture_full_screen")]
    pub capture_full_screen: Vec<String>,

    #[serde(default = "default_capture_active_window")]
    pub capture_active_window: Vec<String>,

    #[serde(default = "default_capture_selection")]
    pub capture_selection: Vec<String>,

    #[serde(default = "default_capture_clipboard_full")]
    pub capture_clipboard_full: Vec<String>,

    #[serde(default = "default_capture_file_full")]
    pub capture_file_full: Vec<String>,

    #[serde(default = "default_capture_clipboard_selection")]
    pub capture_clipboard_selection: Vec<String>,

    #[serde(default = "default_capture_file_selection")]
    pub capture_file_selection: Vec<String>,

    #[serde(default = "default_capture_clipboard_region")]
    pub capture_clipboard_region: Vec<String>,

    #[serde(default = "default_capture_file_region")]
    pub capture_file_region: Vec<String>,

    #[serde(default = "default_open_capture_folder")]
    pub open_capture_folder: Vec<String>,

    #[serde(default = "default_toggle_frozen_mode")]
    pub toggle_frozen_mode: Vec<String>,

    #[serde(default = "default_zoom_in")]
    pub zoom_in: Vec<String>,

    #[serde(default = "default_zoom_out")]
    pub zoom_out: Vec<String>,

    #[serde(default = "default_reset_zoom")]
    pub reset_zoom: Vec<String>,

    #[serde(default = "default_toggle_zoom_lock")]
    pub toggle_zoom_lock: Vec<String>,

    #[serde(default = "default_refresh_zoom_capture")]
    pub refresh_zoom_capture: Vec<String>,

    #[serde(default = "default_apply_preset_1")]
    pub apply_preset_1: Vec<String>,

    #[serde(default = "default_apply_preset_2")]
    pub apply_preset_2: Vec<String>,

    #[serde(default = "default_apply_preset_3")]
    pub apply_preset_3: Vec<String>,

    #[serde(default = "default_apply_preset_4")]
    pub apply_preset_4: Vec<String>,

    #[serde(default = "default_apply_preset_5")]
    pub apply_preset_5: Vec<String>,

    #[serde(default = "default_save_preset_1")]
    pub save_preset_1: Vec<String>,

    #[serde(default = "default_save_preset_2")]
    pub save_preset_2: Vec<String>,

    #[serde(default = "default_save_preset_3")]
    pub save_preset_3: Vec<String>,

    #[serde(default = "default_save_preset_4")]
    pub save_preset_4: Vec<String>,

    #[serde(default = "default_save_preset_5")]
    pub save_preset_5: Vec<String>,

    #[serde(default = "default_clear_preset_1")]
    pub clear_preset_1: Vec<String>,

    #[serde(default = "default_clear_preset_2")]
    pub clear_preset_2: Vec<String>,

    #[serde(default = "default_clear_preset_3")]
    pub clear_preset_3: Vec<String>,

    #[serde(default = "default_clear_preset_4")]
    pub clear_preset_4: Vec<String>,

    #[serde(default = "default_clear_preset_5")]
    pub clear_preset_5: Vec<String>,
}
