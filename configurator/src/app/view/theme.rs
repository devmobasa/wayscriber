#![allow(non_snake_case, non_upper_case_globals)]

pub mod Button {
    pub use iced::widget::button::primary as Primary;
    pub use iced::widget::button::secondary as Secondary;
}

pub mod Container {
    pub use iced::widget::container::rounded_box as Box;
}

pub mod Text {
    pub fn Color(color: iced::Color) -> impl Fn(&iced::Theme) -> iced::widget::text::Style {
        move |_theme| iced::widget::text::Style { color: Some(color) }
    }
}
