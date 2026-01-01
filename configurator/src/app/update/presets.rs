use iced::Command;
use wayscriber::config::PRESET_SLOTS_MAX;

use crate::messages::Message;
use crate::models::{
    ColorMode, NamedColorOption, PresetEraserKindOption, PresetEraserModeOption, PresetTextField,
    PresetToggleField,
};

use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_preset_slot_count_changed(&mut self, count: usize) -> Command<Message> {
        self.status = StatusMessage::idle();
        self.draft.presets.slot_count = count;
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_slot_enabled_changed(
        &mut self,
        slot_index: usize,
        enabled: bool,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
            slot.enabled = enabled;
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_collapse_toggled(&mut self, slot_index: usize) -> Command<Message> {
        if let Some(collapsed) = self.preset_collapsed.get_mut(slot_index.saturating_sub(1)) {
            *collapsed = !*collapsed;
        }
        Command::none()
    }

    pub(super) fn handle_preset_reset_slot(&mut self, slot_index: usize) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let (Some(slot), Some(default_slot)) = (
            self.draft.presets.slot_mut(slot_index),
            self.defaults.presets.slot(slot_index),
        ) {
            let enabled = slot.enabled;
            *slot = default_slot.clone();
            slot.enabled = enabled;
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_duplicate_slot(&mut self, slot_index: usize) -> Command<Message> {
        self.status = StatusMessage::idle();
        let target_index = slot_index + 1;
        if target_index <= PRESET_SLOTS_MAX
            && let Some(source) = self.draft.presets.slot(slot_index).cloned()
            && let Some(target) = self.draft.presets.slot_mut(target_index)
        {
            *target = source;
            target.enabled = true;
            if let Some(collapsed) = self.preset_collapsed.get_mut(target_index - 1) {
                *collapsed = false;
            }
            if self.draft.presets.slot_count < target_index {
                self.draft.presets.slot_count = target_index;
            }
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_tool_changed(
        &mut self,
        slot_index: usize,
        tool: crate::models::ToolOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
            slot.tool = tool;
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_color_mode_changed(
        &mut self,
        slot_index: usize,
        mode: ColorMode,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
            slot.color.mode = mode;
            if matches!(mode, ColorMode::Named) {
                if slot.color.name.trim().is_empty() {
                    slot.color.selected_named = NamedColorOption::Red;
                    slot.color.name = slot.color.selected_named.as_value().to_string();
                } else {
                    slot.color.update_named_from_current();
                }
            }
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_named_color_selected(
        &mut self,
        slot_index: usize,
        option: NamedColorOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
            slot.color.selected_named = option;
            if option != NamedColorOption::Custom {
                slot.color.name = option.as_value().to_string();
            }
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_color_component_changed(
        &mut self,
        slot_index: usize,
        component: usize,
        value: String,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(slot) = self.draft.presets.slot_mut(slot_index)
            && let Some(entry) = slot.color.rgb.get_mut(component)
        {
            *entry = value;
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_text_changed(
        &mut self,
        slot_index: usize,
        field: PresetTextField,
        value: String,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
            match field {
                PresetTextField::Name => {
                    slot.name = value;
                }
                PresetTextField::ColorName => {
                    slot.color.name = value;
                    slot.color.update_named_from_current();
                }
                PresetTextField::Size => slot.size = value,
                PresetTextField::MarkerOpacity => slot.marker_opacity = value,
                PresetTextField::FontSize => slot.font_size = value,
                PresetTextField::ArrowLength => slot.arrow_length = value,
                PresetTextField::ArrowAngle => slot.arrow_angle = value,
            }
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_toggle_option_changed(
        &mut self,
        slot_index: usize,
        field: PresetToggleField,
        value: crate::models::OverrideOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
            match field {
                PresetToggleField::FillEnabled => slot.fill_enabled = value,
                PresetToggleField::TextBackgroundEnabled => {
                    slot.text_background_enabled = value;
                }
                PresetToggleField::ArrowHeadAtEnd => slot.arrow_head_at_end = value,
                PresetToggleField::ShowStatusBar => slot.show_status_bar = value,
            }
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_eraser_kind_changed(
        &mut self,
        slot_index: usize,
        value: PresetEraserKindOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
            slot.eraser_kind = value;
        }
        self.refresh_dirty_flag();
        Command::none()
    }

    pub(super) fn handle_preset_eraser_mode_changed(
        &mut self,
        slot_index: usize,
        value: PresetEraserModeOption,
    ) -> Command<Message> {
        self.status = StatusMessage::idle();
        if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
            slot.eraser_mode = value;
        }
        self.refresh_dirty_flag();
        Command::none()
    }
}
