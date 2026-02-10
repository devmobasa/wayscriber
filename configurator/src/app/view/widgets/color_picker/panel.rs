use iced::theme;
use iced::widget::canvas::Canvas;
use iced::widget::{column, container, row, slider, text};
use iced::{Alignment, Element, Length};

use crate::messages::Message;
use crate::models::{ColorPickerId, ColorPickerValue};

use super::canvas::{HueCanvas, SvCanvas};

pub(super) fn picker_panel<'a>(
    id: ColorPickerId,
    hue: f64,
    saturation: f64,
    value: f64,
    rgb: [f64; 3],
    alpha: Option<f64>,
) -> Element<'a, Message> {
    let sv = Canvas::new(SvCanvas {
        id,
        hue: hue as f32,
        saturation: saturation as f32,
        value: value as f32,
        alpha: alpha.map(|val| val as f32),
    })
    .width(Length::Fixed(220.0))
    .height(Length::Fixed(150.0));

    let hue_slider = Canvas::new(HueCanvas {
        id,
        hue: hue as f32,
        saturation: saturation as f32,
        value: value as f32,
        alpha: alpha.map(|val| val as f32),
    })
    .width(Length::Fill)
    .height(Length::Fixed(16.0));

    let mut column = column![
        sv,
        row![text("Hue").size(12), hue_slider]
            .spacing(8)
            .align_items(Alignment::Center),
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
        .width(Length::Fill);

        column = column.push(
            row![text("Alpha").size(12), slider]
                .spacing(8)
                .align_items(Alignment::Center),
        );
    }

    container(column)
        .padding(8)
        .style(theme::Container::Box)
        .into()
}
