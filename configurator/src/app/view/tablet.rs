use iced::Element;
use iced::widget::{column, row, scrollable, text};

use crate::messages::Message;
use crate::models::{TextField, ToggleField};

use super::super::state::ConfiguratorApp;
use super::widgets::{labeled_input_with_feedback, toggle_row, validate_f64_range};

impl ConfiguratorApp {
    pub(super) fn tablet_tab(&self) -> Element<'_, Message> {
        scrollable(
            column![
                text("Tablet / Stylus").size(20),
                toggle_row(
                    "Enable tablet input",
                    self.draft.tablet_enabled,
                    self.defaults.tablet_enabled,
                    ToggleField::TabletEnabled,
                ),
                toggle_row(
                    "Enable pressure-to-thickness",
                    self.draft.tablet_pressure_enabled,
                    self.defaults.tablet_pressure_enabled,
                    ToggleField::TabletPressureEnabled,
                ),
                row![
                    labeled_input_with_feedback(
                        "Min thickness",
                        &self.draft.tablet_min_thickness,
                        &self.defaults.tablet_min_thickness,
                        TextField::TabletMinThickness,
                        Some("Range: 1-50"),
                        validate_f64_range(&self.draft.tablet_min_thickness, 1.0, 50.0),
                    ),
                    labeled_input_with_feedback(
                        "Max thickness",
                        &self.draft.tablet_max_thickness,
                        &self.defaults.tablet_max_thickness,
                        TextField::TabletMaxThickness,
                        Some("Range: 1-50"),
                        validate_f64_range(&self.draft.tablet_max_thickness, 1.0, 50.0),
                    )
                ]
                .spacing(12),
            ]
            .spacing(12),
        )
        .into()
    }
}
