mod color;
mod header;
mod rows;

use iced::theme;
use iced::widget::{Column, Space, container, text};
use iced::{Element, Length};

use crate::messages::Message;

use crate::app::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn preset_slot_section(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        if self.defaults.presets.slot(slot_index).is_none() {
            return Space::new(Length::Shrink, Length::Shrink).into();
        }
        if slot_index > self.draft.presets.slot_count {
            return Space::new(Length::Shrink, Length::Shrink).into();
        }

        let enabled_row = self.preset_slot_enabled_row(slot_index);
        let slot_header = self.preset_slot_header(slot_index);

        let mut section = Column::new().spacing(8).push(slot_header).push(enabled_row);

        let is_collapsed = self
            .preset_collapsed
            .get(slot_index.saturating_sub(1))
            .copied()
            .unwrap_or(false);
        if is_collapsed {
            return container(section)
                .padding(12)
                .style(theme::Container::Box)
                .into();
        }

        if !slot.enabled {
            section = section.push(
                text("Slot disabled. Enable to configure.")
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );
            return container(section)
                .padding(12)
                .style(theme::Container::Box)
                .into();
        }

        let tool_row = self.preset_slot_tool_row(slot_index);
        let color_block = self.preset_slot_color_block(slot_index);
        let size_row = self.preset_slot_size_row(slot_index);
        let eraser_row = self.preset_slot_eraser_row(slot_index);
        let fill_row = self.preset_slot_fill_row(slot_index);
        let font_row = self.preset_slot_font_row(slot_index);
        let arrow_row = self.preset_slot_arrow_row(slot_index);
        let status_row = self.preset_slot_status_row(slot_index);

        section = section
            .push(tool_row)
            .push(color_block)
            .push(size_row)
            .push(eraser_row)
            .push(fill_row)
            .push(font_row)
            .push(arrow_row)
            .push(status_row);

        container(section)
            .padding(12)
            .style(theme::Container::Box)
            .into()
    }
}
