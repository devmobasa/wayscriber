mod config;
mod fields;
mod presets;
mod tabs;

use iced::Command;

use crate::messages::Message;

use super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn update_message(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::ConfigLoaded(result) => self.handle_config_loaded(result),
            Message::ReloadRequested => self.handle_reload_requested(),
            Message::ResetToDefaults => self.handle_reset_to_defaults(),
            Message::SaveRequested => self.handle_save_requested(),
            Message::ConfigSaved(result) => self.handle_config_saved(result),
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
            Message::ColorModeChanged(mode) => self.handle_color_mode_changed(mode),
            Message::NamedColorSelected(option) => self.handle_named_color_selected(option),
            Message::EraserModeChanged(option) => self.handle_eraser_mode_changed(option),
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
            Message::BoardModeChanged(option) => self.handle_board_mode_changed(option),
            Message::SessionStorageModeChanged(option) => {
                self.handle_session_storage_mode_changed(option)
            }
            Message::SessionCompressionChanged(option) => {
                self.handle_session_compression_changed(option)
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
