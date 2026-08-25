use super::Config;
use crate::config::SpotlightConfig;

impl Config {
    pub(super) fn validate_spotlight(&mut self) {
        let defaults = SpotlightConfig::default();

        // Dim opacity: 0.1 - 0.95. Below 0.1 the spotlight is invisible; above
        // 0.95 the surrounding screen is effectively blacked out.
        if !self.spotlight.dim_opacity.is_finite() {
            log::warn!(
                "Non-finite spotlight dim opacity {:?}, resetting to {:.2}",
                self.spotlight.dim_opacity,
                defaults.dim_opacity
            );
            self.spotlight.dim_opacity = defaults.dim_opacity;
        } else if !(0.1..=0.95).contains(&self.spotlight.dim_opacity) {
            log::warn!(
                "Invalid spotlight dim opacity {:.2}, clamping to 0.1-0.95 range",
                self.spotlight.dim_opacity
            );
            self.spotlight.dim_opacity = self.spotlight.dim_opacity.clamp(0.1, 0.95);
        }

        // Feather: 0.0 - 0.9. Past 0.9 the falloff consumes the bright centre.
        if !self.spotlight.feather.is_finite() {
            log::warn!(
                "Non-finite spotlight feather {:?}, resetting to {:.2}",
                self.spotlight.feather,
                defaults.feather
            );
            self.spotlight.feather = defaults.feather;
        } else if !(0.0..=0.9).contains(&self.spotlight.feather) {
            log::warn!(
                "Invalid spotlight feather {:.2}, clamping to 0.0-0.9 range",
                self.spotlight.feather
            );
            self.spotlight.feather = self.spotlight.feather.clamp(0.0, 0.9);
        }

        if !self.spotlight.magnification.is_finite() {
            log::warn!(
                "Non-finite spotlight magnification {:?}, resetting to {:.2}",
                self.spotlight.magnification,
                defaults.magnification
            );
            self.spotlight.magnification = defaults.magnification;
        } else if !(crate::draw::MIN_SPOTLIGHT_MAGNIFICATION
            ..=crate::draw::MAX_SPOTLIGHT_MAGNIFICATION)
            .contains(&self.spotlight.magnification)
        {
            log::warn!(
                "Invalid spotlight magnification {:.2}, clamping to {:.1}-{:.1} range",
                self.spotlight.magnification,
                crate::draw::MIN_SPOTLIGHT_MAGNIFICATION,
                crate::draw::MAX_SPOTLIGHT_MAGNIFICATION
            );
            self.spotlight.magnification =
                crate::draw::normalize_spotlight_magnification(self.spotlight.magnification);
        }
    }
}
