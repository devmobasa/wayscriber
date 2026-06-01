mod boards;
mod color_picker;
mod config;
mod daemon;
mod fields;
mod presets;
mod render_profiles;
mod tabs;

use iced::Task;

use crate::messages::Message;

use super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(crate) fn update_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ConfigLoaded(result) => self.handle_config_loaded(result),
            Message::ReloadRequested => self.handle_reload_requested(),
            Message::ResetToDefaultsRequested => self.handle_reset_to_defaults_requested(),
            Message::ResetToDefaultsConfirmed => self.handle_reset_to_defaults_confirmed(),
            Message::ResetToDefaultsCanceled => self.handle_reset_to_defaults_canceled(),
            Message::SaveRequested => self.handle_save_requested(),
            Message::ConfigSaved(result) => self.handle_config_saved(result),
            Message::DaemonStatusLoaded(request_id, result) => {
                self.handle_daemon_status_loaded(request_id, result)
            }
            Message::DaemonShortcutInputChanged(value) => {
                self.handle_daemon_shortcut_input_changed(value)
            }
            Message::DaemonActionRequested(action) => self.handle_daemon_action_requested(action),
            Message::DaemonActionCompleted(result) => self.handle_daemon_action_completed(result),
            Message::TabSelected(tab) => self.handle_tab_selected(tab),
            Message::UiTabSelected(tab) => self.handle_ui_tab_selected(tab),
            Message::KeybindingsTabSelected(tab) => self.handle_keybindings_tab_selected(tab),
            Message::ToggleChanged(field, value) => self.handle_toggle_changed(field, value),
            Message::TextChanged(field, value) => self.handle_text_changed(field, value),
            Message::TripletChanged(field, index, value) => {
                self.handle_triplet_changed(field, index, value)
            }
            Message::QuadChanged(field, index, value) => {
                self.handle_quad_changed(field, index, value)
            }
            Message::ColorPickerToggled(id) => self.handle_color_picker_toggled(id),
            Message::ColorPickerAdvancedToggled(id, value) => {
                self.handle_color_picker_advanced_toggled(id, value)
            }
            Message::ColorPickerHexChanged(id, value) => {
                self.handle_color_picker_hex_changed(id, value)
            }
            Message::ColorPickerChanged(id, value) => self.handle_color_picker_changed(id, value),
            Message::ColorModeChanged(mode) => self.handle_color_mode_changed(mode),
            Message::NamedColorSelected(option) => self.handle_named_color_selected(option),
            Message::EraserModeChanged(option) => self.handle_eraser_mode_changed(option),
            Message::DrawingDragMappingSectionToggled(button) => {
                self.handle_drawing_drag_mapping_section_toggled(button)
            }
            Message::DrawingMouseDragToolChanged(button, field, option) => {
                self.handle_drawing_mouse_drag_tool_changed(button, field, option)
            }
            Message::DrawingMouseDragColorChanged(button, field, option) => {
                self.handle_drawing_mouse_drag_color_changed(button, field, option)
            }
            Message::StatusPositionChanged(option) => self.handle_status_position_changed(option),
            Message::ToolbarLayoutModeChanged(option) => {
                self.handle_toolbar_layout_mode_changed(option)
            }
            Message::ToolbarOverrideModeChanged(option) => {
                self.handle_toolbar_override_mode_changed(option)
            }
            Message::ToolbarOverrideChanged(field, option) => {
                self.handle_toolbar_override_changed(field, option)
            }
            Message::BoardsAddItem => self.handle_boards_add_item(),
            Message::BoardsRemoveItem(index) => self.handle_boards_remove_item(index),
            Message::BoardsMoveItemUp(index) => self.handle_boards_move_item(index, true),
            Message::BoardsMoveItemDown(index) => self.handle_boards_move_item(index, false),
            Message::BoardsDuplicateItem(index) => self.handle_boards_duplicate_item(index),
            Message::BoardsCollapseToggled(index) => self.handle_boards_collapse_toggled(index),
            Message::BoardsDefaultChanged(value) => self.handle_boards_default_changed(value),
            Message::BoardsItemTextChanged(index, field, value) => {
                self.handle_boards_item_text_changed(index, field, value)
            }
            Message::BoardsBackgroundKindChanged(index, value) => {
                self.handle_boards_background_kind_changed(index, value)
            }
            Message::BoardsBackgroundColorChanged(index, component, value) => {
                self.handle_boards_background_color_changed(index, component, value)
            }
            Message::BoardsDefaultPenEnabledChanged(index, value) => {
                self.handle_boards_default_pen_enabled_changed(index, value)
            }
            Message::BoardsDefaultPenColorChanged(index, component, value) => {
                self.handle_boards_default_pen_color_changed(index, component, value)
            }
            Message::BoardsItemToggleChanged(index, field, value) => {
                self.handle_boards_item_toggle_changed(index, field, value)
            }
            Message::RenderProfileAdd => self.handle_render_profile_add(),
            Message::RenderProfileRemove(index) => self.handle_render_profile_remove(index),
            Message::RenderProfileDuplicate(index) => self.handle_render_profile_duplicate(index),
            Message::RenderProfileTextChanged(index, field, value) => {
                self.handle_render_profile_text_changed(index, field, value)
            }
            Message::RenderProfileActiveChanged(value) => {
                self.handle_render_profile_active_changed(value)
            }
            Message::RenderProfileExportChanged(value) => {
                self.handle_render_profile_export_changed(value)
            }
            Message::RenderProfileExportProfileChanged(value) => {
                self.handle_render_profile_export_profile_changed(value)
            }
            Message::RenderProfileApplyCanvasChanged(value) => {
                self.handle_render_profile_apply_canvas_changed(value)
            }
            Message::RenderProfileApplyUiChanged(value) => {
                self.handle_render_profile_apply_ui_changed(value)
            }
            Message::RenderProfileMappingAdd(index) => {
                self.handle_render_profile_mapping_add(index)
            }
            Message::RenderProfileMappingRemove(profile, mapping) => {
                self.handle_render_profile_mapping_remove(profile, mapping)
            }
            Message::RenderProfileMappingColorChanged(profile, mapping, side, value) => {
                self.handle_render_profile_mapping_color_changed(profile, mapping, side, value)
            }
            Message::SessionStorageModeChanged(option) => {
                self.handle_session_storage_mode_changed(option)
            }
            Message::SessionCompressionChanged(option) => {
                self.handle_session_compression_changed(option)
            }
            Message::PresenterToolBehaviorChanged(option) => {
                self.handle_presenter_tool_behavior_changed(option)
            }
            Message::ExportPdfPageSizeChanged(option) => {
                self.handle_export_pdf_page_size_changed(option)
            }
            Message::ExportPdfOrientationChanged(option) => {
                self.handle_export_pdf_orientation_changed(option)
            }
            Message::ExportPdfFitChanged(option) => self.handle_export_pdf_fit_changed(option),
            Message::ExportPdfTransparentBackgroundChanged(option) => {
                self.handle_export_pdf_transparent_background_changed(option)
            }
            Message::ExportPdfLabelPositionChanged(option) => {
                self.handle_export_pdf_label_position_changed(option)
            }
            Message::ExportPdfLabelContentChanged(option) => {
                self.handle_export_pdf_label_content_changed(option)
            }
            Message::BufferCountChanged(count) => self.handle_buffer_count_changed(count),
            Message::KeybindingChanged(field, value) => {
                self.handle_keybinding_changed(field, value)
            }
            Message::FontStyleOptionSelected(option) => {
                self.handle_font_style_option_selected(option)
            }
            Message::FontWeightOptionSelected(option) => {
                self.handle_font_weight_option_selected(option)
            }
            #[cfg(feature = "tablet-input")]
            Message::TabletPressureEditModeChanged(option) => {
                self.handle_tablet_pressure_edit_mode_changed(option)
            }
            #[cfg(feature = "tablet-input")]
            Message::TabletPressureEntryModeChanged(option) => {
                self.handle_tablet_pressure_entry_mode_changed(option)
            }
            Message::PresetSlotCountChanged(count) => self.handle_preset_slot_count_changed(count),
            Message::PresetSlotEnabledChanged(slot_index, enabled) => {
                self.handle_preset_slot_enabled_changed(slot_index, enabled)
            }
            Message::PresetCollapseToggled(slot_index) => {
                self.handle_preset_collapse_toggled(slot_index)
            }
            Message::PresetResetSlot(slot_index) => self.handle_preset_reset_slot(slot_index),
            Message::PresetDuplicateSlot(slot_index) => {
                self.handle_preset_duplicate_slot(slot_index)
            }
            Message::PresetToolChanged(slot_index, tool) => {
                self.handle_preset_tool_changed(slot_index, tool)
            }
            Message::PresetColorModeChanged(slot_index, mode) => {
                self.handle_preset_color_mode_changed(slot_index, mode)
            }
            Message::PresetNamedColorSelected(slot_index, option) => {
                self.handle_preset_named_color_selected(slot_index, option)
            }
            Message::PresetColorComponentChanged(slot_index, component, value) => {
                self.handle_preset_color_component_changed(slot_index, component, value)
            }
            Message::PresetTextChanged(slot_index, field, value) => {
                self.handle_preset_text_changed(slot_index, field, value)
            }
            Message::PresetToggleOptionChanged(slot_index, field, value) => {
                self.handle_preset_toggle_option_changed(slot_index, field, value)
            }
            Message::PresetEraserKindChanged(slot_index, value) => {
                self.handle_preset_eraser_kind_changed(slot_index, value)
            }
            Message::PresetEraserModeChanged(slot_index, value) => {
                self.handle_preset_eraser_mode_changed(slot_index, value)
            }
        }
    }
}
