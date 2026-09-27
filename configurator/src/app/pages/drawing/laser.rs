//! Laser pointer group: the color, width, and timing of fading presenter ink.

use wayscriber::config::{LASER_FADE_MS_MAX, LASER_HOLD_MS_MAX, LASER_WIDTH_MAX, LASER_WIDTH_MIN};

use crate::messages::Message;
use crate::models::color::parse_quad_values;
use crate::models::{ColorPickerId, TextField};

use super::super::super::search::SearchArea;
use super::super::PageBuilder;
use super::super::color_rows::color_row;
use super::{validate_f64_range, validate_usize_range};

pub(super) fn build(page: &mut PageBuilder) {
    page.group_in_area("Laser pointer", SearchArea::DrawingLaser);
    color_row(page, "Ink color (hex)", ColorPickerId::LaserColor, |app| {
        let [r, g, b, a] = parse_quad_values(&app.draft.laser_color.components);
        Some((r, g, b, a))
    });
    page.entry_row_validated(
        "Width (px)",
        |app| app.draft.laser_width.clone(),
        |value| Message::TextChanged(TextField::LaserWidth, value),
        |app| validate_f64_range(&app.draft.laser_width, LASER_WIDTH_MIN, LASER_WIDTH_MAX),
    )
    .entry_row_validated(
        "Stay visible after release (ms)",
        |app| app.draft.laser_hold_ms.clone(),
        |value| Message::TextChanged(TextField::LaserHoldMs, value),
        |app| validate_usize_range(&app.draft.laser_hold_ms, 0, LASER_HOLD_MS_MAX as usize),
    )
    .entry_row_validated(
        "Fade out (ms)",
        |app| app.draft.laser_fade_ms.clone(),
        |value| Message::TextChanged(TextField::LaserFadeMs, value),
        |app| validate_usize_range(&app.draft.laser_fade_ms, 0, LASER_FADE_MS_MAX as usize),
    );
}
