use iced::Command;
use wayscriber::config::PRESET_SLOTS_MAX;

use crate::messages::Message;
use crate::models::{
    ColorMode, ConfigDraft, FontStyleOption, FontWeightOption, NamedColorOption, PresetTextField,
    PresetToggleField,
};

use super::io::{load_config_from_disk, load_config_mtime, save_config_to_disk};
use super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn update_message(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::ConfigLoaded(result) => {
                self.is_loading = false;
                match result {
                    Ok(config) => {
                        let draft = ConfigDraft::from_config(config.as_ref());
                        self.draft = draft.clone();
                        self.baseline = draft;
                        self.base_config = config.clone();
                        self.override_mode = self.draft.ui_toolbar_layout_mode;
                        self.config_mtime = load_config_mtime(&self.config_path);
                        self.is_dirty = false;
                        self.status = StatusMessage::success("Configuration loaded from disk.");
                    }
                    Err(err) => {
                        self.status =
                            StatusMessage::error(format!("Failed to load config from disk: {err}"));
                    }
                }
            }
            Message::ReloadRequested => {
                if !self.is_loading && !self.is_saving {
                    self.is_loading = true;
                    self.status = StatusMessage::info("Reloading configuration...");
                    return Command::perform(load_config_from_disk(), Message::ConfigLoaded);
                }
            }
            Message::ResetToDefaults => {
                if !self.is_loading {
                    self.draft = self.defaults.clone();
                    self.override_mode = self.draft.ui_toolbar_layout_mode;
                    self.status = StatusMessage::info("Loaded default configuration (not saved).");
                    self.refresh_dirty_flag();
                }
            }
            Message::SaveRequested => {
                if self.is_saving {
                    return Command::none();
                }
                if self.config_changed_on_disk() {
                    self.status = StatusMessage::error(
                        "Configuration changed on disk. Reload before saving.",
                    );
                    return Command::none();
                }

                match self.draft.to_config(self.base_config.as_ref()) {
                    Ok(mut config) => {
                        config.validate_and_clamp();
                        self.is_saving = true;
                        self.status = StatusMessage::info("Saving configuration...");
                        return Command::perform(save_config_to_disk(config), Message::ConfigSaved);
                    }
                    Err(errors) => {
                        let message = errors
                            .into_iter()
                            .map(|err| format!("{}: {}", err.field, err.message))
                            .collect::<Vec<_>>()
                            .join("\n");
                        self.status = StatusMessage::error(format!(
                            "Cannot save due to validation errors:\n{message}"
                        ));
                    }
                }
            }
            Message::ConfigSaved(result) => {
                self.is_saving = false;
                match result {
                    Ok((backup, saved_config)) => {
                        let draft = ConfigDraft::from_config(saved_config.as_ref());
                        self.last_backup_path = backup.clone();
                        self.draft = draft.clone();
                        self.baseline = draft;
                        self.base_config = saved_config.clone();
                        self.config_mtime = load_config_mtime(&self.config_path);
                        self.is_dirty = false;
                        let mut msg = "Configuration saved successfully.".to_string();
                        if let Some(path) = backup {
                            msg.push_str(&format!("\nBackup created at {}", path.display()));
                        }
                        self.status = StatusMessage::success(msg);
                    }
                    Err(err) => {
                        self.status =
                            StatusMessage::error(format!("Failed to save configuration: {err}"));
                    }
                }
            }
            Message::TabSelected(tab) => {
                self.active_tab = tab;
            }
            Message::UiTabSelected(tab) => {
                self.active_ui_tab = tab;
            }
            Message::KeybindingsTabSelected(tab) => {
                self.active_keybindings_tab = tab;
            }
            Message::ToggleChanged(field, value) => {
                self.status = StatusMessage::idle();
                self.draft.set_toggle(field, value);
                self.refresh_dirty_flag();
            }
            Message::TextChanged(field, value) => {
                self.status = StatusMessage::idle();
                self.draft.set_text(field, value);
                self.refresh_dirty_flag();
            }
            Message::TripletChanged(field, index, value) => {
                self.status = StatusMessage::idle();
                self.draft.set_triplet(field, index, value);
                self.refresh_dirty_flag();
            }
            Message::QuadChanged(field, index, value) => {
                self.status = StatusMessage::idle();
                self.draft.set_quad(field, index, value);
                self.refresh_dirty_flag();
            }
            Message::ColorModeChanged(mode) => {
                self.status = StatusMessage::idle();
                self.draft.drawing_color.mode = mode;
                if matches!(mode, ColorMode::Named) {
                    if self.draft.drawing_color.name.trim().is_empty() {
                        self.draft.drawing_color.selected_named = NamedColorOption::Red;
                        self.draft.drawing_color.name = self
                            .draft
                            .drawing_color
                            .selected_named
                            .as_value()
                            .to_string();
                    } else {
                        self.draft.drawing_color.update_named_from_current();
                    }
                }
                self.refresh_dirty_flag();
            }
            Message::NamedColorSelected(option) => {
                self.status = StatusMessage::idle();
                self.draft.drawing_color.selected_named = option;
                if option != NamedColorOption::Custom {
                    self.draft.drawing_color.name = option.as_value().to_string();
                }
                self.refresh_dirty_flag();
            }
            Message::EraserModeChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.drawing_default_eraser_mode = option;
                self.refresh_dirty_flag();
            }
            Message::StatusPositionChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.ui_status_position = option;
                self.refresh_dirty_flag();
            }
            Message::ToolbarLayoutModeChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.apply_toolbar_layout_mode(option);
                self.refresh_dirty_flag();
            }
            Message::ToolbarOverrideModeChanged(option) => {
                self.override_mode = option;
            }
            Message::ToolbarOverrideChanged(field, option) => {
                self.status = StatusMessage::idle();
                self.draft
                    .set_toolbar_override(self.override_mode, field, option);
                self.refresh_dirty_flag();
            }
            Message::BoardModeChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.board_default_mode = option;
                self.refresh_dirty_flag();
            }
            Message::SessionStorageModeChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.session_storage_mode = option;
                self.refresh_dirty_flag();
            }
            Message::SessionCompressionChanged(option) => {
                self.status = StatusMessage::idle();
                self.draft.session_compression = option;
                self.refresh_dirty_flag();
            }
            Message::BufferCountChanged(count) => {
                self.status = StatusMessage::idle();
                self.draft.performance_buffer_count = count;
                self.refresh_dirty_flag();
            }
            Message::KeybindingChanged(field, value) => {
                self.status = StatusMessage::idle();
                self.draft.keybindings.set(field, value);
                self.refresh_dirty_flag();
            }
            Message::FontStyleOptionSelected(option) => {
                self.status = StatusMessage::idle();
                self.draft.drawing_font_style_option = option;
                if option != FontStyleOption::Custom {
                    self.draft.drawing_font_style = option.canonical_value().to_string();
                }
                self.refresh_dirty_flag();
            }
            Message::FontWeightOptionSelected(option) => {
                self.status = StatusMessage::idle();
                self.draft.drawing_font_weight_option = option;
                if option != FontWeightOption::Custom {
                    self.draft.drawing_font_weight = option.canonical_value().to_string();
                }
                self.refresh_dirty_flag();
            }
            Message::PresetSlotCountChanged(count) => {
                self.status = StatusMessage::idle();
                self.draft.presets.slot_count = count;
                self.refresh_dirty_flag();
            }
            Message::PresetSlotEnabledChanged(slot_index, enabled) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.enabled = enabled;
                }
                self.refresh_dirty_flag();
            }
            Message::PresetCollapseToggled(slot_index) => {
                if let Some(collapsed) = self.preset_collapsed.get_mut(slot_index.saturating_sub(1))
                {
                    *collapsed = !*collapsed;
                }
            }
            Message::PresetResetSlot(slot_index) => {
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
            }
            Message::PresetDuplicateSlot(slot_index) => {
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
            }
            Message::PresetToolChanged(slot_index, tool) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.tool = tool;
                }
                self.refresh_dirty_flag();
            }
            Message::PresetColorModeChanged(slot_index, mode) => {
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
            }
            Message::PresetNamedColorSelected(slot_index, option) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.color.selected_named = option;
                    if option != NamedColorOption::Custom {
                        slot.color.name = option.as_value().to_string();
                    }
                }
                self.refresh_dirty_flag();
            }
            Message::PresetColorComponentChanged(slot_index, component, value) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index)
                    && let Some(entry) = slot.color.rgb.get_mut(component)
                {
                    *entry = value;
                }
                self.refresh_dirty_flag();
            }
            Message::PresetTextChanged(slot_index, field, value) => {
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
            }
            Message::PresetToggleOptionChanged(slot_index, field, value) => {
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
            }
            Message::PresetEraserKindChanged(slot_index, value) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.eraser_kind = value;
                }
                self.refresh_dirty_flag();
            }
            Message::PresetEraserModeChanged(slot_index, value) => {
                self.status = StatusMessage::idle();
                if let Some(slot) = self.draft.presets.slot_mut(slot_index) {
                    slot.eraser_mode = value;
                }
                self.refresh_dirty_flag();
            }
        }

        Command::none()
    }
}
