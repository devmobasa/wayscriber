use super::Config;

impl Config {
    pub(super) fn validate_spotlight(&mut self) {
        // Dim opacity: 0.1 - 0.95. Below 0.1 the spotlight is invisible; above
        // 0.95 the surrounding screen is effectively blacked out.
        if !(0.1..=0.95).contains(&self.spotlight.dim_opacity) {
            log::warn!(
                "Invalid spotlight dim opacity {:.2}, clamping to 0.1-0.95 range",
                self.spotlight.dim_opacity
            );
            self.spotlight.dim_opacity = self.spotlight.dim_opacity.clamp(0.1, 0.95);
        }

        // Feather: 0.0 - 0.9. Past 0.9 the falloff consumes the bright centre.
        if !(0.0..=0.9).contains(&self.spotlight.feather) {
            log::warn!(
                "Invalid spotlight feather {:.2}, clamping to 0.0-0.9 range",
                self.spotlight.feather
            );
            self.spotlight.feather = self.spotlight.feather.clamp(0.0, 0.9);
        }
    }
}
