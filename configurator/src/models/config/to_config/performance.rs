use super::super::draft::ConfigDraft;
use super::super::performance_fields::{parse_performance_range, validate_performance_choice};
use crate::models::error::FormError;
use wayscriber::config::{Config, PERFORMANCE_FIELDS};

impl ConfigDraft {
    pub(super) fn apply_performance(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        match validate_performance_choice(
            PERFORMANCE_FIELDS.buffer_count(),
            self.performance_buffer_count,
        ) {
            Ok(value) => config.performance.buffer_count = value,
            Err(error) => errors.push(error),
        }
        config.performance.enable_vsync = self.performance_enable_vsync;
        match parse_performance_range(
            PERFORMANCE_FIELDS.max_fps_no_vsync(),
            &self.performance_max_fps_no_vsync,
        ) {
            Ok(value) => config.performance.max_fps_no_vsync = value,
            Err(error) => errors.push(error),
        }
        match parse_performance_range(
            PERFORMANCE_FIELDS.ui_animation_fps(),
            &self.performance_ui_animation_fps,
        ) {
            Ok(value) => config.performance.ui_animation_fps = value,
            Err(error) => errors.push(error),
        }
    }
}
