use super::Config;

impl Config {
    pub(super) fn validate_ui(&mut self) {
        // Validate click highlight settings
        if !(16.0..=160.0).contains(&self.ui.click_highlight.radius) {
            log::warn!(
                "Invalid click highlight radius {:.1}, clamping to 16.0-160.0 range",
                self.ui.click_highlight.radius
            );
            self.ui.click_highlight.radius = self.ui.click_highlight.radius.clamp(16.0, 160.0);
        }

        if !(1.0..=12.0).contains(&self.ui.click_highlight.outline_thickness) {
            log::warn!(
                "Invalid click highlight outline thickness {:.1}, clamping to 1.0-12.0 range",
                self.ui.click_highlight.outline_thickness
            );
            self.ui.click_highlight.outline_thickness =
                self.ui.click_highlight.outline_thickness.clamp(1.0, 12.0);
        }

        if !(150..=1500).contains(&self.ui.click_highlight.duration_ms) {
            log::warn!(
                "Invalid click highlight duration {}ms, clamping to 150-1500ms range",
                self.ui.click_highlight.duration_ms
            );
            self.ui.click_highlight.duration_ms =
                self.ui.click_highlight.duration_ms.clamp(150, 1500);
        }

        // Sanitize NaN/Inf before clamping (clamp doesn't fix non-finite values)
        if !self.ui.toolbar.scale.is_finite() {
            log::warn!(
                "Non-finite toolbar scale {:?}, resetting to 1.0",
                self.ui.toolbar.scale
            );
            self.ui.toolbar.scale = 1.0;
        } else if !(0.5..=3.0).contains(&self.ui.toolbar.scale) {
            log::warn!(
                "Invalid toolbar scale {:.2}, clamping to 0.5-3.0 range",
                self.ui.toolbar.scale
            );
            self.ui.toolbar.scale = self.ui.toolbar.scale.clamp(0.5, 3.0);
        }

        for i in 0..4 {
            if !(0.0..=1.0).contains(&self.ui.click_highlight.fill_color[i]) {
                log::warn!(
                    "Invalid click highlight fill_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.ui.click_highlight.fill_color[i]
                );
                self.ui.click_highlight.fill_color[i] =
                    self.ui.click_highlight.fill_color[i].clamp(0.0, 1.0);
            }
            if !(0.0..=1.0).contains(&self.ui.click_highlight.outline_color[i]) {
                log::warn!(
                    "Invalid click highlight outline_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.ui.click_highlight.outline_color[i]
                );
                self.ui.click_highlight.outline_color[i] =
                    self.ui.click_highlight.outline_color[i].clamp(0.0, 1.0);
            }
        }
    }
}
