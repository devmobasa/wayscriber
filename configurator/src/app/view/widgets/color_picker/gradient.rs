use iced::border::Radius;
use iced::widget::{Space, button, column, container, row};
use iced::{Background, Border, Color, Element, Length, Shadow};

use crate::app::view::theme;
use crate::messages::Message;
use crate::models::color::hsv_to_rgb;
use crate::models::{ColorPickerId, ColorPickerValue};

const GRADIENT_COLUMNS: usize = 18;
const GRADIENT_ROWS: usize = 6;
const CELL_HEIGHT: f32 = 15.0;

pub(in crate::app::view) fn color_gradient<'a>(
    id: ColorPickerId,
    hue: f64,
    saturation: f64,
    value: f64,
    alpha: Option<f64>,
) -> Element<'a, Message> {
    let selected_col = selected_index(hue, GRADIENT_COLUMNS);
    let selected_row = selected_index(1.0 - value, GRADIENT_ROWS);

    (0..GRADIENT_ROWS)
        .fold(column![].spacing(2), |column, row_index| {
            let row_value = 1.0 - normalized_step(row_index, GRADIENT_ROWS);
            let row = (0..GRADIENT_COLUMNS).fold(row![].spacing(2), |row, col_index| {
                let cell_hue = normalized_step(col_index, GRADIENT_COLUMNS);
                let rgb = hsv_to_rgb(cell_hue, saturation, row_value);
                let selected = row_index == selected_row && col_index == selected_col;

                row.push(
                    button(color_cell(rgb, selected))
                        .padding(0)
                        .width(Length::FillPortion(1))
                        .height(Length::Fixed(CELL_HEIGHT))
                        .style(theme::Button::Subtle)
                        .on_press(Message::ColorPickerChanged(
                            id,
                            ColorPickerValue { rgb, alpha },
                        )),
                )
            });
            column.push(row)
        })
        .into()
}

fn color_cell<'a>(rgb: [f64; 3], selected: bool) -> Element<'a, Message> {
    container(
        Space::new()
            .width(Length::Fill)
            .height(Length::Fixed(CELL_HEIGHT)),
    )
    .style(cell_style(rgb, selected))
    .into()
}

fn cell_style(rgb: [f64; 3], selected: bool) -> impl Fn(&iced::Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(Color::from_rgb(
            rgb[0] as f32,
            rgb[1] as f32,
            rgb[2] as f32,
        ))),
        text_color: None,
        border: Border {
            color: if selected {
                Color::from_rgba(1.0, 1.0, 1.0, 0.95)
            } else {
                Color::from_rgba(0.0, 0.0, 0.0, 0.22)
            },
            width: if selected { 2.0 } else { 1.0 },
            radius: Radius::from(2.0),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

fn normalized_step(index: usize, count: usize) -> f64 {
    if count <= 1 {
        0.0
    } else {
        index as f64 / (count - 1) as f64
    }
}

fn selected_index(value: f64, count: usize) -> usize {
    if count <= 1 {
        0
    } else {
        (value.clamp(0.0, 1.0) * (count - 1) as f64).round() as usize
    }
}
