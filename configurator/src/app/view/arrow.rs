use iced::Element;
use iced::widget::{column, row, scrollable, text};

use crate::messages::Message;
use crate::models::{TextField, ToggleField};

use super::super::state::ConfiguratorApp;
use super::widgets::{labeled_input_with_feedback, toggle_row, validate_f64_range};

impl ConfiguratorApp {
    pub(super) fn arrow_tab(&self) -> Element<'_, Message> {
        scrollable(
            column![
                text("Arrow Settings").size(20),
                row![
                    labeled_input_with_feedback(
                        "Arrow length (px)",
                        &self.draft.arrow_length,
                        &self.defaults.arrow_length,
                        TextField::ArrowLength,
                        Some("Range: 5-50 px"),
                        validate_f64_range(&self.draft.arrow_length, 5.0, 50.0),
                    ),
                    labeled_input_with_feedback(
                        "Arrow angle (deg)",
                        &self.draft.arrow_angle,
                        &self.defaults.arrow_angle,
                        TextField::ArrowAngle,
                        Some("Range: 15-60 deg"),
                        validate_f64_range(&self.draft.arrow_angle, 15.0, 60.0),
                    )
                ]
                .spacing(12),
                toggle_row(
                    "Place arrowhead at end of line",
                    self.draft.arrow_head_at_end,
                    self.defaults.arrow_head_at_end,
                    ToggleField::ArrowHeadAtEnd,
                )
            ]
            .spacing(12),
        )
        .into()
    }
}
