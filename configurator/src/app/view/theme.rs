#![allow(non_snake_case, non_upper_case_globals)]

pub mod Button {
    use iced::widget::button::{self, Status, Style};
    use iced::{Background, Border, Shadow};

    pub use iced::widget::button::primary as Primary;
    pub use iced::widget::button::secondary as Secondary;
    pub use iced::widget::button::subtle as Subtle;
    pub use iced::widget::button::warning as Warning;

    pub fn TabActive(theme: &iced::Theme, status: Status) -> Style {
        let palette = theme.extended_palette();
        let background = match status {
            Status::Active => palette.background.weakest.color,
            Status::Hovered => palette.background.weak.color,
            Status::Pressed => palette.background.strong.color,
            Status::Disabled => palette.background.weakest.color.scale_alpha(0.5),
        };
        let text_color = match status {
            Status::Disabled => palette.primary.base.color.scale_alpha(0.5),
            _ => palette.primary.strong.color,
        };

        Style {
            background: Some(Background::Color(background)),
            text_color,
            border: Border {
                color: palette.primary.base.color,
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }

    pub fn TabInactive(theme: &iced::Theme, status: Status) -> Style {
        let mut style = button::background(theme, status);
        style.border.radius = 4.0.into();
        style
    }
}

pub mod Container {
    use iced::{Background, Border, Shadow};

    pub use iced::widget::container::rounded_box as Box;
    pub use iced::widget::container::warning as Warning;

    pub fn ActionBar(theme: &iced::Theme) -> iced::widget::container::Style {
        let palette = theme.extended_palette();

        iced::widget::container::Style {
            background: Some(Background::Color(palette.background.weakest.color)),
            text_color: Some(palette.background.weakest.text),
            border: Border {
                color: palette.background.weak.color,
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

pub mod Text {
    pub fn Color(color: iced::Color) -> impl Fn(&iced::Theme) -> iced::widget::text::Style {
        move |_theme| iced::widget::text::Style { color: Some(color) }
    }
}
