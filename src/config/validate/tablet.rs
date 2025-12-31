use super::Config;
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

impl Config {
    pub(super) fn validate_tablet(&mut self) {
        if self.tablet.min_thickness > self.tablet.max_thickness {
            std::mem::swap(
                &mut self.tablet.min_thickness,
                &mut self.tablet.max_thickness,
            );
        }
        self.tablet.min_thickness = self
            .tablet
            .min_thickness
            .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        self.tablet.max_thickness = self
            .tablet
            .max_thickness
            .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
    }
}
