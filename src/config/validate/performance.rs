use super::Config;
use crate::config::{
    PERFORMANCE_BUFFER_COUNT_MAX, PERFORMANCE_BUFFER_COUNT_MIN, PERFORMANCE_UI_ANIMATION_FPS_MAX,
};

impl Config {
    pub(super) fn validate_performance(&mut self) {
        if !(PERFORMANCE_BUFFER_COUNT_MIN..=PERFORMANCE_BUFFER_COUNT_MAX)
            .contains(&self.performance.buffer_count)
        {
            log::warn!(
                "Invalid buffer_count {}, clamping to {}-{} range",
                self.performance.buffer_count,
                PERFORMANCE_BUFFER_COUNT_MIN,
                PERFORMANCE_BUFFER_COUNT_MAX,
            );
            self.performance.buffer_count = self
                .performance
                .buffer_count
                .clamp(PERFORMANCE_BUFFER_COUNT_MIN, PERFORMANCE_BUFFER_COUNT_MAX);
        }

        // UI animation FPS: allow 0 (unlimited), otherwise clamp to 1 - 240.
        if self.performance.ui_animation_fps > PERFORMANCE_UI_ANIMATION_FPS_MAX {
            log::warn!(
                "Invalid ui_animation_fps {}, clamping to 0-{} range",
                self.performance.ui_animation_fps,
                PERFORMANCE_UI_ANIMATION_FPS_MAX
            );
            self.performance.ui_animation_fps = self
                .performance
                .ui_animation_fps
                .min(PERFORMANCE_UI_ANIMATION_FPS_MAX);
        }
    }
}
