use iced::widget::{checkbox, column, pick_list, row, text, text_input};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{
    OverrideOption, PresetTextField, PresetToggleField, TextField, ToggleField,
    ToolbarOverrideField,
};

use super::constants::{DEFAULT_LABEL_GAP, SMALL_PICKER_WIDTH};
use super::labels::{default_value_text, feedback_text};

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
