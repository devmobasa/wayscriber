use iced::theme;
use iced::widget::text;

pub(super) fn default_label_color(changed: bool) -> iced::Color {
    if changed {
        iced::Color::from_rgb(0.95, 0.6, 0.2)
    } else {
        iced::Color::from_rgb(0.65, 0.74, 0.82)
    }
}

pub(super) fn default_value_text<'a>(
    value: impl Into<String>,
    changed: bool,
) -> iced::widget::Text<'a> {
    let label = format!("Default: {}", value.into());
    text(label)
        .size(12)
        .style(theme::Text::Color(default_label_color(changed)))
}

pub(super) fn feedback_text<'a>(
    message: impl Into<String>,
    is_error: bool,
) -> iced::widget::Text<'a> {
    let color = if is_error {
        iced::Color::from_rgb(0.95, 0.6, 0.6)
    } else {
        iced::Color::from_rgb(0.6, 0.6, 0.6)
    };
    let message = message.into();
    text(message).size(12).style(theme::Text::Color(color))
}
