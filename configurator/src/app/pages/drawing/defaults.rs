use wayscriber::i18n::tr;

use crate::messages::Message;
use crate::models::{EraserModeOption, TextField, ToggleField};

use super::super::super::search::SearchArea;
use super::super::PageBuilder;
use super::{validate_f64_range, validate_usize_min, validate_usize_range};


pub(super) fn build(page: &mut PageBuilder) {
    page.group_in_area(tr("Drawing defaults"), SearchArea::DrawingDefaults)
        .entry_row_validated(
            tr("Thickness (px)"),
            |app| app.draft.drawing_default_thickness.clone(),
            |value| Message::TextChanged(TextField::DrawingThickness, value),
            |app| validate_f64_range(&app.draft.drawing_default_thickness, 1.0, 50.0),
        )
        .entry_row_validated(
            tr("Font size (pt)"),
            |app| app.draft.drawing_default_font_size.clone(),
            |value| Message::TextChanged(TextField::DrawingFontSize, value),
            |app| validate_f64_range(&app.draft.drawing_default_font_size, 8.0, 72.0),
        )
        .entry_row_validated(
            tr("Polygon sides"),
            |app| app.draft.drawing_polygon_sides.clone(),
            |value| Message::TextChanged(TextField::DrawingPolygonSides, value),
            |app| validate_usize_range(&app.draft.drawing_polygon_sides, 3, 12),
        )
        .entry_row_validated(
            tr("Eraser size (px)"),
            |app| app.draft.drawing_default_eraser_size.clone(),
            |value| Message::TextChanged(TextField::DrawingEraserSize, value),
            |app| validate_f64_range(&app.draft.drawing_default_eraser_size, 1.0, 50.0),
        )
        .combo_row(
            tr("Eraser mode"),
            "",
            EraserModeOption::list(),
            EraserModeOption::list()
                .iter()
                .map(|option| option.label().to_string())
                .collect(),
            |app| app.draft.drawing_default_eraser_mode,
            Message::EraserModeChanged,
        )
        .entry_row_validated(
            tr("Marker opacity (0.05-0.9)"),
            |app| app.draft.drawing_marker_opacity.clone(),
            |value| Message::TextChanged(TextField::DrawingMarkerOpacity, value),
            |app| validate_f64_range(&app.draft.drawing_marker_opacity, 0.05, 0.9),
        )
        .entry_row_validated(
            tr("Pen smoothing (0-6)"),
            |app| app.draft.drawing_pen_smoothing.clone(),
            |value| Message::TextChanged(TextField::DrawingPenSmoothing, value),
            |app| validate_usize_range(&app.draft.drawing_pen_smoothing, 0, 6),
        )
        .entry_row_validated(
            tr("Shape Pen sensitivity (0-4)"),
            |app| app.draft.drawing_shape_recognition_sensitivity.clone(),
            |value| Message::TextChanged(TextField::DrawingShapeRecognitionSensitivity, value),
            |app| {
                validate_usize_range(
                    &app.draft.drawing_shape_recognition_sensitivity,
                    0,
                    usize::from(wayscriber::config::MAX_SHAPE_RECOGNITION_SENSITIVITY),
                )
            },
        )
        .switch_row(
            tr("Snap Shape Pen to board paper"),
            tr("Recognized lines, rectangles, ovals, and triangles line up with nearby grid lines and points"),
            |app| app.draft.drawing_shape_recognition_grid_snap,
            |value| Message::ToggleChanged(ToggleField::DrawingShapeRecognitionGridSnap, value),
        )
        .switch_row(
            tr("Name recognized Shape Pen shapes"),
            tr("A brief chip beside each recognized shape names it and the undo shortcut that keeps the ink"),
            |app| app.draft.drawing_shape_recognition_feedback,
            |value| Message::ToggleChanged(ToggleField::DrawingShapeRecognitionFeedback, value),
        )
        .entry_row_validated(
            tr("Undo stack limit"),
            |app| app.draft.drawing_undo_stack_limit.clone(),
            |value| Message::TextChanged(TextField::DrawingUndoStackLimit, value),
            |app| validate_usize_range(&app.draft.drawing_undo_stack_limit, 10, 1000),
        )
        .entry_row_validated(
            tr("Hit-test tolerance (px)"),
            |app| app.draft.drawing_hit_test_tolerance.clone(),
            |value| Message::TextChanged(TextField::DrawingHitTestTolerance, value),
            |app| validate_f64_range(&app.draft.drawing_hit_test_tolerance, 1.0, 20.0),
        )
        .entry_row_validated(
            tr("Hit-test threshold"),
            |app| app.draft.drawing_hit_test_linear_threshold.clone(),
            |value| Message::TextChanged(TextField::DrawingHitTestThreshold, value),
            |app| validate_usize_min(&app.draft.drawing_hit_test_linear_threshold, 1),
        )
        .switch_row(
            tr("Enable text background"),
            "",
            |app| app.draft.drawing_text_background_enabled,
            |value| Message::ToggleChanged(ToggleField::DrawingTextBackground, value),
        )
        .switch_row(
            tr("Enable text halo"),
            tr("Draw a contrasting outline around text"),
            |app| app.draft.drawing_text_halo_enabled,
            |value| Message::ToggleChanged(ToggleField::DrawingTextHalo, value),
        )
        .switch_row(
            tr("Start shapes filled"),
            "",
            |app| app.draft.drawing_default_fill_enabled,
            |value| Message::ToggleChanged(ToggleField::DrawingFillEnabled, value),
        );
}
