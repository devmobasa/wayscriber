//! Tablet page: stylus input and the pressure-to-thickness mapping.
//!
//! Compiled only with the `tablet-input` feature, the same gate the draft
//! fields and both pressure messages sit behind.

use relm4::prelude::*;

use crate::messages::Message;
use crate::models::{
    PressureThicknessEditModeOption, PressureThicknessEntryModeOption, TabId, TextField,
    ToggleField,
};

use super::super::search::SearchArea;
use super::super::state::ConfiguratorApp;
use super::capture::validate_f64_range;
use super::{BuiltPage, PageBuilder};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let edit_modes = PressureThicknessEditModeOption::list();
    let edit_mode_labels = edit_modes
        .iter()
        .map(|mode| mode.label().to_string())
        .collect();
    let entry_modes = PressureThicknessEntryModeOption::list();
    let entry_mode_labels = entry_modes
        .iter()
        .map(|mode| mode.label().to_string())
        .collect();

    let mut page = PageBuilder::new(sender, TabId::Tablet);

    page.group_in_area("Stylus", SearchArea::Tablet)
        .switch_row(
            "Enable tablet input",
            "",
            |app| app.draft.tablet_enabled,
            |value| Message::ToggleChanged(ToggleField::TabletEnabled, value),
        )
        .switch_row(
            "Enable pressure-to-thickness",
            "Freehand Pen strokes only",
            |app| app.draft.tablet_pressure_enabled,
            |value| Message::ToggleChanged(ToggleField::TabletPressureEnabled, value),
        )
        .switch_row(
            "Auto-switch to eraser",
            "",
            |app| app.draft.tablet_auto_eraser_switch,
            |value| Message::ToggleChanged(ToggleField::TabletAutoEraserSwitch, value),
        );

    page.group_in_area("Pressure", SearchArea::Tablet)
        .entry_row_validated(
            "Min thickness",
            |app| app.draft.tablet_min_thickness.clone(),
            |value| Message::TextChanged(TextField::TabletMinThickness, value),
            |app| validate_f64_range(&app.draft.tablet_min_thickness, 1.0, 50.0),
        )
        .entry_row_validated(
            "Max thickness",
            |app| app.draft.tablet_max_thickness.clone(),
            |value| Message::TextChanged(TextField::TabletMaxThickness, value),
            |app| validate_f64_range(&app.draft.tablet_max_thickness, 1.0, 50.0),
        )
        // The Iced view showed a "Minimum: 0" hint here but never marked the
        // field in error; the config clamps whatever arrives.
        .entry_row(
            "Pressure variation threshold",
            |app| app.draft.tablet_pressure_variation_threshold.clone(),
            |value| Message::TextChanged(TextField::TabletPressureVariationThreshold, value),
        )
        .entry_row_validated(
            "Pressure thickness scale step",
            |app| app.draft.tablet_pressure_thickness_scale_step.clone(),
            |value| Message::TextChanged(TextField::TabletPressureScaleStep, value),
            |app| validate_f64_range(&app.draft.tablet_pressure_thickness_scale_step, 0.0, 1.0),
        )
        .combo_row(
            "Pressure thickness edit mode",
            "",
            edit_modes,
            edit_mode_labels,
            |app| app.draft.tablet_pressure_thickness_edit_mode,
            Message::TabletPressureEditModeChanged,
        )
        .combo_row(
            "Pressure thickness entry mode",
            "",
            entry_modes,
            entry_mode_labels,
            |app| app.draft.tablet_pressure_thickness_entry_mode,
            Message::TabletPressureEntryModeChanged,
        );

    page.finish()
}
