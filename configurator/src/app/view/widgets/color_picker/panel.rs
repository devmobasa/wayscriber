use crate::app::view::theme;
use iced::border::Radius;
use iced::widget::{Space, button, column, container, row, slider, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow};

use crate::messages::Message;
use crate::models::color::{hex_from_rgb, hsv_to_rgb};
use crate::models::{ColorPickerId, ColorPickerValue};

use super::color_gradient;

const COLOR_SLIDER_STEP: f32 = 0.001;
const PRESET_SWATCH_SIZE: f32 = 24.0;
const PRESET_SWATCH_GAP: f32 = 6.0;
const PRESET_COLORS: [(&str, [f64; 3]); 11] = [
    ("Red", [1.0, 0.0, 0.0]),
    ("Green", [0.0, 1.0, 0.0]),
    ("Blue", [0.0, 0.0, 1.0]),
    ("Yellow", [1.0, 1.0, 0.0]),
    ("White", [1.0, 1.0, 1.0]),
    ("Black", [0.0, 0.0, 0.0]),
    ("Orange", [1.0, 0.5, 0.0]),
    ("Pink", [1.0, 0.0, 1.0]),
    ("Cyan", [0.0, 1.0, 1.0]),
    ("Purple", [0.6, 0.4, 0.8]),
    ("Gray", [0.4, 0.4, 0.4]),
];

pub(in crate::app::view) fn picker_panel<'a>(
    id: ColorPickerId,
    hue: f64,
    saturation: f64,
    value: f64,
    rgb: [f64; 3],
    alpha: Option<f64>,
) -> Element<'a, Message> {
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

    let mut column = column![
        color_gradient(id, hue, saturation, value, alpha),
        row![
            color_swatch(rgb, false),
            text(hex_from_rgb(rgb)).size(12),
            Space::new().width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        preset_palette(id, rgb, alpha),
        row![text("Saturation").size(12), saturation_slider]
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
        .padding(10)
        .style(theme::Container::Box)
        .into()
}

fn preset_palette<'a>(
    id: ColorPickerId,
    current_rgb: [f64; 3],
    alpha: Option<f64>,
) -> Element<'a, Message> {
    PRESET_COLORS
        .chunks(6)
        .fold(column![].spacing(PRESET_SWATCH_GAP), |column, chunk| {
            let row = chunk.iter().fold(
                row![].spacing(PRESET_SWATCH_GAP).align_y(Alignment::Center),
                |row, (_label, rgb)| {
                    row.push(
                        button(color_swatch(*rgb, colors_approx_equal(*rgb, current_rgb)))
                            .padding(0)
                            .on_press(Message::ColorPickerChanged(
                                id,
                                ColorPickerValue { rgb: *rgb, alpha },
                            )),
                    )
                },
            );
            column.push(row)
        })
        .into()
}

fn color_swatch<'a>(rgb: [f64; 3], selected: bool) -> Element<'a, Message> {
    container(
        Space::new()
            .width(Length::Fixed(PRESET_SWATCH_SIZE))
            .height(Length::Fixed(PRESET_SWATCH_SIZE)),
    )
    .style(swatch_style(rgb, selected))
    .into()
}

fn swatch_style(rgb: [f64; 3], selected: bool) -> impl Fn(&iced::Theme) -> container::Style {
    move |_theme| {
        let color = Color::from_rgb(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32);
        let luminance = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
        let border_color = if selected {
            Color::from_rgb(0.95, 0.95, 0.95)
        } else if luminance < 0.3 {
            Color::from_rgba(0.65, 0.68, 0.72, 0.85)
        } else {
            Color::from_rgba(0.12, 0.13, 0.15, 0.75)
        };

        container::Style {
            background: Some(Background::Color(color)),
            text_color: None,
            border: Border {
                color: border_color,
                width: if selected { 2.0 } else { 1.0 },
                radius: Radius::from(4.0),
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

fn colors_approx_equal(left: [f64; 3], right: [f64; 3]) -> bool {
    left.iter()
        .zip(right.iter())
        .all(|(left, right)| (left - right).abs() <= 0.01)
}
