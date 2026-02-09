use iced::widget::canvas::{self, Frame, Path, Program, Stroke};
use iced::{Color, Point, Rectangle, Size};

use crate::messages::Message;
use crate::models::color::hsv_to_rgb;
use crate::models::{ColorPickerId, ColorPickerValue};

#[derive(Debug, Default)]
pub(super) struct DragState {
    dragging: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SvCanvas {
    pub id: ColorPickerId,
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    pub alpha: Option<f32>,
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
pub(super) struct HueCanvas {
    pub id: ColorPickerId,
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    pub alpha: Option<f32>,
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
