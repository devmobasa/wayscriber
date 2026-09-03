use super::Config;
use crate::config::{ARROW_ANGLE_MAX, ARROW_ANGLE_MIN, ARROW_LENGTH_MAX, ARROW_LENGTH_MIN};

impl Config {
    pub(super) fn validate_arrow(&mut self) {
        if !(ARROW_LENGTH_MIN..=ARROW_LENGTH_MAX).contains(&self.arrow.length) {
            log::warn!(
                "Invalid arrow length {:.1}, clamping to {ARROW_LENGTH_MIN:.1}-{ARROW_LENGTH_MAX:.1} range",
                self.arrow.length
            );
            self.arrow.length = self.arrow.length.clamp(ARROW_LENGTH_MIN, ARROW_LENGTH_MAX);
        }

        if !(ARROW_ANGLE_MIN..=ARROW_ANGLE_MAX).contains(&self.arrow.angle_degrees) {
            log::warn!(
                "Invalid arrow angle {:.1} deg, clamping to {ARROW_ANGLE_MIN:.1}-{ARROW_ANGLE_MAX:.1} deg range",
                self.arrow.angle_degrees
            );
            self.arrow.angle_degrees = self
                .arrow
                .angle_degrees
                .clamp(ARROW_ANGLE_MIN, ARROW_ANGLE_MAX);
        }
    }
}
