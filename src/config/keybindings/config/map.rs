use std::collections::HashMap;

use super::super::{Action, KeyBinding};
use super::types::KeybindingsConfig;

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
