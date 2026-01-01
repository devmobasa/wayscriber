use iced::widget::{Column, pick_list, scrollable, text};
use iced::{Element, Length};
use wayscriber::config::{PRESET_SLOTS_MAX, PRESET_SLOTS_MIN};

use crate::messages::Message;

use super::super::state::ConfiguratorApp;
use super::widgets::{SMALL_PICKER_WIDTH, labeled_control};

mod slot;

impl ConfiguratorApp {
    pub(super) fn presets_tab(&self) -> Element<'_, Message> {
        let slot_counts: Vec<usize> = (PRESET_SLOTS_MIN..=PRESET_SLOTS_MAX).collect();
        let slot_picker = pick_list(
            slot_counts,
            Some(self.draft.presets.slot_count),
            Message::PresetSlotCountChanged,
        )
        .width(Length::Fixed(SMALL_PICKER_WIDTH));

        let slot_count_control = labeled_control(
            "Visible slots",
            slot_picker.into(),
            self.defaults.presets.slot_count.to_string(),
            self.draft.presets.slot_count != self.defaults.presets.slot_count,
        );

        let mut column = Column::new()
            .spacing(12)
            .push(text("Preset Slots").size(20))
            .push(slot_count_control);

        let slot_limit = self
            .draft
            .presets
            .slot_count
            .clamp(PRESET_SLOTS_MIN, PRESET_SLOTS_MAX);
        for slot_index in 1..=slot_limit {
            column = column.push(self.preset_slot_section(slot_index));
        }

        scrollable(column).into()
    }
}
