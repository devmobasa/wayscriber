use super::super::draft::ConfigDraft;
use super::super::parse::{parse_field_in_range, parse_u64_in_range};
use crate::models::error::FormError;
use wayscriber::config::{
    Config, LASER_FADE_MS_MAX, LASER_HOLD_MS_MAX, LASER_WIDTH_MAX, LASER_WIDTH_MIN,
};

impl ConfigDraft {
    pub(super) fn apply_laser(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        match self.laser_color.to_array("laser.color") {
            Ok(values) => config.laser.color = values,
            Err(err) => errors.push(err),
        }
        parse_field_in_range(
            &self.laser_width,
            "laser.width",
            LASER_WIDTH_MIN,
            LASER_WIDTH_MAX,
            errors,
            |value| config.laser.width = value,
        );
        parse_u64_in_range(
            &self.laser_hold_ms,
            "laser.hold_ms",
            0,
            LASER_HOLD_MS_MAX,
            errors,
            |value| config.laser.hold_ms = value,
        );
        parse_u64_in_range(
            &self.laser_fade_ms,
            "laser.fade_ms",
            0,
            LASER_FADE_MS_MAX,
            errors,
            |value| config.laser.fade_ms = value,
        );
    }
}
