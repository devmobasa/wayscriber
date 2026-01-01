mod color;
mod font;

use iced::widget::{column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{EraserModeOption, TextField, ToggleField};

use self::color::drawing_color_block;
use self::font::font_controls;
use super::super::state::ConfiguratorApp;
use super::widgets::{
    labeled_control, labeled_input_with_feedback, toggle_row, validate_f64_range,
    validate_usize_min, validate_usize_range,
};

impl ConfiguratorApp {
    pub(super) fn drawing_tab(&self) -> Element<'_, Message> {
        let eraser_mode_pick = pick_list(
            EraserModeOption::list(),
            Some(self.draft.drawing_default_eraser_mode),
            Message::EraserModeChanged,
        );

        let column = column![
            text("Drawing Defaults").size(20),
            drawing_color_block(self),
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
            font_controls(self),
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
