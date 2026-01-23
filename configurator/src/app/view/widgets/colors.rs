use iced::border::Radius;
use iced::theme::{self, Theme};
use iced::widget::container::Appearance;
use iced::widget::{Space, column, container, text};
use iced::{Background, Border, Element, Length};

use crate::messages::Message;

pub(in crate::app::view) fn color_preview_badge<'a>(
    color: Option<iced::Color>,
) -> Element<'a, Message> {
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

pub(in crate::app::view) fn color_preview_labeled<'a>(
    color: Option<iced::Color>,
) -> Element<'a, Message> {
    column![text("Preview").size(12), color_preview_badge(color)]
        .spacing(2)
        .align_items(iced::Alignment::Center)
        .into()
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
