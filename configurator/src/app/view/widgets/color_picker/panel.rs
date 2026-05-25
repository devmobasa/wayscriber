use crate::app::view::theme;
use iced::widget::{column, container, row, slider, text};
use iced::{Alignment, Element, Length};

use crate::messages::Message;
use crate::models::color::hsv_to_rgb;
use crate::models::{ColorPickerId, ColorPickerValue};

const COLOR_SLIDER_STEP: f32 = 0.001;

pub(in crate::app::view) fn picker_panel<'a>(
    id: ColorPickerId,
    hue: f64,
    saturation: f64,
    value: f64,
    rgb: [f64; 3],
    alpha: Option<f64>,
) -> Element<'a, Message> {
    let hue_slider = slider(0.0..=1.0, hue as f32, move |val| {
        Message::ColorPickerChanged(
            id,
            ColorPickerValue {
                rgb: hsv_to_rgb(val as f64, saturation, value),
                alpha,
            },
        )
    })
    .step(COLOR_SLIDER_STEP)
    .width(Length::Fill);

    let saturation_slider = slider(0.0..=1.0, saturation as f32, move |val| {
        Message::ColorPickerChanged(
            id,
            ColorPickerValue {
                rgb: hsv_to_rgb(hue, val as f64, value),
                alpha,
            },
        )
    })
    .step(COLOR_SLIDER_STEP)
    .width(Length::Fill);

    let value_slider = slider(0.0..=1.0, value as f32, move |val| {
        Message::ColorPickerChanged(
            id,
            ColorPickerValue {
                rgb: hsv_to_rgb(hue, saturation, val as f64),
                alpha,
            },
        )
    })
    .step(COLOR_SLIDER_STEP)
    .width(Length::Fill);

    let mut column = column![
        row![text("Hue").size(12), hue_slider]
            .spacing(8)
            .align_y(Alignment::Center),
        row![text("Saturation").size(12), saturation_slider]
            .spacing(8)
            .align_y(Alignment::Center),
        row![text("Value").size(12), value_slider]
            .spacing(8)
            .align_y(Alignment::Center),
    ]
    .spacing(8);

    if let Some(alpha_value) = alpha {
        let slider = slider(0.0..=1.0, alpha_value as f32, move |val| {
            Message::ColorPickerChanged(
                id,
                ColorPickerValue {
                    rgb,
                    alpha: Some(val as f64),
                },
            )
        })
        .step(COLOR_SLIDER_STEP)
        .width(Length::Fill);

        column = column.push(
            row![text("Alpha").size(12), slider]
                .spacing(8)
                .align_y(Alignment::Center),
        );
    }

    container(column)
        .padding(8)
        .style(theme::Container::Box)
        .into()
}
