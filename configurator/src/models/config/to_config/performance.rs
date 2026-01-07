use super::super::draft::ConfigDraft;
use super::super::parse::parse_u32_field;
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_performance(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        config.performance.buffer_count = self.performance_buffer_count;
        config.performance.enable_vsync = self.performance_enable_vsync;
        parse_u32_field(
            &self.performance_max_fps_no_vsync,
            "performance.max_fps_no_vsync",
            errors,
            |value| config.performance.max_fps_no_vsync = value,
        );
        parse_u32_field(
            &self.performance_ui_animation_fps,
            "performance.ui_animation_fps",
            errors,
            |value| config.performance.ui_animation_fps = value,
        );
    }
}
