use super::super::draft::ConfigDraft;
#[cfg(feature = "tablet-input")]
use super::super::parse::parse_field;
use crate::models::error::FormError;
use wayscriber::config::Config;

#[cfg(feature = "tablet-input")]
impl ConfigDraft {
    pub(super) fn apply_tablet(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        config.tablet.enabled = self.tablet_enabled;
        config.tablet.pressure_enabled = self.tablet_pressure_enabled;
        config.tablet.auto_eraser_switch = self.tablet_auto_eraser_switch;
        config.tablet.pressure_thickness_edit_mode =
            self.tablet_pressure_thickness_edit_mode.to_mode();
        config.tablet.pressure_thickness_entry_mode =
            self.tablet_pressure_thickness_entry_mode.to_mode();
        parse_field(
            &self.tablet_min_thickness,
            "tablet.min_thickness",
            errors,
            |value| config.tablet.min_thickness = value,
        );
        parse_field(
            &self.tablet_max_thickness,
            "tablet.max_thickness",
            errors,
            |value| config.tablet.max_thickness = value,
        );
        parse_field(
            &self.tablet_pressure_variation_threshold,
            "tablet.pressure_variation_threshold",
            errors,
            |value| config.tablet.pressure_variation_threshold = value,
        );
        parse_field(
            &self.tablet_pressure_thickness_scale_step,
            "tablet.pressure_thickness_scale_step",
            errors,
            |value| config.tablet.pressure_thickness_scale_step = value,
        );
    }
}

#[cfg(not(feature = "tablet-input"))]
impl ConfigDraft {
    pub(super) fn apply_tablet(&self, _config: &mut Config, _errors: &mut Vec<FormError>) {}
}
