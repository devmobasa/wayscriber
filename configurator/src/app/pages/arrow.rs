//! Arrow page: arrowhead geometry for the arrow tool.
//!
//! The two numeric fields keep the core config's documented ranges. The
//! Iced view showed those ranges as always-on hint text under each input;
//! an `AdwEntryRow` has no hint slot, so the range appears in the row's
//! error text instead, on the same `.error` styling every ported page uses.

use relm4::prelude::*;

use crate::messages::Message;
use crate::models::util::format_float;
use crate::models::{ArrowStyleOption, TabId, TextField, ToggleField};

use super::super::search::SearchArea;
use super::super::state::ConfiguratorApp;
use super::{BuiltPage, PageBuilder};

/// `ArrowConfig::length`, in pixels.
const LENGTH_RANGE: (f64, f64) = (5.0, 50.0);
/// `ArrowConfig::angle_degrees`, in degrees.
const ANGLE_RANGE: (f64, f64) = (15.0, 60.0);

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Arrow);

    page.group_in_area("Arrow Settings", SearchArea::Arrow)
        .entry_row_validated(
            "Arrow length (px)",
            |app| app.draft.arrow_length.clone(),
            |value| Message::TextChanged(TextField::ArrowLength, value),
            |app| validate_f64_range(&app.draft.arrow_length, LENGTH_RANGE.0, LENGTH_RANGE.1),
        )
        .entry_row_validated(
            "Arrow angle (deg)",
            |app| app.draft.arrow_angle.clone(),
            |value| Message::TextChanged(TextField::ArrowAngle, value),
            |app| validate_f64_range(&app.draft.arrow_angle, ANGLE_RANGE.0, ANGLE_RANGE.1),
        )
        .switch_row(
            "Place arrowhead at end of line",
            "Off draws the head at the start of the line instead.",
            |app| app.draft.arrow_head_at_end,
            |value| Message::ToggleChanged(ToggleField::ArrowHeadAtEnd, value),
        )
        .combo_row(
            "Arrow style",
            "Shape of the next arrow drawn. Every arrow keeps its own style, so \
             changing this never restyles existing drawings.",
            ArrowStyleOption::list(),
            ArrowStyleOption::list()
                .iter()
                .map(|option| option.label().to_string())
                .collect(),
            |app| app.draft.arrow_style,
            Message::ArrowStyleChanged,
        );

    page.finish()
}

/// Error text for a numeric field constrained to `min..=max`, `None` while
/// the input is acceptable.
fn validate_f64_range(value: &str, min: f64, max: f64) -> Option<String> {
    match value.trim().parse::<f64>() {
        Ok(parsed) if (min..=max).contains(&parsed) => None,
        _ => Some(format!(
            "Enter a number between {} and {}.",
            format_float(min),
            format_float(max)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_f64_range_accepts_the_bounds() {
        assert_eq!(validate_f64_range("5", 5.0, 50.0), None);
        assert_eq!(validate_f64_range(" 50 ", 5.0, 50.0), None);
        assert_eq!(validate_f64_range("26.5", 15.0, 60.0), None);
    }

    #[test]
    fn validate_f64_range_reports_the_range_for_bad_input() {
        let expected = Some("Enter a number between 5 and 50.".to_string());
        assert_eq!(validate_f64_range("4.9", 5.0, 50.0), expected);
        assert_eq!(validate_f64_range("", 5.0, 50.0), expected);
        assert_eq!(validate_f64_range("wide", 5.0, 50.0), expected);
    }
}
