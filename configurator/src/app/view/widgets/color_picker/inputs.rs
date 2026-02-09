use iced::widget::{button, checkbox, column, row, text, text_input};
use iced::{Alignment, Color, Element, Length, Theme};

use crate::messages::Message;
use crate::models::color::{parse_quad_values, parse_triplet_values, rgb_to_hsv};
use crate::models::{ColorQuadInput, ColorTripletInput, QuadField};

use super::super::colors::color_preview_badge;
use super::super::constants::DEFAULT_LABEL_GAP;
use super::super::labels::default_value_text;
use super::ColorPickerUi;
use super::panel::picker_panel;

fn input<'a>(placeholder: &'static str, value: &'a str) -> iced::widget::TextInput<'a, Message> {
    text_input::<Message, Theme, iced::Renderer>(placeholder, value)
}

pub(in crate::app::view) fn color_triplet_picker<'a>(
    label: &'static str,
    picker: ColorPickerUi<'a>,
    triplet: &'a ColorTripletInput,
    index: usize,
    on_component: fn(usize, usize, String) -> Message,
) -> Element<'a, Message> {
    let rgb = parse_triplet_values(&triplet.components);
    let (hue, saturation, value) = rgb_to_hsv(rgb);
    let preview = Color::from_rgb(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32);

    let header = row![
        text(label).size(14),
        color_preview_badge(Some(preview)),
        input("HEX", picker.hex_value)
            .on_input(move |val| Message::ColorPickerHexChanged(picker.id, val))
            .width(Length::Fixed(120.0)),
        button(if picker.is_open {
            "Hide picker"
        } else {
            "Pick"
        })
        .on_press(Message::ColorPickerToggled(picker.id)),
        checkbox("Advanced", picker.show_advanced)
            .on_toggle(move |value| { Message::ColorPickerAdvancedToggled(picker.id, value) }),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let picker_panel: Element<'a, Message> = if picker.is_open {
        picker_panel(picker.id, hue, saturation, value, rgb, None)
    } else {
        column![].into()
    };

    let advanced_inputs: Element<'a, Message> = if picker.show_advanced {
        row![
            input("R", &triplet.components[0]).on_input(move |val| on_component(index, 0, val)),
            input("G", &triplet.components[1]).on_input(move |val| on_component(index, 1, val)),
            input("B", &triplet.components[2]).on_input(move |val| on_component(index, 2, val)),
        ]
        .spacing(8)
        .align_items(Alignment::Center)
        .into()
    } else {
        column![].into()
    };

    column![header, picker_panel, advanced_inputs]
        .spacing(6)
        .into()
}

pub(in crate::app::view) fn color_quad_picker<'a>(
    label: &'static str,
    picker: ColorPickerUi<'a>,
    colors: &'a ColorQuadInput,
    default: &'a ColorQuadInput,
    field: QuadField,
) -> Element<'a, Message> {
    let rgba = parse_quad_values(&colors.components);
    let (hue, saturation, value) = rgb_to_hsv([rgba[0], rgba[1], rgba[2]]);
    let preview = Color::from_rgba(
        rgba[0] as f32,
        rgba[1] as f32,
        rgba[2] as f32,
        rgba[3] as f32,
    );
    let changed = colors != default;

    let label_row = row![
        text(label).size(14),
        default_value_text(default.summary(), changed),
    ]
    .spacing(DEFAULT_LABEL_GAP)
    .align_items(Alignment::Center);

    let header = row![
        color_preview_badge(Some(preview)),
        input("HEX", picker.hex_value)
            .on_input(move |val| Message::ColorPickerHexChanged(picker.id, val))
            .width(Length::Fixed(140.0)),
        button(if picker.is_open {
            "Hide picker"
        } else {
            "Pick"
        })
        .on_press(Message::ColorPickerToggled(picker.id)),
        checkbox("Advanced", picker.show_advanced)
            .on_toggle(move |value| { Message::ColorPickerAdvancedToggled(picker.id, value) }),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let picker_panel: Element<'a, Message> = if picker.is_open {
        picker_panel(
            picker.id,
            hue,
            saturation,
            value,
            [rgba[0], rgba[1], rgba[2]],
            Some(rgba[3]),
        )
    } else {
        column![].into()
    };

    let advanced_inputs: Element<'a, Message> = if picker.show_advanced {
        row![
            input("Red", &colors.components[0])
                .on_input(move |val| Message::QuadChanged(field, 0, val)),
            input("Green", &colors.components[1])
                .on_input(move |val| Message::QuadChanged(field, 1, val)),
            input("Blue", &colors.components[2])
                .on_input(move |val| Message::QuadChanged(field, 2, val)),
            input("Alpha", &colors.components[3])
                .on_input(move |val| Message::QuadChanged(field, 3, val)),
        ]
        .spacing(8)
        .align_items(Alignment::Center)
        .into()
    } else {
        column![].into()
    };

    column![label_row, header, picker_panel, advanced_inputs]
        .spacing(6)
        .into()
}
