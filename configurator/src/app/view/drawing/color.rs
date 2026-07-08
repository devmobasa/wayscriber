use crate::app::view::theme;
use iced::widget::{Column, Row, button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length};
use wayscriber::config::QUICK_COLOR_RENDER_LIMIT;

use crate::messages::Message;
use crate::models::{ColorMode, ColorPickerId, NamedColorOption, TextField, TripletField};

use super::super::super::state::ConfiguratorApp;
use super::super::widgets::{
    COLOR_PICKER_WIDTH, ColorPickerUi, DEFAULT_LABEL_GAP, color_preview_labeled,
    color_rgb255_picker, default_value_text,
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
        .align_y(Alignment::Center),
        color_mode_picker,
        color_section
    ]
    .spacing(8)
    .into()
}

pub(super) fn quick_colors_block(app: &ConfiguratorApp) -> Element<'_, Message> {
    let mut column = Column::new().spacing(10).push(
        row![
            text("Quick colors").size(16),
            button("Add color")
                .style(theme::Button::Secondary)
                .on_press(Message::QuickColorAdded),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    );
    if let Some(warning) =
        quick_color_render_limit_warning(app.draft.drawing_quick_colors.entries.len())
    {
        column = column.push(
            container(text(warning).size(12))
                .padding(8)
                .width(Length::Fill)
                .style(theme::Container::Warning),
        );
    }

    for index in 0..app.draft.drawing_quick_colors.entries.len() {
        column = column.push(quick_color_entry_block(app, index));
    }

    column.into()
}

fn quick_color_render_limit_warning(count: usize) -> Option<String> {
    (count > QUICK_COLOR_RENDER_LIMIT).then(|| {
        format!(
            "Only the first {QUICK_COLOR_RENDER_LIMIT} quick colors are shown in toolbar and radial menus."
        )
    })
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
    .align_y(Alignment::Center);

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

fn quick_color_entry_block(app: &ConfiguratorApp, index: usize) -> Element<'_, Message> {
    let Some(current) = app.draft.drawing_quick_colors.get(index) else {
        return column![].into();
    };
    let default = app.defaults.drawing_quick_colors.get(index);
    let changed = default != Some(current);
    let default_summary = default
        .map(|entry| format!("{} / {}", entry.label.trim(), entry.color.summary()))
        .unwrap_or_else(|| "not set".to_string());
    let quick_color_count = app.draft.drawing_quick_colors.entries.len();

    let label_row = row![
        text(format!("Color {}", index + 1)).size(14),
        default_value_text(default_summary, changed),
    ]
    .spacing(DEFAULT_LABEL_GAP)
    .align_y(Alignment::Center);

    let mut remove_button = button("Remove").style(theme::Button::Secondary);
    if quick_color_count > 8 {
        remove_button = remove_button.on_press(Message::QuickColorRemoved(index));
    }

    let controls = row![
        text_input("Label", &current.label)
            .on_input(move |value| Message::TextChanged(TextField::QuickColorLabel(index), value))
            .width(Length::Fill),
        button("Up").on_press(Message::QuickColorMoved(index, -1)),
        button("Down").on_press(Message::QuickColorMoved(index, 1)),
        remove_button,
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let color_mode_picker = Row::new()
        .spacing(8)
        .push(
            button("Named / Hex")
                .style(if current.color.mode == ColorMode::Named {
                    theme::Button::Primary
                } else {
                    theme::Button::Secondary
                })
                .on_press(Message::QuickColorModeChanged(index, ColorMode::Named)),
        )
        .push(
            button("RGB")
                .style(if current.color.mode == ColorMode::Rgb {
                    theme::Button::Primary
                } else {
                    theme::Button::Secondary
                })
                .on_press(Message::QuickColorModeChanged(index, ColorMode::Rgb)),
        );

    let editor = match current.color.mode {
        ColorMode::Named => quick_named_color_section(app, index),
        ColorMode::Rgb => quick_rgb_color_section(app, index),
    };

    column![label_row, controls, color_mode_picker, editor,]
        .spacing(6)
        .into()
}

fn quick_named_color_section(app: &ConfiguratorApp, index: usize) -> Element<'_, Message> {
    let Some(entry) = app.draft.drawing_quick_colors.get(index) else {
        return column![].into();
    };
    let color = &entry.color;
    let picker = pick_list(
        NamedColorOption::list(),
        Some(color.selected_named),
        move |value| Message::QuickNamedColorSelected(index, value),
    )
    .width(Length::Fixed(COLOR_PICKER_WIDTH));

    let picker_row = row![picker, color_preview_labeled(color.preview_color())]
        .spacing(8)
        .align_y(Alignment::Center);

    let mut column = Column::new().spacing(8).push(picker_row);

    if color.selected_named_is_custom() {
        column = column.push(
            text_input("Known color name or #RRGGBB", &color.name)
                .on_input(move |value| {
                    Message::TextChanged(TextField::QuickColorName(index), value)
                })
                .width(Length::Fill),
        );

        if color.preview_color().is_none() && !color.name.trim().is_empty() {
            column = column.push(
                text("Use a known color name or #RRGGBB")
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.6, 0.6))),
            );
        }
    }

    column.into()
}

fn quick_rgb_color_section(app: &ConfiguratorApp, index: usize) -> Element<'_, Message> {
    let Some(entry) = app.draft.drawing_quick_colors.get(index) else {
        return column![].into();
    };
    let picker_id = ColorPickerId::QuickColor(index);
    let color = &entry.color;
    let rgb_picker = color_rgb255_picker(
        ColorPickerUi {
            id: picker_id,
            is_open: app.color_picker_open == Some(picker_id),
            show_advanced: false,
            hex_value: app
                .color_picker_hex
                .get(&picker_id)
                .map(String::as_str)
                .unwrap_or(""),
        },
        &color.rgb,
        color.preview_color(),
        TripletField::QuickColorRgb(index),
    );

    let mut column = Column::new().spacing(8).push(rgb_picker);

    if color.preview_color().is_none() && color.rgb.iter().any(|value| !value.trim().is_empty()) {
        column = column.push(
            text("RGB values must be between 0 and 255")
                .size(12)
                .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.6, 0.6))),
        );
    }

    column.into()
}

fn rgb_color_section(app: &ConfiguratorApp) -> Element<'_, Message> {
    let picker_id = ColorPickerId::DrawingColor;
    let rgb_picker = color_rgb255_picker(
        ColorPickerUi {
            id: picker_id,
            is_open: app.color_picker_open == Some(picker_id),
            show_advanced: false,
            hex_value: app
                .color_picker_hex
                .get(&picker_id)
                .map(String::as_str)
                .unwrap_or(""),
        },
        &app.draft.drawing_color.rgb,
        app.draft.drawing_color.preview_color(),
        TripletField::DrawingColorRgb,
    );

    let mut column = Column::new().spacing(8).push(rgb_picker);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_color_render_limit_warning_starts_after_render_cap() {
        assert!(quick_color_render_limit_warning(QUICK_COLOR_RENDER_LIMIT).is_none());

        let warning = quick_color_render_limit_warning(QUICK_COLOR_RENDER_LIMIT + 1)
            .expect("expected warning once quick colors exceed render cap");

        assert!(warning.contains(&QUICK_COLOR_RENDER_LIMIT.to_string()));
    }
}
