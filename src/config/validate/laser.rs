use super::Config;
use crate::config::LaserConfig;
use crate::config::types::{
    LASER_FADE_MS_MAX, LASER_HOLD_MS_MAX, LASER_WIDTH_MAX, LASER_WIDTH_MIN,
};

impl Config {
    pub(super) fn validate_laser(&mut self) {
        let defaults = LaserConfig::default();

        for (index, component) in self.laser.color.iter_mut().enumerate() {
            if !component.is_finite() {
                log::warn!(
                    "Non-finite laser color component {index} {component:?}, resetting to {:.2}",
                    defaults.color[index]
                );
                *component = defaults.color[index];
            } else if !(0.0..=1.0).contains(component) {
                log::warn!(
                    "Invalid laser color component {index} {component:.2}, clamping to 0.0-1.0 range"
                );
                *component = component.clamp(0.0, 1.0);
            }
        }

        if !self.laser.width.is_finite() {
            log::warn!(
                "Non-finite laser width {:?}, resetting to {:.1}",
                self.laser.width,
                defaults.width
            );
            self.laser.width = defaults.width;
        } else if !(LASER_WIDTH_MIN..=LASER_WIDTH_MAX).contains(&self.laser.width) {
            log::warn!(
                "Invalid laser width {:.1}, clamping to {LASER_WIDTH_MIN:.1}-{LASER_WIDTH_MAX:.1} range",
                self.laser.width
            );
            self.laser.width = self.laser.width.clamp(LASER_WIDTH_MIN, LASER_WIDTH_MAX);
        }

        if self.laser.hold_ms > LASER_HOLD_MS_MAX {
            log::warn!(
                "Invalid laser hold_ms {}, clamping to 0-{LASER_HOLD_MS_MAX} range",
                self.laser.hold_ms
            );
            self.laser.hold_ms = LASER_HOLD_MS_MAX;
        }

        if self.laser.fade_ms > LASER_FADE_MS_MAX {
            log::warn!(
                "Invalid laser fade_ms {}, clamping to 0-{LASER_FADE_MS_MAX} range",
                self.laser.fade_ms
            );
            self.laser.fade_ms = LASER_FADE_MS_MAX;
        }
    }
}
