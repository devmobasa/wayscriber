use iced::border::Radius;
use iced::theme::{self, Theme};
use iced::widget::container::Appearance;
use iced::widget::{Space, checkbox, column, container, pick_list, row, text, text_input};
use iced::{Background, Border, Element, Length};

use crate::messages::Message;
use crate::models::util::format_float;
use crate::models::{
    ColorQuadInput, ColorTripletInput, OverrideOption, PresetTextField, PresetToggleField,
    QuadField, TextField, ToggleField, ToolbarOverrideField, TripletField,
};

pub(super) const DEFAULT_LABEL_GAP: f32 = 12.0;
pub(super) const LABEL_COLUMN_WIDTH: f32 = 220.0;
pub(super) const SMALL_PICKER_WIDTH: f32 = 140.0;
pub(super) const COLOR_PICKER_WIDTH: f32 = 160.0;
pub(super) const BUFFER_PICKER_WIDTH: f32 = 120.0;

pub(super) fn labeled_input<'a>(
    label: &'static str,
    value: &'a str,
    default: &'a str,
    field: TextField,
) -> Element<'a, Message> {
    labeled_input_with_feedback(label, value, default, field, None, None)
}

pub(super) fn labeled_input_with_feedback<'a>(
    label: &'static str,
    value: &'a str,
    default: &'a str,
    field: TextField,
    hint: Option<&'static str>,
    error: Option<String>,
) -> Element<'a, Message> {
    let changed = value.trim() != default.trim();
    let input = text_input(label, value).on_input(move |val| Message::TextChanged(field, val));
    let mut column = column![
        row![text(label).size(14), default_value_text(default, changed)]
            .spacing(DEFAULT_LABEL_GAP)
            .align_items(iced::Alignment::Center),
        input
    ]
    .spacing(4)
    .width(Length::Fill);

    if let Some(message) = error {
        column = column.push(feedback_text(message, true));
    } else if let Some(message) = hint {
        column = column.push(feedback_text(message, false));
    }

    column.into()
}

pub(super) fn labeled_input_state<'a>(
    label: &'static str,
    value: &'a str,
    default: &'a str,
    field: TextField,
    enabled: bool,
    hint: Option<&'static str>,
    error: Option<String>,
) -> Element<'a, Message> {
    let changed = value.trim() != default.trim();
    let input = if enabled {
        text_input(label, value).on_input(move |val| Message::TextChanged(field, val))
    } else {
        text_input(label, value)
    };

    let mut column = column![
        row![text(label).size(14), default_value_text(default, changed)]
            .spacing(DEFAULT_LABEL_GAP)
            .align_items(iced::Alignment::Center),
        input
    ]
    .spacing(4)
    .width(Length::Fill);

    if let Some(message) = error {
        column = column.push(feedback_text(message, true));
    } else if let Some(message) = hint {
        column = column.push(feedback_text(message, false));
    }

    column.into()
}

pub(super) fn labeled_control<'a>(
    label: &'static str,
    control: Element<'a, Message>,
    default: impl Into<String>,
    changed: bool,
) -> Element<'a, Message> {
    column![
        row![text(label).size(14), default_value_text(default, changed)]
            .spacing(DEFAULT_LABEL_GAP)
            .align_items(iced::Alignment::Center),
        control
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

pub(super) fn preset_input<'a>(
    label: &'static str,
    value: &'a str,
    default: &'a str,
    slot_index: usize,
    field: PresetTextField,
    show_unset: bool,
) -> Element<'a, Message> {
    let changed = value.trim() != default.trim();
    let default_label = if show_unset && default.trim().is_empty() {
        "unset".to_string()
    } else {
        default.trim().to_string()
    };

    column![
        row![
            text(label).size(14),
            default_value_text(default_label, changed)
        ]
        .spacing(DEFAULT_LABEL_GAP)
        .align_items(iced::Alignment::Center),
        text_input(label, value)
            .on_input(move |val| Message::PresetTextChanged(slot_index, field, val))
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

pub(super) fn preset_override_control<'a>(
    label: &'static str,
    value: OverrideOption,
    default: OverrideOption,
    slot_index: usize,
    field: PresetToggleField,
) -> Element<'a, Message> {
    let picker = pick_list(OverrideOption::list(), Some(value), move |opt| {
        Message::PresetToggleOptionChanged(slot_index, field, opt)
    })
    .width(Length::Fill);

    labeled_control(label, picker.into(), default.label(), value != default)
}

pub(super) fn override_row<'a>(
    field: ToolbarOverrideField,
    value: OverrideOption,
) -> Element<'a, Message> {
    row![
        text(field.label()).size(14),
        pick_list(OverrideOption::list(), Some(value), move |opt| {
            Message::ToolbarOverrideChanged(field, opt)
        },)
        .width(Length::Fixed(SMALL_PICKER_WIDTH)),
    ]
    .spacing(12)
    .align_items(iced::Alignment::Center)
    .into()
}

pub(super) fn color_triplet_editor<'a>(
    label: &'static str,
    colors: &'a ColorTripletInput,
    default: &'a ColorTripletInput,
    field: TripletField,
) -> Element<'a, Message> {
    let changed = colors != default;
    column![
        row![
            text(label).size(14),
            default_value_text(default.summary(), changed),
        ]
        .spacing(DEFAULT_LABEL_GAP)
        .align_items(iced::Alignment::Center),
        row![
            text_input("Red", &colors.components[0])
                .on_input(move |val| Message::TripletChanged(field, 0, val)),
            text_input("Green", &colors.components[1])
                .on_input(move |val| Message::TripletChanged(field, 1, val)),
            text_input("Blue", &colors.components[2])
                .on_input(move |val| Message::TripletChanged(field, 2, val)),
        ]
        .spacing(8)
    ]
    .spacing(4)
    .into()
}

pub(super) fn color_quad_editor<'a>(
    label: &'static str,
    colors: &'a ColorQuadInput,
    default: &'a ColorQuadInput,
    field: QuadField,
) -> Element<'a, Message> {
    let changed = colors != default;
    column![
        row![
            text(label).size(14),
            default_value_text(default.summary(), changed),
        ]
        .spacing(DEFAULT_LABEL_GAP)
        .align_items(iced::Alignment::Center),
        row![
            text_input("Red", &colors.components[0])
                .on_input(move |val| Message::QuadChanged(field, 0, val)),
            text_input("Green", &colors.components[1])
                .on_input(move |val| Message::QuadChanged(field, 1, val)),
            text_input("Blue", &colors.components[2])
                .on_input(move |val| Message::QuadChanged(field, 2, val)),
            text_input("Alpha", &colors.components[3])
                .on_input(move |val| Message::QuadChanged(field, 3, val)),
        ]
        .spacing(8)
    ]
    .spacing(4)
    .into()
}

pub(super) fn color_preview_badge<'a>(color: Option<iced::Color>) -> Element<'a, Message> {
    let (preview_color, is_valid) = match color {
        Some(color) => (color, true),
        None => (iced::Color::from_rgb(0.2, 0.2, 0.2), false),
    };

    let content: Element<'_, Message> = if is_valid {
        Space::new(Length::Shrink, Length::Shrink).into()
    } else {
        text("?")
            .size(14)
            .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.95, 0.95)))
            .into()
    };

    container(content)
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(24.0))
        .center_x()
        .center_y()
        .style(theme::Container::Custom(Box::new(ColorPreviewStyle {
            color: preview_color,
            is_invalid: !is_valid,
        })))
        .into()
}

pub(super) fn color_preview_labeled<'a>(color: Option<iced::Color>) -> Element<'a, Message> {
    column![text("Preview").size(12), color_preview_badge(color)]
        .spacing(2)
        .align_items(iced::Alignment::Center)
        .into()
}

pub(super) fn default_label_color(changed: bool) -> iced::Color {
    if changed {
        iced::Color::from_rgb(0.95, 0.6, 0.2)
    } else {
        iced::Color::from_rgb(0.65, 0.74, 0.82)
    }
}

pub(super) fn default_value_text<'a>(
    value: impl Into<String>,
    changed: bool,
) -> iced::widget::Text<'a> {
    let label = format!("Default: {}", value.into());
    text(label)
        .size(12)
        .style(theme::Text::Color(default_label_color(changed)))
}

pub(super) fn feedback_text<'a>(
    message: impl Into<String>,
    is_error: bool,
) -> iced::widget::Text<'a> {
    let color = if is_error {
        iced::Color::from_rgb(0.95, 0.6, 0.6)
    } else {
        iced::Color::from_rgb(0.6, 0.6, 0.6)
    };
    let message = message.into();
    text(message).size(12).style(theme::Text::Color(color))
}

pub(super) fn bool_label(value: bool) -> &'static str {
    if value { "On" } else { "Off" }
}

pub(super) fn toggle_row<'a>(
    label: &'static str,
    value: bool,
    default: bool,
    field: ToggleField,
) -> Element<'a, Message> {
    let changed = value != default;
    row![
        checkbox(label, value).on_toggle(move |val| Message::ToggleChanged(field, val)),
        default_value_text(bool_label(default), changed),
    ]
    .spacing(DEFAULT_LABEL_GAP)
    .align_items(iced::Alignment::Center)
    .into()
}

pub(super) fn validate_f64_range(value: &str, min: f64, max: f64) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a numeric value".to_string());
    }

    match trimmed.parse::<f64>() {
        Ok(value) => {
            if value < min || value > max {
                Some(format!(
                    "Range: {}-{}",
                    format_float(min),
                    format_float(max)
                ))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a numeric value".to_string()),
    }
}

pub(super) fn validate_u32_range(value: &str, min: u32, max: u32) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }

    match trimmed.parse::<u32>() {
        Ok(value) => {
            if value < min || value > max {
                Some(format!("Range: {min}-{max}"))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

pub(super) fn validate_u64_range(value: &str, min: u64, max: u64) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }

    match trimmed.parse::<u64>() {
        Ok(value) => {
            if value < min || value > max {
                Some(format!("Range: {min}-{max}"))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

pub(super) fn validate_u64_min(value: &str, min: u64) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }

    match trimmed.parse::<u64>() {
        Ok(value) => {
            if value < min {
                Some(format!("Minimum: {min}"))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

pub(super) fn validate_usize_range(value: &str, min: usize, max: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }

    match trimmed.parse::<usize>() {
        Ok(value) => {
            if value < min || value > max {
                Some(format!("Range: {min}-{max}"))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

pub(super) fn validate_usize_min(value: &str, min: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }

    match trimmed.parse::<usize>() {
        Ok(value) => {
            if value < min {
                Some(format!("Minimum: {min}"))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

#[derive(Clone, Copy)]
struct ColorPreviewStyle {
    color: iced::Color,
    is_invalid: bool,
}

impl container::StyleSheet for ColorPreviewStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            background: Some(Background::Color(self.color)),
            text_color: None,
            border: Border {
                color: if self.is_invalid {
                    iced::Color::from_rgb(0.9, 0.4, 0.4)
                } else {
                    iced::Color::from_rgb(0.4, 0.4, 0.4)
                },
                width: 1.0,
                radius: Radius::from(6.0),
            },
            shadow: Default::default(),
        }
    }
}
