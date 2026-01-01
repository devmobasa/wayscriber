use iced::theme;
use iced::widget::{Column, Row, button, column, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length};

use crate::messages::Message;
use crate::models::{ColorMode, NamedColorOption, TextField, TripletField};

use super::super::super::state::ConfiguratorApp;
use super::super::widgets::{
    COLOR_PICKER_WIDTH, DEFAULT_LABEL_GAP, color_preview_labeled, default_value_text,
};

pub(super) fn drawing_color_block(app: &ConfiguratorApp) -> Element<'_, Message> {
    let color_mode_picker = Row::new()
        .spacing(12)
        .push(
            button("Named Color")
                .style(if app.draft.drawing_color.mode == ColorMode::Named {
                    theme::Button::Primary
                } else {
                    theme::Button::Secondary
                })
                .on_press(Message::ColorModeChanged(ColorMode::Named)),
        )
        .push(
            button("RGB Color")
                .style(if app.draft.drawing_color.mode == ColorMode::Rgb {
                    theme::Button::Primary
                } else {
                    theme::Button::Secondary
                })
                .on_press(Message::ColorModeChanged(ColorMode::Rgb)),
        );

    let color_section: Element<'_, Message> = match app.draft.drawing_color.mode {
        ColorMode::Named => named_color_section(app),
        ColorMode::Rgb => rgb_color_section(app),
    };

    column![
        row![
            text("Pen color").size(14),
            default_value_text(
                app.defaults.drawing_color.summary(),
                app.draft.drawing_color != app.defaults.drawing_color,
            ),
        ]
        .spacing(DEFAULT_LABEL_GAP)
        .align_items(Alignment::Center),
        color_mode_picker,
        color_section
    ]
    .spacing(8)
    .into()
}

fn named_color_section(app: &ConfiguratorApp) -> Element<'_, Message> {
    let picker = pick_list(
        NamedColorOption::list(),
        Some(app.draft.drawing_color.selected_named),
        Message::NamedColorSelected,
    )
    .width(Length::Fixed(COLOR_PICKER_WIDTH));

    let picker_row = row![
        picker,
        color_preview_labeled(app.draft.drawing_color.preview_color()),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let mut column = Column::new().spacing(8).push(picker_row);

    if app.draft.drawing_color.selected_named_is_custom() {
        column = column.push(
            text_input("Custom color name", &app.draft.drawing_color.name)
                .on_input(|value| Message::TextChanged(TextField::DrawingColorName, value))
                .width(Length::Fill),
        );

        if app.draft.drawing_color.preview_color().is_none()
            && !app.draft.drawing_color.name.trim().is_empty()
        {
            column = column.push(
                text("Unknown color name")
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.6, 0.6))),
            );
        }
    }

    column.into()
}

fn rgb_color_section(app: &ConfiguratorApp) -> Element<'_, Message> {
    let rgb_inputs = row![
        text_input("R (0-255)", &app.draft.drawing_color.rgb[0])
            .on_input(|value| { Message::TripletChanged(TripletField::DrawingColorRgb, 0, value) }),
        text_input("G (0-255)", &app.draft.drawing_color.rgb[1])
            .on_input(|value| { Message::TripletChanged(TripletField::DrawingColorRgb, 1, value) }),
        text_input("B (0-255)", &app.draft.drawing_color.rgb[2])
            .on_input(|value| { Message::TripletChanged(TripletField::DrawingColorRgb, 2, value) }),
        color_preview_labeled(app.draft.drawing_color.preview_color()),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let mut column = Column::new().spacing(8).push(rgb_inputs);

    if app.draft.drawing_color.preview_color().is_none()
        && app
            .draft
            .drawing_color
            .rgb
            .iter()
            .any(|value| !value.trim().is_empty())
    {
        column = column.push(
            text("RGB values must be between 0 and 255")
                .size(12)
                .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.6, 0.6))),
        );
    }

    column.into()
}
