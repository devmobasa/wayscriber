use iced::theme;
use iced::widget::{Column, Space, button, column, pick_list, row, text, text_input};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{ColorMode, NamedColorOption, PresetTextField};

use super::super::super::widgets::{
    COLOR_PICKER_WIDTH, DEFAULT_LABEL_GAP, color_preview_labeled, default_value_text,
};
use crate::app::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn preset_slot_color_block(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };

        let color_mode_picker = row![
            button("Named Color")
                .style(if slot.color.mode == ColorMode::Named {
                    theme::Button::Primary
                } else {
                    theme::Button::Secondary
                })
                .on_press(Message::PresetColorModeChanged(
                    slot_index,
                    ColorMode::Named,
                )),
            button("RGB Color")
                .style(if slot.color.mode == ColorMode::Rgb {
                    theme::Button::Primary
                } else {
                    theme::Button::Secondary
                })
                .on_press(Message::PresetColorModeChanged(slot_index, ColorMode::Rgb)),
        ]
        .spacing(12);

        let color_section: Element<'_, Message> = match slot.color.mode {
            ColorMode::Named => {
                let picker = pick_list(
                    NamedColorOption::list(),
                    Some(slot.color.selected_named),
                    move |opt| Message::PresetNamedColorSelected(slot_index, opt),
                )
                .width(Length::Fixed(COLOR_PICKER_WIDTH));

                let picker_row = row![picker, color_preview_labeled(slot.color.preview_color()),]
                    .spacing(8)
                    .align_items(iced::Alignment::Center);

                let mut column = Column::new().spacing(8).push(picker_row);

                if slot.color.selected_named_is_custom() {
                    column = column.push(
                        text_input("Custom color name", &slot.color.name)
                            .on_input(move |value| {
                                Message::PresetTextChanged(
                                    slot_index,
                                    PresetTextField::ColorName,
                                    value,
                                )
                            })
                            .width(Length::Fill),
                    );

                    if slot.color.preview_color().is_none() && !slot.color.name.trim().is_empty() {
                        column = column.push(
                            text("Unknown color name")
                                .size(12)
                                .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.6, 0.6))),
                        );
                    }
                }

                column.into()
            }
            ColorMode::Rgb => {
                let rgb_inputs = row![
                    text_input("R (0-255)", &slot.color.rgb[0]).on_input(move |value| {
                        Message::PresetColorComponentChanged(slot_index, 0, value)
                    }),
                    text_input("G (0-255)", &slot.color.rgb[1]).on_input(move |value| {
                        Message::PresetColorComponentChanged(slot_index, 1, value)
                    }),
                    text_input("B (0-255)", &slot.color.rgb[2]).on_input(move |value| {
                        Message::PresetColorComponentChanged(slot_index, 2, value)
                    }),
                    color_preview_labeled(slot.color.preview_color()),
                ]
                .spacing(8)
                .align_items(iced::Alignment::Center);

                let mut column = Column::new().spacing(8).push(rgb_inputs);

                if slot.color.preview_color().is_none()
                    && slot.color.rgb.iter().any(|value| !value.trim().is_empty())
                {
                    column = column.push(
                        text("RGB values must be between 0 and 255")
                            .size(12)
                            .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.6, 0.6))),
                    );
                }

                column.into()
            }
        };

        let color_block = column![
            row![
                text("Color").size(14),
                default_value_text(
                    default_slot.color.summary(),
                    slot.color != default_slot.color
                ),
            ]
            .spacing(DEFAULT_LABEL_GAP)
            .align_items(iced::Alignment::Center),
            color_mode_picker,
            color_section
        ]
        .spacing(8);

        color_block.into()
    }
}
