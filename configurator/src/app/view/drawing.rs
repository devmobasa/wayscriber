use iced::theme;
use iced::widget::{Column, Row, button, column, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{
    ColorMode, EraserModeOption, FontStyleOption, FontWeightOption, NamedColorOption, TextField,
    ToggleField, TripletField,
};

use super::super::state::ConfiguratorApp;
use super::widgets::{
    COLOR_PICKER_WIDTH, DEFAULT_LABEL_GAP, color_preview_labeled, default_value_text,
    labeled_control, labeled_input, labeled_input_with_feedback, toggle_row, validate_f64_range,
    validate_usize_min, validate_usize_range,
};

impl ConfiguratorApp {
    pub(super) fn drawing_tab(&self) -> Element<'_, Message> {
        let color_mode_picker = Row::new()
            .spacing(12)
            .push(
                button("Named Color")
                    .style(if self.draft.drawing_color.mode == ColorMode::Named {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::ColorModeChanged(ColorMode::Named)),
            )
            .push(
                button("RGB Color")
                    .style(if self.draft.drawing_color.mode == ColorMode::Rgb {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::ColorModeChanged(ColorMode::Rgb)),
            );

        let color_section: Element<'_, Message> = match self.draft.drawing_color.mode {
            ColorMode::Named => {
                let picker = pick_list(
                    NamedColorOption::list(),
                    Some(self.draft.drawing_color.selected_named),
                    Message::NamedColorSelected,
                )
                .width(Length::Fixed(COLOR_PICKER_WIDTH));

                let picker_row = row![
                    picker,
                    color_preview_labeled(self.draft.drawing_color.preview_color()),
                ]
                .spacing(8)
                .align_items(iced::Alignment::Center);

                let mut column = Column::new().spacing(8).push(picker_row);

                if self.draft.drawing_color.selected_named_is_custom() {
                    column = column.push(
                        text_input("Custom color name", &self.draft.drawing_color.name)
                            .on_input(|value| {
                                Message::TextChanged(TextField::DrawingColorName, value)
                            })
                            .width(Length::Fill),
                    );

                    if self.draft.drawing_color.preview_color().is_none()
                        && !self.draft.drawing_color.name.trim().is_empty()
                    {
                        column = column.push(
                            text("Unknown color name")
                                .size(12)
                                .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.6, 0.6))),
                        );
                    }
                }

                column.into()
            }
            ColorMode::Rgb => {
                let rgb_inputs = row![
                    text_input("R (0-255)", &self.draft.drawing_color.rgb[0]).on_input(|value| {
                        Message::TripletChanged(TripletField::DrawingColorRgb, 0, value)
                    }),
                    text_input("G (0-255)", &self.draft.drawing_color.rgb[1]).on_input(|value| {
                        Message::TripletChanged(TripletField::DrawingColorRgb, 1, value)
                    }),
                    text_input("B (0-255)", &self.draft.drawing_color.rgb[2]).on_input(|value| {
                        Message::TripletChanged(TripletField::DrawingColorRgb, 2, value)
                    }),
                    color_preview_labeled(self.draft.drawing_color.preview_color()),
                ]
                .spacing(8)
                .align_items(iced::Alignment::Center);

                let mut column = Column::new().spacing(8).push(rgb_inputs);

                if self.draft.drawing_color.preview_color().is_none()
                    && self
                        .draft
                        .drawing_color
                        .rgb
                        .iter()
                        .any(|value| !value.trim().is_empty())
                {
                    column = column.push(
                        text("RGB values must be between 0 and 255")
                            .size(12)
                            .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.6, 0.6))),
                    );
                }

                column.into()
            }
        };

        let color_block = column![
            row![
                text("Pen color").size(14),
                default_value_text(
                    self.defaults.drawing_color.summary(),
                    self.draft.drawing_color != self.defaults.drawing_color,
                ),
            ]
            .spacing(DEFAULT_LABEL_GAP)
            .align_items(iced::Alignment::Center),
            color_mode_picker,
            color_section
        ]
        .spacing(8);

        let eraser_mode_pick = pick_list(
            EraserModeOption::list(),
            Some(self.draft.drawing_default_eraser_mode),
            Message::EraserModeChanged,
        );

        let column = column![
            text("Drawing Defaults").size(20),
            color_block,
            row![
                labeled_input_with_feedback(
                    "Thickness (px)",
                    &self.draft.drawing_default_thickness,
                    &self.defaults.drawing_default_thickness,
                    TextField::DrawingThickness,
                    Some("Range: 1-50 px"),
                    validate_f64_range(&self.draft.drawing_default_thickness, 1.0, 50.0),
                ),
                labeled_input_with_feedback(
                    "Font size (pt)",
                    &self.draft.drawing_default_font_size,
                    &self.defaults.drawing_default_font_size,
                    TextField::DrawingFontSize,
                    Some("Range: 8-72 pt"),
                    validate_f64_range(&self.draft.drawing_default_font_size, 8.0, 72.0),
                )
            ]
            .spacing(12),
            row![
                labeled_input_with_feedback(
                    "Eraser size (px)",
                    &self.draft.drawing_default_eraser_size,
                    &self.defaults.drawing_default_eraser_size,
                    TextField::DrawingEraserSize,
                    Some("Range: 1-50 px"),
                    validate_f64_range(&self.draft.drawing_default_eraser_size, 1.0, 50.0),
                ),
                labeled_control(
                    "Eraser mode",
                    eraser_mode_pick.width(Length::Fill).into(),
                    self.defaults
                        .drawing_default_eraser_mode
                        .label()
                        .to_string(),
                    self.draft.drawing_default_eraser_mode
                        != self.defaults.drawing_default_eraser_mode,
                )
            ]
            .spacing(12),
            row![
                labeled_input_with_feedback(
                    "Marker opacity (0.05-0.9)",
                    &self.draft.drawing_marker_opacity,
                    &self.defaults.drawing_marker_opacity,
                    TextField::DrawingMarkerOpacity,
                    None,
                    validate_f64_range(&self.draft.drawing_marker_opacity, 0.05, 0.9),
                ),
                labeled_input_with_feedback(
                    "Undo stack limit",
                    &self.draft.drawing_undo_stack_limit,
                    &self.defaults.drawing_undo_stack_limit,
                    TextField::DrawingUndoStackLimit,
                    Some("Range: 10-1000"),
                    validate_usize_range(&self.draft.drawing_undo_stack_limit, 10, 1000),
                )
            ]
            .spacing(12),
            row![
                labeled_input_with_feedback(
                    "Hit-test tolerance (px)",
                    &self.draft.drawing_hit_test_tolerance,
                    &self.defaults.drawing_hit_test_tolerance,
                    TextField::DrawingHitTestTolerance,
                    Some("Range: 1-20 px"),
                    validate_f64_range(&self.draft.drawing_hit_test_tolerance, 1.0, 20.0),
                ),
                labeled_input_with_feedback(
                    "Hit-test threshold",
                    &self.draft.drawing_hit_test_linear_threshold,
                    &self.defaults.drawing_hit_test_linear_threshold,
                    TextField::DrawingHitTestThreshold,
                    Some("Minimum: 1"),
                    validate_usize_min(&self.draft.drawing_hit_test_linear_threshold, 1),
                )
            ]
            .spacing(12),
            row![
                labeled_input(
                    "Font family",
                    &self.draft.drawing_font_family,
                    &self.defaults.drawing_font_family,
                    TextField::DrawingFontFamily,
                ),
                column![
                    row![
                        text("Font weight").size(14),
                        default_value_text(
                            self.defaults.drawing_font_weight.clone(),
                            self.draft.drawing_font_weight != self.defaults.drawing_font_weight,
                        )
                    ]
                    .spacing(DEFAULT_LABEL_GAP)
                    .align_items(iced::Alignment::Center),
                    pick_list(
                        FontWeightOption::list(),
                        Some(self.draft.drawing_font_weight_option),
                        Message::FontWeightOptionSelected,
                    )
                    .width(Length::Fill),
                    labeled_input(
                        "Custom or numeric weight",
                        &self.draft.drawing_font_weight,
                        &self.defaults.drawing_font_weight,
                        TextField::DrawingFontWeight,
                    )
                ]
                .spacing(6),
                {
                    let mut column = column![
                        row![
                            text("Font style").size(14),
                            default_value_text(
                                self.defaults.drawing_font_style.clone(),
                                self.draft.drawing_font_style != self.defaults.drawing_font_style,
                            )
                        ]
                        .spacing(DEFAULT_LABEL_GAP)
                        .align_items(iced::Alignment::Center),
                        pick_list(
                            FontStyleOption::list(),
                            Some(self.draft.drawing_font_style_option),
                            Message::FontStyleOptionSelected,
                        )
                        .width(Length::Fill),
                    ]
                    .spacing(6);

                    if self.draft.drawing_font_style_option == FontStyleOption::Custom {
                        column = column.push(labeled_input(
                            "Custom style",
                            &self.draft.drawing_font_style,
                            &self.defaults.drawing_font_style,
                            TextField::DrawingFontStyle,
                        ));
                    }

                    column
                }
            ]
            .spacing(12),
            toggle_row(
                "Enable text background",
                self.draft.drawing_text_background_enabled,
                self.defaults.drawing_text_background_enabled,
                ToggleField::DrawingTextBackground,
            ),
            toggle_row(
                "Start shapes filled",
                self.draft.drawing_default_fill_enabled,
                self.defaults.drawing_default_fill_enabled,
                ToggleField::DrawingFillEnabled,
            )
        ]
        .spacing(12)
        .width(Length::Fill);

        scrollable(column).into()
    }
}
