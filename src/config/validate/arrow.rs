use super::Config;

impl Config {
    pub(super) fn validate_arrow(&mut self) {
        // Arrow length: 5.0 - 50.0
        if !(5.0..=50.0).contains(&self.arrow.length) {
            log::warn!(
                "Invalid arrow length {:.1}, clamping to 5.0-50.0 range",
                self.arrow.length
            );
            self.arrow.length = self.arrow.length.clamp(5.0, 50.0);
        }

        // Arrow angle: 15.0 - 60.0 degrees
        if !(15.0..=60.0).contains(&self.arrow.angle_degrees) {
            log::warn!(
                "Invalid arrow angle {:.1} deg, clamping to 15.0-60.0 deg range",
                self.arrow.angle_degrees
            );
            self.arrow.angle_degrees = self.arrow.angle_degrees.clamp(15.0, 60.0);
        }
    }
}
