mod actions;
mod boards;
mod delays;
mod layout;
mod pages;
mod tools;

use crate::input::InputState;

use super::events::ToolbarEvent;

impl InputState {
    /// Applies a toolbar-originated event to the input state.
    ///
    /// Returns true if the event resulted in a state change.
    pub fn apply_toolbar_event(&mut self, event: ToolbarEvent) -> bool {
        match event {
            ToolbarEvent::SelectTool(tool) => self.apply_toolbar_select_tool(tool),
            ToolbarEvent::SetColor(color) => self.apply_toolbar_set_color(color),
            ToolbarEvent::SetThickness(value) => self.apply_toolbar_set_thickness(value),
            ToolbarEvent::SetMarkerOpacity(value) => self.apply_toolbar_set_marker_opacity(value),
            ToolbarEvent::SetEraserMode(mode) => self.apply_toolbar_set_eraser_mode(mode),
            ToolbarEvent::SetFont(descriptor) => self.apply_toolbar_set_font(descriptor),
            ToolbarEvent::SetFontSize(size) => self.apply_toolbar_set_font_size(size),
            ToolbarEvent::ToggleFill(enable) => self.apply_toolbar_toggle_fill(enable),
            ToolbarEvent::ToggleArrowLabels(enable) => {
                self.apply_toolbar_toggle_arrow_labels(enable)
            }
            ToolbarEvent::ResetArrowLabelCounter => self.apply_toolbar_reset_arrow_label_counter(),
            ToolbarEvent::SetUndoDelay(delay_secs) => self.apply_toolbar_set_undo_delay(delay_secs),
            ToolbarEvent::SetRedoDelay(delay_secs) => self.apply_toolbar_set_redo_delay(delay_secs),
            ToolbarEvent::SetCustomUndoDelay(delay_secs) => {
                self.apply_toolbar_set_custom_undo_delay(delay_secs)
            }
            ToolbarEvent::SetCustomRedoDelay(delay_secs) => {
                self.apply_toolbar_set_custom_redo_delay(delay_secs)
            }
            ToolbarEvent::SetCustomUndoSteps(steps) => {
                self.apply_toolbar_set_custom_undo_steps(steps)
            }
            ToolbarEvent::SetCustomRedoSteps(steps) => {
                self.apply_toolbar_set_custom_redo_steps(steps)
            }
            ToolbarEvent::NudgeThickness(delta) => self.apply_toolbar_nudge_thickness(delta),
            ToolbarEvent::NudgeMarkerOpacity(delta) => {
                self.apply_toolbar_nudge_marker_opacity(delta)
            }
            ToolbarEvent::Undo => self.apply_toolbar_undo(),
            ToolbarEvent::Redo => self.apply_toolbar_redo(),
            ToolbarEvent::UndoAll => self.apply_toolbar_undo_all(),
            ToolbarEvent::RedoAll => self.apply_toolbar_redo_all(),
            ToolbarEvent::UndoAllDelayed => self.apply_toolbar_undo_all_delayed(),
            ToolbarEvent::RedoAllDelayed => self.apply_toolbar_redo_all_delayed(),
            ToolbarEvent::CustomUndo => self.apply_toolbar_custom_undo(),
            ToolbarEvent::CustomRedo => self.apply_toolbar_custom_redo(),
            ToolbarEvent::ClearCanvas => self.apply_toolbar_clear_canvas(),
            ToolbarEvent::PagePrev => self.apply_toolbar_page_prev(),
            ToolbarEvent::PageNext => self.apply_toolbar_page_next(),
            ToolbarEvent::PageNew => self.apply_toolbar_page_new(),
            ToolbarEvent::PageDuplicate => self.apply_toolbar_page_duplicate(),
            ToolbarEvent::PageDelete => self.apply_toolbar_page_delete(),
            ToolbarEvent::BoardPrev => self.apply_toolbar_board_prev(),
            ToolbarEvent::BoardNext => self.apply_toolbar_board_next(),
            ToolbarEvent::BoardNew => self.apply_toolbar_board_new(),
            ToolbarEvent::BoardDelete => self.apply_toolbar_board_delete(),
            ToolbarEvent::ToggleBoardPicker => self.apply_toolbar_toggle_board_picker(),
            ToolbarEvent::EnterTextMode => self.apply_toolbar_enter_text_mode(),
            ToolbarEvent::EnterStickyNoteMode => self.apply_toolbar_enter_sticky_note_mode(),
            ToolbarEvent::ToggleAllHighlight(enable) => {
                self.apply_toolbar_toggle_all_highlight(enable)
            }
            ToolbarEvent::ToggleFreeze => self.apply_toolbar_toggle_freeze(),
            ToolbarEvent::ZoomIn => self.apply_toolbar_zoom_in(),
            ToolbarEvent::ZoomOut => self.apply_toolbar_zoom_out(),
            ToolbarEvent::ResetZoom => self.apply_toolbar_reset_zoom(),
            ToolbarEvent::ToggleZoomLock => self.apply_toolbar_toggle_zoom_lock(),
            ToolbarEvent::RefreshZoomCapture => self.apply_toolbar_refresh_zoom_capture(),
            ToolbarEvent::ToggleCustomSection(enable) => {
                self.apply_toolbar_toggle_custom_section(enable)
            }
            ToolbarEvent::ToggleDelaySliders(show) => self.apply_toolbar_toggle_delay_sliders(show),
            ToolbarEvent::OpenConfigurator => self.apply_toolbar_open_configurator(),
            ToolbarEvent::OpenConfigFile => self.apply_toolbar_open_config_file(),
            ToolbarEvent::CloseTopToolbar => self.apply_toolbar_close_top_toolbar(),
            ToolbarEvent::CloseSideToolbar => self.apply_toolbar_close_side_toolbar(),
            ToolbarEvent::PinTopToolbar(pin) => self.apply_toolbar_pin_top_toolbar(pin),
            ToolbarEvent::PinSideToolbar(pin) => self.apply_toolbar_pin_side_toolbar(pin),
            ToolbarEvent::ToggleIconMode(use_icons) => {
                self.apply_toolbar_toggle_icon_mode(use_icons)
            }
            ToolbarEvent::ToggleMoreColors(show) => self.apply_toolbar_toggle_more_colors(show),
            ToolbarEvent::ToggleActionsSection(show) => {
                self.apply_toolbar_toggle_actions_section(show)
            }
            ToolbarEvent::ToggleActionsAdvanced(show) => {
                self.apply_toolbar_toggle_actions_advanced(show)
            }
            ToolbarEvent::ToggleZoomActions(show) => self.apply_toolbar_toggle_zoom_actions(show),
            ToolbarEvent::TogglePagesSection(show) => self.apply_toolbar_toggle_pages_section(show),
            ToolbarEvent::ToggleBoardsSection(show) => {
                self.apply_toolbar_toggle_boards_section(show)
            }
            ToolbarEvent::TogglePresets(show) => self.apply_toolbar_toggle_presets(show),
            ToolbarEvent::ToggleStepSection(show) => self.apply_toolbar_toggle_step_section(show),
            ToolbarEvent::ToggleTextControls(show) => self.apply_toolbar_toggle_text_controls(show),
            ToolbarEvent::TogglePresetToasts(show) => self.apply_toolbar_toggle_preset_toasts(show),
            ToolbarEvent::ToggleToolPreview(show) => self.apply_toolbar_toggle_tool_preview(show),
            ToolbarEvent::ToggleStatusBar(show) => self.apply_toolbar_toggle_status_bar(show),
            ToolbarEvent::ToggleDrawer(open) => self.apply_toolbar_toggle_drawer(open),
            ToolbarEvent::SetDrawerTab(tab) => self.apply_toolbar_set_drawer_tab(tab),
            ToolbarEvent::SetToolbarLayoutMode(mode) => self.apply_toolbar_set_layout_mode(mode),
            ToolbarEvent::ToggleShapePicker(open) => self.apply_toolbar_toggle_shape_picker(open),
            ToolbarEvent::ApplyPreset(slot) => self.apply_toolbar_apply_preset(slot),
            ToolbarEvent::SavePreset(slot) => self.apply_toolbar_save_preset(slot),
            ToolbarEvent::ClearPreset(slot) => self.apply_toolbar_clear_preset(slot),
            ToolbarEvent::MoveTopToolbar { .. } | ToolbarEvent::MoveSideToolbar { .. } => false,
        }
    }
}
