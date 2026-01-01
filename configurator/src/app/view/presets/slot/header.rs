use iced::theme;
use iced::widget::{Row, Space, button, checkbox, row, text};
use iced::{Element, Length};
use wayscriber::config::PRESET_SLOTS_MAX;

use crate::messages::Message;

use super::super::super::widgets::{DEFAULT_LABEL_GAP, bool_label, default_value_text};
use crate::app::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn preset_slot_enabled_row(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };

        let enabled_row = row![
            checkbox("Enabled", slot.enabled)
                .on_toggle(move |val| Message::PresetSlotEnabledChanged(slot_index, val)),
            default_value_text(
                bool_label(default_slot.enabled),
                slot.enabled != default_slot.enabled
            ),
        ]
        .spacing(DEFAULT_LABEL_GAP)
        .align_items(iced::Alignment::Center);

        enabled_row.into()
    }

    pub(super) fn preset_slot_header(&self, slot_index: usize) -> Element<'_, Message> {
        let is_collapsed = self
            .preset_collapsed
            .get(slot_index.saturating_sub(1))
            .copied()
            .unwrap_or(false);
        let collapse_label = if is_collapsed { "Expand" } else { "Collapse" };
        let collapse_button = button(collapse_label)
            .style(theme::Button::Secondary)
            .on_press(Message::PresetCollapseToggled(slot_index));
        let reset_button = button("Reset")
            .style(theme::Button::Secondary)
            .on_press(Message::PresetResetSlot(slot_index));
        let mut duplicate_button = button("Duplicate").style(theme::Button::Secondary);
        if slot_index < PRESET_SLOTS_MAX {
            duplicate_button = duplicate_button.on_press(Message::PresetDuplicateSlot(slot_index));
        }

        let slot_header = Row::new()
            .spacing(8)
            .align_items(iced::Alignment::Center)
            .push(text(format!("Slot {slot_index} settings")).size(18))
            .push(Space::new(Length::Fill, Length::Shrink))
            .push(collapse_button)
            .push(reset_button)
            .push(duplicate_button);

        slot_header.into()
    }
}
