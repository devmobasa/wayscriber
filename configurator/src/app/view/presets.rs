use iced::theme;
use iced::widget::{
    Column, Row, Space, button, checkbox, column, container, pick_list, row, scrollable, text,
    text_input,
};
use iced::{Element, Length};
use wayscriber::config::{PRESET_SLOTS_MAX, PRESET_SLOTS_MIN};

use crate::messages::Message;
use crate::models::{
    ColorMode, NamedColorOption, PresetEraserKindOption, PresetEraserModeOption, PresetTextField,
    PresetToggleField, ToolOption,
};

use super::super::state::ConfiguratorApp;
use super::widgets::{
    COLOR_PICKER_WIDTH, DEFAULT_LABEL_GAP, SMALL_PICKER_WIDTH, bool_label, color_preview_labeled,
    default_value_text, labeled_control, preset_input, preset_override_control,
};

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

    fn preset_slot_section(&self, slot_index: usize) -> Element<'_, Message> {
        let Some(slot) = self.draft.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        let Some(default_slot) = self.defaults.presets.slot(slot_index) else {
            return Space::new(Length::Shrink, Length::Shrink).into();
        };
        if slot_index > self.draft.presets.slot_count {
            return Space::new(Length::Shrink, Length::Shrink).into();
        }

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

        let mut section = Column::new().spacing(8).push(slot_header).push(enabled_row);

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

        let tool_picker = pick_list(ToolOption::list(), Some(slot.tool), move |opt| {
            Message::PresetToolChanged(slot_index, opt)
        })
        .width(Length::Fill);

        let tool_row = row![
            preset_input(
                "Label",
                &slot.name,
                &default_slot.name,
                slot_index,
                PresetTextField::Name,
                true,
            ),
            labeled_control(
                "Tool",
                tool_picker.into(),
                default_slot.tool.label(),
                slot.tool != default_slot.tool,
            )
        ]
        .spacing(12);

        let color_mode_picker = Row::new()
            .spacing(12)
            .push(
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
            )
            .push(
                button("RGB Color")
                    .style(if slot.color.mode == ColorMode::Rgb {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::PresetColorModeChanged(slot_index, ColorMode::Rgb)),
            );

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
                    slot.color != default_slot.color,
                ),
            ]
            .spacing(DEFAULT_LABEL_GAP)
            .align_items(iced::Alignment::Center),
            color_mode_picker,
            color_section
        ]
        .spacing(8);

        let size_row = row![
            preset_input(
                "Size (px)",
                &slot.size,
                &default_slot.size,
                slot_index,
                PresetTextField::Size,
                false,
            ),
            preset_input(
                "Marker opacity (0.05-0.9)",
                &slot.marker_opacity,
                &default_slot.marker_opacity,
                slot_index,
                PresetTextField::MarkerOpacity,
                true,
            )
        ]
        .spacing(12);

        let eraser_row = row![
            labeled_control(
                "Eraser kind",
                pick_list(
                    PresetEraserKindOption::list(),
                    Some(slot.eraser_kind),
                    move |opt| Message::PresetEraserKindChanged(slot_index, opt),
                )
                .width(Length::Fill)
                .into(),
                default_slot.eraser_kind.label(),
                slot.eraser_kind != default_slot.eraser_kind,
            ),
            labeled_control(
                "Eraser mode",
                pick_list(
                    PresetEraserModeOption::list(),
                    Some(slot.eraser_mode),
                    move |opt| Message::PresetEraserModeChanged(slot_index, opt),
                )
                .width(Length::Fill)
                .into(),
                default_slot.eraser_mode.label(),
                slot.eraser_mode != default_slot.eraser_mode,
            )
        ]
        .spacing(12);

        let fill_row = row![
            preset_override_control(
                "Fill enabled",
                slot.fill_enabled,
                default_slot.fill_enabled,
                slot_index,
                PresetToggleField::FillEnabled,
            ),
            preset_override_control(
                "Text background",
                slot.text_background_enabled,
                default_slot.text_background_enabled,
                slot_index,
                PresetToggleField::TextBackgroundEnabled,
            )
        ]
        .spacing(12);

        let font_row = row![
            preset_input(
                "Font size (pt)",
                &slot.font_size,
                &default_slot.font_size,
                slot_index,
                PresetTextField::FontSize,
                true,
            ),
            preset_input(
                "Arrow length (px)",
                &slot.arrow_length,
                &default_slot.arrow_length,
                slot_index,
                PresetTextField::ArrowLength,
                true,
            )
        ]
        .spacing(12);

        let arrow_row = row![
            preset_input(
                "Arrow angle (deg)",
                &slot.arrow_angle,
                &default_slot.arrow_angle,
                slot_index,
                PresetTextField::ArrowAngle,
                true,
            ),
            preset_override_control(
                "Arrow head at end",
                slot.arrow_head_at_end,
                default_slot.arrow_head_at_end,
                slot_index,
                PresetToggleField::ArrowHeadAtEnd,
            )
        ]
        .spacing(12);

        let status_row = row![preset_override_control(
            "Show status bar",
            slot.show_status_bar,
            default_slot.show_status_bar,
            slot_index,
            PresetToggleField::ShowStatusBar,
        )];

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
