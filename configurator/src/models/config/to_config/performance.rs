use super::super::draft::ConfigDraft;
use super::super::performance_fields::{parse_performance_u32, validate_performance_u32};
use crate::models::error::FormError;
use wayscriber::config::{Config, PerformanceFieldId};

impl ConfigDraft {
    pub(super) fn apply_performance(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        if let Some(value) = validate_performance_u32(
            PerformanceFieldId::BufferCount,
            self.performance_buffer_count,
            errors,
        ) {
            config.performance.buffer_count = value;
        }
        config.performance.enable_vsync = self.performance_enable_vsync;
        if let Some(value) = parse_performance_u32(
            PerformanceFieldId::MaxFpsNoVsync,
            &self.performance_max_fps_no_vsync,
            errors,
        ) {
            config.performance.max_fps_no_vsync = value;
        }
        if let Some(value) = parse_performance_u32(
            PerformanceFieldId::UiAnimationFps,
            &self.performance_ui_animation_fps,
            errors,
        ) {
            config.performance.ui_animation_fps = value;
        }
    }
}
