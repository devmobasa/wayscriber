use super::Config;
use crate::config::types::DEFAULT_HIT_TEST_TOLERANCE;
use crate::draw::shape::{MAX_PEN_SMOOTHING, REGULAR_POLYGON_MAX_SIDES, REGULAR_POLYGON_MIN_SIDES};
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

impl Config {
    pub(super) fn validate_drawing(&mut self) {
        // Thickness: 1.0 - 50.0
        if !(MIN_STROKE_THICKNESS..=MAX_STROKE_THICKNESS).contains(&self.drawing.default_thickness)
        {
            log::warn!(
                "Invalid default_thickness {:.1}, clamping to {:.1}-{:.1} range",
                self.drawing.default_thickness,
                MIN_STROKE_THICKNESS,
                MAX_STROKE_THICKNESS
            );
            self.drawing.default_thickness = self
                .drawing
                .default_thickness
                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        }

        // Eraser size: 1.0 - 50.0
        if !(MIN_STROKE_THICKNESS..=MAX_STROKE_THICKNESS)
            .contains(&self.drawing.default_eraser_size)
        {
            log::warn!(
                "Invalid default_eraser_size {:.1}, clamping to {:.1}-{:.1} range",
                self.drawing.default_eraser_size,
                MIN_STROKE_THICKNESS,
                MAX_STROKE_THICKNESS
            );
            self.drawing.default_eraser_size = self
                .drawing
                .default_eraser_size
                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        }

        // Marker opacity: 0.05 - 0.9
        // Font cycle: drop blank entries and repeats, keeping the given order.
        // A repeat would make the action appear to skip, and a blank name would
        // resolve to whatever the font system falls back to.
        //
        // Repeats are judged without case, the way fontconfig resolves a family
        // name. `["Sans", "sans"]` is one font written twice, and keeping both
        // would leave a step that changes only the spelling.
        let mut seen = std::collections::BTreeSet::new();
        let before = self.drawing.font_cycle.len();
        self.drawing.font_cycle.retain(|family| {
            let trimmed = family.trim();
            !trimmed.is_empty() && seen.insert(trimmed.to_lowercase())
        });
        if self.drawing.font_cycle.len() != before {
            log::warn!(
                "Dropped {} blank or repeated entries from drawing.font_cycle",
                before - self.drawing.font_cycle.len()
            );
        }
        for family in &mut self.drawing.font_cycle {
            let trimmed = family.trim().to_string();
            *family = trimmed;
        }

        // Pen smoothing: 0 - MAX_PEN_SMOOTHING passes
        if self.drawing.pen_smoothing > MAX_PEN_SMOOTHING {
            log::warn!(
                "Invalid pen_smoothing {}, clamping to 0-{MAX_PEN_SMOOTHING} range",
                self.drawing.pen_smoothing
            );
            self.drawing.pen_smoothing = MAX_PEN_SMOOTHING;
        }

        if !(0.05..=0.9).contains(&self.drawing.marker_opacity) {
            log::warn!(
                "Invalid marker_opacity {:.2}, clamping to 0.05-0.90 range",
                self.drawing.marker_opacity
            );
            self.drawing.marker_opacity = self.drawing.marker_opacity.clamp(0.05, 0.9);
        }

        // Font size: 8.0 - 72.0
        if !(8.0..=72.0).contains(&self.drawing.default_font_size) {
            log::warn!(
                "Invalid default_font_size {:.1}, clamping to 8.0-72.0 range",
                self.drawing.default_font_size
            );
            self.drawing.default_font_size = self.drawing.default_font_size.clamp(8.0, 72.0);
        }

        if !(REGULAR_POLYGON_MIN_SIDES..=REGULAR_POLYGON_MAX_SIDES)
            .contains(&self.drawing.polygon_sides)
        {
            log::warn!(
                "Invalid polygon_sides {}, clamping to {}-{} range",
                self.drawing.polygon_sides,
                REGULAR_POLYGON_MIN_SIDES,
                REGULAR_POLYGON_MAX_SIDES
            );
            self.drawing.polygon_sides = self
                .drawing
                .polygon_sides
                .clamp(REGULAR_POLYGON_MIN_SIDES, REGULAR_POLYGON_MAX_SIDES);
        }

        if !self.drawing.hit_test_tolerance.is_finite() {
            log::warn!(
                "Invalid non-finite hit_test_tolerance; resetting to default {:.1}",
                DEFAULT_HIT_TEST_TOLERANCE
            );
            self.drawing.hit_test_tolerance = DEFAULT_HIT_TEST_TOLERANCE;
        } else if !(1.0..=20.0).contains(&self.drawing.hit_test_tolerance) {
            log::warn!(
                "Invalid hit_test_tolerance {:.1}, clamping to 1.0-20.0 range",
                self.drawing.hit_test_tolerance
            );
            self.drawing.hit_test_tolerance = self.drawing.hit_test_tolerance.clamp(1.0, 20.0);
        }

        if self.drawing.hit_test_linear_threshold == 0 {
            log::warn!("hit_test_linear_threshold must be at least 1; using default 400");
            self.drawing.hit_test_linear_threshold = 400;
        }

        if !(10..=1000).contains(&self.drawing.undo_stack_limit) {
            log::warn!(
                "Invalid undo_stack_limit {}, clamping to 10-1000 range",
                self.drawing.undo_stack_limit
            );
            self.drawing.undo_stack_limit = self.drawing.undo_stack_limit.clamp(10, 1000);
        }
    }
}
