use iced::Element;
use iced::widget::{column, row, scrollable, text};

use crate::app::state::ConfiguratorApp;
use crate::messages::Message;
use crate::models::{ColorPickerId, QuadField, TextField, ToggleField};

use super::super::widgets::{
    ColorPickerUi, color_quad_picker, labeled_input_with_feedback, toggle_row, validate_f64_range,
    validate_u64_range,
};

impl ConfiguratorApp {
    pub(super) fn ui_click_highlight_tab(&self) -> Element<'_, Message> {
        let column = column![
            text("Click Highlight").size(18),
            toggle_row(
                "Enable click highlight",
                self.draft.click_highlight_enabled,
                self.defaults.click_highlight_enabled,
                ToggleField::UiClickHighlightEnabled,
            ),
            toggle_row(
                "Link highlight color to current pen",
                self.draft.click_highlight_use_pen_color,
                self.defaults.click_highlight_use_pen_color,
                ToggleField::UiClickHighlightUsePenColor,
            ),
            row![
                labeled_input_with_feedback(
                    "Radius",
                    &self.draft.click_highlight_radius,
                    &self.defaults.click_highlight_radius,
                    TextField::HighlightRadius,
                    Some("Range: 16-160"),
                    validate_f64_range(&self.draft.click_highlight_radius, 16.0, 160.0),
                ),
                labeled_input_with_feedback(
                    "Outline thickness",
                    &self.draft.click_highlight_outline_thickness,
                    &self.defaults.click_highlight_outline_thickness,
                    TextField::HighlightOutlineThickness,
                    Some("Range: 1-12"),
                    validate_f64_range(&self.draft.click_highlight_outline_thickness, 1.0, 12.0,),
                ),
                labeled_input_with_feedback(
                    "Duration (ms)",
                    &self.draft.click_highlight_duration_ms,
                    &self.defaults.click_highlight_duration_ms,
                    TextField::HighlightDurationMs,
                    Some("Range: 150-1500 ms"),
                    validate_u64_range(&self.draft.click_highlight_duration_ms, 150, 1500),
                )
            ]
            .spacing(12),
            color_quad_picker(
                "Fill RGBA (0-1)",
                ColorPickerUi {
                    id: ColorPickerId::HighlightFill,
                    is_open: self.color_picker_open == Some(ColorPickerId::HighlightFill),
                    show_advanced: self
                        .color_picker_advanced
                        .contains(&ColorPickerId::HighlightFill),
                    hex_value: self
                        .color_picker_hex
                        .get(&ColorPickerId::HighlightFill)
                        .map(String::as_str)
                        .unwrap_or(""),
                },
                &self.draft.click_highlight_fill_color,
                &self.defaults.click_highlight_fill_color,
                QuadField::HighlightFill,
            ),
            color_quad_picker(
                "Outline RGBA (0-1)",
                ColorPickerUi {
                    id: ColorPickerId::HighlightOutline,
                    is_open: self.color_picker_open == Some(ColorPickerId::HighlightOutline),
                    show_advanced: self
                        .color_picker_advanced
                        .contains(&ColorPickerId::HighlightOutline),
                    hex_value: self
                        .color_picker_hex
                        .get(&ColorPickerId::HighlightOutline)
                        .map(String::as_str)
                        .unwrap_or(""),
                },
                &self.draft.click_highlight_outline_color,
                &self.defaults.click_highlight_outline_color,
                QuadField::HighlightOutline,
            )
        ]
        .spacing(12);

        scrollable(column).into()
    }
}
