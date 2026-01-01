use super::Config;

impl Config {
    pub(super) fn validate_performance(&mut self) {
        // Buffer count: 2 - 4
        if !(2..=4).contains(&self.performance.buffer_count) {
            log::warn!(
                "Invalid buffer_count {}, clamping to 2-4 range",
                self.performance.buffer_count
            );
            self.performance.buffer_count = self.performance.buffer_count.clamp(2, 4);
        }

        // UI animation FPS: allow 0 (unlimited), otherwise clamp to 1 - 240.
        const MAX_UI_ANIMATION_FPS: u32 = 240;
        if self.performance.ui_animation_fps > MAX_UI_ANIMATION_FPS {
            log::warn!(
                "Invalid ui_animation_fps {}, clamping to 0-{} range",
                self.performance.ui_animation_fps,
                MAX_UI_ANIMATION_FPS
            );
            self.performance.ui_animation_fps =
                self.performance.ui_animation_fps.min(MAX_UI_ANIMATION_FPS);
        }
    }
}
