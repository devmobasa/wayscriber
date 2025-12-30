use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::defaults::*;
use super::{Action, KeyBinding};

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

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            exit: default_exit(),
            enter_text_mode: default_enter_text_mode(),
            enter_sticky_note_mode: default_enter_sticky_note_mode(),
            clear_canvas: default_clear_canvas(),
            undo: default_undo(),
            redo: default_redo(),
            undo_all: Vec::new(),
            redo_all: Vec::new(),
            undo_all_delayed: Vec::new(),
            redo_all_delayed: Vec::new(),
            duplicate_selection: default_duplicate_selection(),
            copy_selection: default_copy_selection(),
            paste_selection: default_paste_selection(),
            select_all: default_select_all(),
            move_selection_to_front: default_move_selection_to_front(),
            move_selection_to_back: default_move_selection_to_back(),
            nudge_selection_up: default_nudge_selection_up(),
            nudge_selection_down: default_nudge_selection_down(),
            nudge_selection_left: default_nudge_selection_left(),
            nudge_selection_right: default_nudge_selection_right(),
            nudge_selection_up_large: default_nudge_selection_up_large(),
            nudge_selection_down_large: default_nudge_selection_down_large(),
            move_selection_to_start: default_move_selection_to_start(),
            move_selection_to_end: default_move_selection_to_end(),
            move_selection_to_top: default_move_selection_to_top(),
            move_selection_to_bottom: default_move_selection_to_bottom(),
            delete_selection: default_delete_selection(),
            increase_thickness: default_increase_thickness(),
            decrease_thickness: default_decrease_thickness(),
            increase_marker_opacity: default_increase_marker_opacity(),
            decrease_marker_opacity: default_decrease_marker_opacity(),
            select_marker_tool: default_select_marker_tool(),
            select_eraser_tool: default_select_eraser_tool(),
            toggle_eraser_mode: default_toggle_eraser_mode(),
            select_pen_tool: default_select_pen_tool(),
            select_line_tool: default_select_line_tool(),
            select_rect_tool: default_select_rect_tool(),
            select_ellipse_tool: default_select_ellipse_tool(),
            select_arrow_tool: default_select_arrow_tool(),
            select_highlight_tool: default_select_highlight_tool(),
            increase_font_size: default_increase_font_size(),
            decrease_font_size: default_decrease_font_size(),
            toggle_whiteboard: default_toggle_whiteboard(),
            toggle_blackboard: default_toggle_blackboard(),
            return_to_transparent: default_return_to_transparent(),
            page_prev: default_page_prev(),
            page_next: default_page_next(),
            page_new: default_page_new(),
            page_duplicate: default_page_duplicate(),
            page_delete: default_page_delete(),
            toggle_help: default_toggle_help(),
            toggle_status_bar: default_toggle_status_bar(),
            toggle_click_highlight: default_toggle_click_highlight(),
            toggle_toolbar: default_toggle_toolbar(),
            toggle_fill: default_toggle_fill(),
            toggle_highlight_tool: default_toggle_highlight_tool(),
            toggle_selection_properties: default_toggle_selection_properties(),
            open_context_menu: default_open_context_menu(),
            open_configurator: default_open_configurator(),
            set_color_red: default_set_color_red(),
            set_color_green: default_set_color_green(),
            set_color_blue: default_set_color_blue(),
            set_color_yellow: default_set_color_yellow(),
            set_color_orange: default_set_color_orange(),
            set_color_pink: default_set_color_pink(),
            set_color_white: default_set_color_white(),
            set_color_black: default_set_color_black(),
            capture_full_screen: default_capture_full_screen(),
            capture_active_window: default_capture_active_window(),
            capture_selection: default_capture_selection(),
            capture_clipboard_full: default_capture_clipboard_full(),
            capture_file_full: default_capture_file_full(),
            capture_clipboard_selection: default_capture_clipboard_selection(),
            capture_file_selection: default_capture_file_selection(),
            capture_clipboard_region: default_capture_clipboard_region(),
            capture_file_region: default_capture_file_region(),
            open_capture_folder: default_open_capture_folder(),
            toggle_frozen_mode: default_toggle_frozen_mode(),
            zoom_in: default_zoom_in(),
            zoom_out: default_zoom_out(),
            reset_zoom: default_reset_zoom(),
            toggle_zoom_lock: default_toggle_zoom_lock(),
            refresh_zoom_capture: default_refresh_zoom_capture(),
            apply_preset_1: default_apply_preset_1(),
            apply_preset_2: default_apply_preset_2(),
            apply_preset_3: default_apply_preset_3(),
            apply_preset_4: default_apply_preset_4(),
            apply_preset_5: default_apply_preset_5(),
            save_preset_1: default_save_preset_1(),
            save_preset_2: default_save_preset_2(),
            save_preset_3: default_save_preset_3(),
            save_preset_4: default_save_preset_4(),
            save_preset_5: default_save_preset_5(),
            clear_preset_1: default_clear_preset_1(),
            clear_preset_2: default_clear_preset_2(),
            clear_preset_3: default_clear_preset_3(),
            clear_preset_4: default_clear_preset_4(),
            clear_preset_5: default_clear_preset_5(),
        }
    }
}

impl KeybindingsConfig {
    /// Build a lookup map from keybindings to actions for efficient matching.
    /// Returns an error if any keybinding string is invalid or if duplicates are detected.
    pub fn build_action_map(&self) -> Result<HashMap<KeyBinding, Action>, String> {
        let mut map = HashMap::new();

        // Helper closure to insert and check for duplicates
        let mut insert_binding = |binding_str: &str, action: Action| -> Result<(), String> {
            let binding = KeyBinding::parse(binding_str)?;
            if let Some(existing_action) = map.insert(binding.clone(), action) {
                return Err(format!(
                    "Duplicate keybinding '{}' assigned to both {:?} and {:?}",
                    binding_str, existing_action, action
                ));
            }
            Ok(())
        };

        for binding_str in &self.exit {
            insert_binding(binding_str, Action::Exit)?;
        }

        for binding_str in &self.enter_text_mode {
            insert_binding(binding_str, Action::EnterTextMode)?;
        }
        for binding_str in &self.enter_sticky_note_mode {
            insert_binding(binding_str, Action::EnterStickyNoteMode)?;
        }

        for binding_str in &self.clear_canvas {
            insert_binding(binding_str, Action::ClearCanvas)?;
        }

        for binding_str in &self.undo {
            insert_binding(binding_str, Action::Undo)?;
        }

        for binding_str in &self.redo {
            insert_binding(binding_str, Action::Redo)?;
        }

        for binding_str in &self.undo_all {
            insert_binding(binding_str, Action::UndoAll)?;
        }

        for binding_str in &self.redo_all {
            insert_binding(binding_str, Action::RedoAll)?;
        }

        for binding_str in &self.undo_all_delayed {
            insert_binding(binding_str, Action::UndoAllDelayed)?;
        }

        for binding_str in &self.redo_all_delayed {
            insert_binding(binding_str, Action::RedoAllDelayed)?;
        }

        for binding_str in &self.duplicate_selection {
            insert_binding(binding_str, Action::DuplicateSelection)?;
        }

        for binding_str in &self.copy_selection {
            insert_binding(binding_str, Action::CopySelection)?;
        }

        for binding_str in &self.paste_selection {
            insert_binding(binding_str, Action::PasteSelection)?;
        }

        for binding_str in &self.select_all {
            insert_binding(binding_str, Action::SelectAll)?;
        }

        for binding_str in &self.move_selection_to_front {
            insert_binding(binding_str, Action::MoveSelectionToFront)?;
        }

        for binding_str in &self.move_selection_to_back {
            insert_binding(binding_str, Action::MoveSelectionToBack)?;
        }

        for binding_str in &self.nudge_selection_up {
            insert_binding(binding_str, Action::NudgeSelectionUp)?;
        }

        for binding_str in &self.nudge_selection_down {
            insert_binding(binding_str, Action::NudgeSelectionDown)?;
        }

        for binding_str in &self.nudge_selection_left {
            insert_binding(binding_str, Action::NudgeSelectionLeft)?;
        }

        for binding_str in &self.nudge_selection_right {
            insert_binding(binding_str, Action::NudgeSelectionRight)?;
        }

        for binding_str in &self.nudge_selection_up_large {
            insert_binding(binding_str, Action::NudgeSelectionUpLarge)?;
        }

        for binding_str in &self.nudge_selection_down_large {
            insert_binding(binding_str, Action::NudgeSelectionDownLarge)?;
        }

        for binding_str in &self.move_selection_to_start {
            insert_binding(binding_str, Action::MoveSelectionToStart)?;
        }

        for binding_str in &self.move_selection_to_end {
            insert_binding(binding_str, Action::MoveSelectionToEnd)?;
        }

        for binding_str in &self.move_selection_to_top {
            insert_binding(binding_str, Action::MoveSelectionToTop)?;
        }

        for binding_str in &self.move_selection_to_bottom {
            insert_binding(binding_str, Action::MoveSelectionToBottom)?;
        }

        for binding_str in &self.delete_selection {
            insert_binding(binding_str, Action::DeleteSelection)?;
        }

        for binding_str in &self.increase_thickness {
            insert_binding(binding_str, Action::IncreaseThickness)?;
        }

        for binding_str in &self.decrease_thickness {
            insert_binding(binding_str, Action::DecreaseThickness)?;
        }

        for binding_str in &self.increase_marker_opacity {
            insert_binding(binding_str, Action::IncreaseMarkerOpacity)?;
        }

        for binding_str in &self.decrease_marker_opacity {
            insert_binding(binding_str, Action::DecreaseMarkerOpacity)?;
        }

        for binding_str in &self.select_marker_tool {
            insert_binding(binding_str, Action::SelectMarkerTool)?;
        }

        for binding_str in &self.select_eraser_tool {
            insert_binding(binding_str, Action::SelectEraserTool)?;
        }

        for binding_str in &self.toggle_eraser_mode {
            insert_binding(binding_str, Action::ToggleEraserMode)?;
        }

        for binding_str in &self.select_pen_tool {
            insert_binding(binding_str, Action::SelectPenTool)?;
        }

        for binding_str in &self.select_line_tool {
            insert_binding(binding_str, Action::SelectLineTool)?;
        }

        for binding_str in &self.select_rect_tool {
            insert_binding(binding_str, Action::SelectRectTool)?;
        }

        for binding_str in &self.select_ellipse_tool {
            insert_binding(binding_str, Action::SelectEllipseTool)?;
        }

        for binding_str in &self.select_arrow_tool {
            insert_binding(binding_str, Action::SelectArrowTool)?;
        }

        for binding_str in &self.select_highlight_tool {
            insert_binding(binding_str, Action::SelectHighlightTool)?;
        }

        for binding_str in &self.increase_font_size {
            insert_binding(binding_str, Action::IncreaseFontSize)?;
        }

        for binding_str in &self.decrease_font_size {
            insert_binding(binding_str, Action::DecreaseFontSize)?;
        }

        for binding_str in &self.toggle_whiteboard {
            insert_binding(binding_str, Action::ToggleWhiteboard)?;
        }

        for binding_str in &self.toggle_blackboard {
            insert_binding(binding_str, Action::ToggleBlackboard)?;
        }

        for binding_str in &self.return_to_transparent {
            insert_binding(binding_str, Action::ReturnToTransparent)?;
        }

        for binding_str in &self.page_prev {
            insert_binding(binding_str, Action::PagePrev)?;
        }

        for binding_str in &self.page_next {
            insert_binding(binding_str, Action::PageNext)?;
        }

        for binding_str in &self.page_new {
            insert_binding(binding_str, Action::PageNew)?;
        }

        for binding_str in &self.page_duplicate {
            insert_binding(binding_str, Action::PageDuplicate)?;
        }

        for binding_str in &self.page_delete {
            insert_binding(binding_str, Action::PageDelete)?;
        }

        for binding_str in &self.toggle_help {
            insert_binding(binding_str, Action::ToggleHelp)?;
        }

        for binding_str in &self.toggle_status_bar {
            insert_binding(binding_str, Action::ToggleStatusBar)?;
        }

        for binding_str in &self.toggle_click_highlight {
            insert_binding(binding_str, Action::ToggleClickHighlight)?;
        }

        for binding_str in &self.toggle_toolbar {
            insert_binding(binding_str, Action::ToggleToolbar)?;
        }

        for binding_str in &self.toggle_fill {
            insert_binding(binding_str, Action::ToggleFill)?;
        }

        for binding_str in &self.toggle_highlight_tool {
            insert_binding(binding_str, Action::ToggleHighlightTool)?;
        }

        for binding_str in &self.toggle_selection_properties {
            insert_binding(binding_str, Action::ToggleSelectionProperties)?;
        }

        for binding_str in &self.open_context_menu {
            insert_binding(binding_str, Action::OpenContextMenu)?;
        }

        for binding_str in &self.open_configurator {
            insert_binding(binding_str, Action::OpenConfigurator)?;
        }

        for binding_str in &self.set_color_red {
            insert_binding(binding_str, Action::SetColorRed)?;
        }

        for binding_str in &self.set_color_green {
            insert_binding(binding_str, Action::SetColorGreen)?;
        }

        for binding_str in &self.set_color_blue {
            insert_binding(binding_str, Action::SetColorBlue)?;
        }

        for binding_str in &self.set_color_yellow {
            insert_binding(binding_str, Action::SetColorYellow)?;
        }

        for binding_str in &self.set_color_orange {
            insert_binding(binding_str, Action::SetColorOrange)?;
        }

        for binding_str in &self.set_color_pink {
            insert_binding(binding_str, Action::SetColorPink)?;
        }

        for binding_str in &self.set_color_white {
            insert_binding(binding_str, Action::SetColorWhite)?;
        }

        for binding_str in &self.set_color_black {
            insert_binding(binding_str, Action::SetColorBlack)?;
        }

        for binding_str in &self.capture_full_screen {
            insert_binding(binding_str, Action::CaptureFullScreen)?;
        }

        for binding_str in &self.capture_active_window {
            insert_binding(binding_str, Action::CaptureActiveWindow)?;
        }

        for binding_str in &self.capture_selection {
            insert_binding(binding_str, Action::CaptureSelection)?;
        }

        for binding_str in &self.capture_clipboard_full {
            insert_binding(binding_str, Action::CaptureClipboardFull)?;
        }

        for binding_str in &self.capture_file_full {
            insert_binding(binding_str, Action::CaptureFileFull)?;
        }

        for binding_str in &self.capture_clipboard_selection {
            insert_binding(binding_str, Action::CaptureClipboardSelection)?;
        }

        for binding_str in &self.capture_file_selection {
            insert_binding(binding_str, Action::CaptureFileSelection)?;
        }

        for binding_str in &self.capture_clipboard_region {
            insert_binding(binding_str, Action::CaptureClipboardRegion)?;
        }

        for binding_str in &self.capture_file_region {
            insert_binding(binding_str, Action::CaptureFileRegion)?;
        }

        for binding_str in &self.open_capture_folder {
            insert_binding(binding_str, Action::OpenCaptureFolder)?;
        }

        for binding_str in &self.toggle_frozen_mode {
            insert_binding(binding_str, Action::ToggleFrozenMode)?;
        }

        for binding_str in &self.zoom_in {
            insert_binding(binding_str, Action::ZoomIn)?;
        }

        for binding_str in &self.zoom_out {
            insert_binding(binding_str, Action::ZoomOut)?;
        }

        for binding_str in &self.reset_zoom {
            insert_binding(binding_str, Action::ResetZoom)?;
        }

        for binding_str in &self.toggle_zoom_lock {
            insert_binding(binding_str, Action::ToggleZoomLock)?;
        }

        for binding_str in &self.refresh_zoom_capture {
            insert_binding(binding_str, Action::RefreshZoomCapture)?;
        }

        for binding_str in &self.apply_preset_1 {
            insert_binding(binding_str, Action::ApplyPreset1)?;
        }
        for binding_str in &self.apply_preset_2 {
            insert_binding(binding_str, Action::ApplyPreset2)?;
        }
        for binding_str in &self.apply_preset_3 {
            insert_binding(binding_str, Action::ApplyPreset3)?;
        }
        for binding_str in &self.apply_preset_4 {
            insert_binding(binding_str, Action::ApplyPreset4)?;
        }
        for binding_str in &self.apply_preset_5 {
            insert_binding(binding_str, Action::ApplyPreset5)?;
        }
        for binding_str in &self.save_preset_1 {
            insert_binding(binding_str, Action::SavePreset1)?;
        }
        for binding_str in &self.save_preset_2 {
            insert_binding(binding_str, Action::SavePreset2)?;
        }
        for binding_str in &self.save_preset_3 {
            insert_binding(binding_str, Action::SavePreset3)?;
        }
        for binding_str in &self.save_preset_4 {
            insert_binding(binding_str, Action::SavePreset4)?;
        }
        for binding_str in &self.save_preset_5 {
            insert_binding(binding_str, Action::SavePreset5)?;
        }
        for binding_str in &self.clear_preset_1 {
            insert_binding(binding_str, Action::ClearPreset1)?;
        }
        for binding_str in &self.clear_preset_2 {
            insert_binding(binding_str, Action::ClearPreset2)?;
        }
        for binding_str in &self.clear_preset_3 {
            insert_binding(binding_str, Action::ClearPreset3)?;
        }
        for binding_str in &self.clear_preset_4 {
            insert_binding(binding_str, Action::ClearPreset4)?;
        }
        for binding_str in &self.clear_preset_5 {
            insert_binding(binding_str, Action::ClearPreset5)?;
        }

        Ok(map)
    }
}
