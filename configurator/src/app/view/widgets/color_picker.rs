use iced::theme;
use iced::widget::canvas::{self, Canvas, Frame, Path, Program, Stroke};
use iced::widget::{button, checkbox, column, container, row, slider, text, text_input};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Size, Theme};

use crate::messages::Message;
use crate::models::color::{hsv_to_rgb, parse_quad_values, parse_triplet_values, rgb_to_hsv};
use crate::models::{
    ColorPickerId, ColorPickerValue, ColorQuadInput, ColorTripletInput, QuadField,
};

use super::colors::color_preview_badge;
use super::constants::DEFAULT_LABEL_GAP;
use super::labels::default_value_text;

fn input<'a>(placeholder: &'static str, value: &'a str) -> iced::widget::TextInput<'a, Message> {
    text_input::<Message, Theme, iced::Renderer>(placeholder, value)
}

#[derive(Clone, Copy)]
pub(in crate::app::view) struct ColorPickerUi<'a> {
    pub id: ColorPickerId,
    pub is_open: bool,
    pub show_advanced: bool,
    pub hex_value: &'a str,
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

    let picker_panel = if picker.is_open {
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

    let picker_panel = if picker.is_open {
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

fn picker_panel<'a>(
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

#[derive(Debug, Default)]
struct DragState {
    dragging: bool,
}

#[derive(Debug, Clone, Copy)]
struct SvCanvas {
    id: ColorPickerId,
    hue: f32,
    saturation: f32,
    value: f32,
    alpha: Option<f32>,
}

impl Program<Message> for SvCanvas {
    type State = DragState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        match event {
            canvas::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_over(bounds) {
                    state.dragging = true;
                    return (
                        canvas::event::Status::Captured,
                        Some(self.message_from_position(bounds, pos)),
                    );
                }
            }
            canvas::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                state.dragging = false;
            }
            canvas::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                if state.dragging {
                    let pos = clamp_point(position, bounds);
                    return (
                        canvas::event::Status::Captured,
                        Some(self.message_from_position(bounds, pos)),
                    );
                }
            }
            _ => {}
        }

        (canvas::event::Status::Ignored, None)
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let rect = Path::rectangle(Point::ORIGIN, frame.size());

        let hue_rgb = hsv_to_rgb(self.hue as f64, 1.0, 1.0);
        frame.fill(
            &rect,
            Color::from_rgb(hue_rgb[0] as f32, hue_rgb[1] as f32, hue_rgb[2] as f32),
        );

        let horizontal =
            canvas::gradient::Linear::new(Point::ORIGIN, Point::new(frame.width(), 0.0))
                .add_stop(0.0, Color::from_rgba(1.0, 1.0, 1.0, 1.0))
                .add_stop(1.0, Color::from_rgba(1.0, 1.0, 1.0, 0.0));
        frame.fill(&rect, horizontal);

        let vertical =
            canvas::gradient::Linear::new(Point::ORIGIN, Point::new(0.0, frame.height()))
                .add_stop(0.0, Color::from_rgba(0.0, 0.0, 0.0, 0.0))
                .add_stop(1.0, Color::from_rgba(0.0, 0.0, 0.0, 1.0));
        frame.fill(&rect, vertical);

        let x = (self.saturation * frame.width()).clamp(0.0, frame.width());
        let y = ((1.0 - self.value) * frame.height()).clamp(0.0, frame.height());
        let indicator = Path::circle(Point::new(x, y), 6.0);
        frame.stroke(
            &indicator,
            Stroke::default().with_width(2.0).with_color(Color::WHITE),
        );
        frame.stroke(
            &indicator,
            Stroke::default().with_width(1.0).with_color(Color::BLACK),
        );

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        if cursor.is_over(bounds) {
            iced::mouse::Interaction::Pointer
        } else {
            iced::mouse::Interaction::default()
        }
    }
}

impl SvCanvas {
    fn message_from_position(&self, bounds: Rectangle, position: Point) -> Message {
        let width = bounds.width.max(1.0);
        let height = bounds.height.max(1.0);
        let saturation = (position.x - bounds.x) / width;
        let value = 1.0 - (position.y - bounds.y) / height;
        let rgb = hsv_to_rgb(
            self.hue as f64,
            saturation.clamp(0.0, 1.0) as f64,
            value.clamp(0.0, 1.0) as f64,
        );

        Message::ColorPickerChanged(
            self.id,
            ColorPickerValue {
                rgb,
                alpha: self.alpha.map(|val| val as f64),
            },
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct HueCanvas {
    id: ColorPickerId,
    hue: f32,
    saturation: f32,
    value: f32,
    alpha: Option<f32>,
}

impl Program<Message> for HueCanvas {
    type State = DragState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        match event {
            canvas::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_over(bounds) {
                    state.dragging = true;
                    return (
                        canvas::event::Status::Captured,
                        Some(self.message_from_position(bounds, pos)),
                    );
                }
            }
            canvas::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                state.dragging = false;
            }
            canvas::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                if state.dragging {
                    let pos = clamp_point(position, bounds);
                    return (
                        canvas::event::Status::Captured,
                        Some(self.message_from_position(bounds, pos)),
                    );
                }
            }
            _ => {}
        }

        (canvas::event::Status::Ignored, None)
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let rect = Path::rectangle(Point::ORIGIN, frame.size());

        let gradient = canvas::gradient::Linear::new(Point::ORIGIN, Point::new(frame.width(), 0.0))
            .add_stop(0.0, Color::from_rgb(1.0, 0.0, 0.0))
            .add_stop(1.0 / 6.0, Color::from_rgb(1.0, 1.0, 0.0))
            .add_stop(2.0 / 6.0, Color::from_rgb(0.0, 1.0, 0.0))
            .add_stop(3.0 / 6.0, Color::from_rgb(0.0, 1.0, 1.0))
            .add_stop(4.0 / 6.0, Color::from_rgb(0.0, 0.0, 1.0))
            .add_stop(5.0 / 6.0, Color::from_rgb(1.0, 0.0, 1.0))
            .add_stop(1.0, Color::from_rgb(1.0, 0.0, 0.0));
        frame.fill(&rect, gradient);

        let x = (self.hue * frame.width()).clamp(0.0, frame.width());
        let indicator = Path::rectangle(Point::new(x - 1.0, 0.0), Size::new(2.0, frame.height()));
        frame.stroke(
            &indicator,
            Stroke::default().with_width(2.0).with_color(Color::WHITE),
        );
        frame.stroke(
            &indicator,
            Stroke::default().with_width(1.0).with_color(Color::BLACK),
        );

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        if cursor.is_over(bounds) {
            iced::mouse::Interaction::Pointer
        } else {
            iced::mouse::Interaction::default()
        }
    }
}

impl HueCanvas {
    fn message_from_position(&self, bounds: Rectangle, position: Point) -> Message {
        let width = bounds.width.max(1.0);
        let hue = ((position.x - bounds.x) / width).clamp(0.0, 1.0);
        let rgb = hsv_to_rgb(hue as f64, self.saturation as f64, self.value as f64);

        Message::ColorPickerChanged(
            self.id,
            ColorPickerValue {
                rgb,
                alpha: self.alpha.map(|val| val as f64),
            },
        )
    }
}

fn clamp_point(point: Point, bounds: Rectangle) -> Point {
    Point::new(
        point.x.clamp(bounds.x, bounds.x + bounds.width),
        point.y.clamp(bounds.y, bounds.y + bounds.height),
    )
}
