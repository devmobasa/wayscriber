use super::*;

pub(super) fn build(page: &mut PageBuilder) {
    page.group_in_area("Drawing defaults", SearchArea::DrawingDefaults)
        .entry_row_validated(
            "Thickness (px)",
            |app| app.draft.drawing_default_thickness.clone(),
            |value| Message::TextChanged(TextField::DrawingThickness, value),
            |app| validate_f64_range(&app.draft.drawing_default_thickness, 1.0, 50.0),
        )
        .entry_row_validated(
            "Font size (pt)",
            |app| app.draft.drawing_default_font_size.clone(),
            |value| Message::TextChanged(TextField::DrawingFontSize, value),
            |app| validate_f64_range(&app.draft.drawing_default_font_size, 8.0, 72.0),
        )
        .entry_row_validated(
            "Polygon sides",
            |app| app.draft.drawing_polygon_sides.clone(),
            |value| Message::TextChanged(TextField::DrawingPolygonSides, value),
            |app| validate_usize_range(&app.draft.drawing_polygon_sides, 3, 12),
        )
        .entry_row_validated(
            "Eraser size (px)",
            |app| app.draft.drawing_default_eraser_size.clone(),
            |value| Message::TextChanged(TextField::DrawingEraserSize, value),
            |app| validate_f64_range(&app.draft.drawing_default_eraser_size, 1.0, 50.0),
        )
        .combo_row(
            "Eraser mode",
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
            "Marker opacity (0.05-0.9)",
            |app| app.draft.drawing_marker_opacity.clone(),
            |value| Message::TextChanged(TextField::DrawingMarkerOpacity, value),
            |app| validate_f64_range(&app.draft.drawing_marker_opacity, 0.05, 0.9),
        )
        .entry_row_validated(
            "Undo stack limit",
            |app| app.draft.drawing_undo_stack_limit.clone(),
            |value| Message::TextChanged(TextField::DrawingUndoStackLimit, value),
            |app| validate_usize_range(&app.draft.drawing_undo_stack_limit, 10, 1000),
        )
        .entry_row_validated(
            "Hit-test tolerance (px)",
            |app| app.draft.drawing_hit_test_tolerance.clone(),
            |value| Message::TextChanged(TextField::DrawingHitTestTolerance, value),
            |app| validate_f64_range(&app.draft.drawing_hit_test_tolerance, 1.0, 20.0),
        )
        .entry_row_validated(
            "Hit-test threshold",
            |app| app.draft.drawing_hit_test_linear_threshold.clone(),
            |value| Message::TextChanged(TextField::DrawingHitTestThreshold, value),
            |app| validate_usize_min(&app.draft.drawing_hit_test_linear_threshold, 1),
        )
        .switch_row(
            "Enable text background",
            "",
            |app| app.draft.drawing_text_background_enabled,
            |value| Message::ToggleChanged(ToggleField::DrawingTextBackground, value),
        )
        .switch_row(
            "Start shapes filled",
            "",
            |app| app.draft.drawing_default_fill_enabled,
            |value| Message::ToggleChanged(ToggleField::DrawingFillEnabled, value),
        );
}
