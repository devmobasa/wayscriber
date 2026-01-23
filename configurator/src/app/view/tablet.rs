use iced::widget::{column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{
    PressureThicknessEditModeOption, PressureThicknessEntryModeOption, TextField, ToggleField,
};

use super::super::state::ConfiguratorApp;
use super::widgets::{
    labeled_control, labeled_input_with_feedback, toggle_row, validate_f64_range,
};

impl ConfiguratorApp {
    pub(super) fn tablet_tab(&self) -> Element<'_, Message> {
        let edit_mode = pick_list(
            PressureThicknessEditModeOption::list(),
            Some(self.draft.tablet_pressure_thickness_edit_mode),
            Message::TabletPressureEditModeChanged,
        );
        let entry_mode = pick_list(
            PressureThicknessEntryModeOption::list(),
            Some(self.draft.tablet_pressure_thickness_entry_mode),
            Message::TabletPressureEntryModeChanged,
        );
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
                toggle_row(
                    "Auto-switch to eraser",
                    self.draft.tablet_auto_eraser_switch,
                    self.defaults.tablet_auto_eraser_switch,
                    ToggleField::TabletAutoEraserSwitch,
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
                labeled_input_with_feedback(
                    "Pressure variation threshold",
                    &self.draft.tablet_pressure_variation_threshold,
                    &self.defaults.tablet_pressure_variation_threshold,
                    TextField::TabletPressureVariationThreshold,
                    Some("Minimum: 0"),
                    None,
                ),
                labeled_input_with_feedback(
                    "Pressure thickness scale step",
                    &self.draft.tablet_pressure_thickness_scale_step,
                    &self.defaults.tablet_pressure_thickness_scale_step,
                    TextField::TabletPressureScaleStep,
                    Some("Range: 0-1"),
                    validate_f64_range(&self.draft.tablet_pressure_thickness_scale_step, 0.0, 1.0),
                ),
                labeled_control(
                    "Pressure thickness edit mode",
                    edit_mode.width(Length::Fill).into(),
                    self.defaults
                        .tablet_pressure_thickness_edit_mode
                        .label()
                        .to_string(),
                    self.draft.tablet_pressure_thickness_edit_mode
                        != self.defaults.tablet_pressure_thickness_edit_mode,
                ),
                labeled_control(
                    "Pressure thickness entry mode",
                    entry_mode.width(Length::Fill).into(),
                    self.defaults
                        .tablet_pressure_thickness_entry_mode
                        .label()
                        .to_string(),
                    self.draft.tablet_pressure_thickness_entry_mode
                        != self.defaults.tablet_pressure_thickness_entry_mode,
                ),
            ]
            .spacing(12),
        )
        .into()
    }
}
