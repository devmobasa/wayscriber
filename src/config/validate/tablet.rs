use super::{Config, float::finite_or_default};
use crate::domain::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

impl Config {
    pub(super) fn validate_tablet(&mut self) {
        let defaults = crate::config::TabletInputConfig::default();
        for (field, value, fallback) in [
            (
                "tablet.min_thickness",
                &mut self.tablet.min_thickness,
                defaults.min_thickness,
            ),
            (
                "tablet.max_thickness",
                &mut self.tablet.max_thickness,
                defaults.max_thickness,
            ),
            (
                "tablet.pressure_variation_threshold",
                &mut self.tablet.pressure_variation_threshold,
                defaults.pressure_variation_threshold,
            ),
            (
                "tablet.pressure_thickness_scale_step",
                &mut self.tablet.pressure_thickness_scale_step,
                defaults.pressure_thickness_scale_step,
            ),
        ] {
            *value = finite_or_default(*value, fallback, field);
        }

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

        if self.tablet.pressure_variation_threshold < 0.0 {
            self.tablet.pressure_variation_threshold = 0.0;
        }
        self.tablet.pressure_thickness_scale_step =
            self.tablet.pressure_thickness_scale_step.clamp(0.0, 1.0);
    }
}
