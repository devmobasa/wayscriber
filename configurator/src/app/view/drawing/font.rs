use iced::widget::{column, pick_list, row, text};
use iced::{Alignment, Element, Length};

use crate::messages::Message;
use crate::models::{FontStyleOption, FontWeightOption, TextField};

use super::super::state::ConfiguratorApp;
use super::widgets::{DEFAULT_LABEL_GAP, default_value_text, labeled_input};

pub(super) fn font_controls(app: &ConfiguratorApp) -> Element<'_, Message> {
    let weight_column = column![
        row![
            text("Font weight").size(14),
            default_value_text(
                app.defaults.drawing_font_weight.clone(),
                app.draft.drawing_font_weight != app.defaults.drawing_font_weight,
            )
        ]
        .spacing(DEFAULT_LABEL_GAP)
        .align_items(Alignment::Center),
        pick_list(
            FontWeightOption::list(),
            Some(app.draft.drawing_font_weight_option),
            Message::FontWeightOptionSelected,
        )
        .width(Length::Fill),
        labeled_input(
            "Custom or numeric weight",
            &app.draft.drawing_font_weight,
            &app.defaults.drawing_font_weight,
            TextField::DrawingFontWeight,
        )
    ]
    .spacing(6);

    let mut style_column = column![
        row![
            text("Font style").size(14),
            default_value_text(
                app.defaults.drawing_font_style.clone(),
                app.draft.drawing_font_style != app.defaults.drawing_font_style,
            )
        ]
        .spacing(DEFAULT_LABEL_GAP)
        .align_items(Alignment::Center),
        pick_list(
            FontStyleOption::list(),
            Some(app.draft.drawing_font_style_option),
            Message::FontStyleOptionSelected,
        )
        .width(Length::Fill),
    ]
    .spacing(6);

    if app.draft.drawing_font_style_option == FontStyleOption::Custom {
        style_column = style_column.push(labeled_input(
            "Custom style",
            &app.draft.drawing_font_style,
            &app.defaults.drawing_font_style,
            TextField::DrawingFontStyle,
        ));
    }

    row![
        labeled_input(
            "Font family",
            &app.draft.drawing_font_family,
            &app.defaults.drawing_font_family,
            TextField::DrawingFontFamily,
        ),
        weight_column,
        style_column,
    ]
    .spacing(12)
    .into()
}
