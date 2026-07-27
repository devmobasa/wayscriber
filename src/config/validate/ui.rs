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

        if !(300..=5000).contains(&self.ui.command_palette_toast_duration_ms) {
            log::warn!(
                "Invalid command palette toast duration {}ms, clamping to 300-5000ms range",
                self.ui.command_palette_toast_duration_ms
            );
            self.ui.command_palette_toast_duration_ms =
                self.ui.command_palette_toast_duration_ms.clamp(300, 5000);
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

        let resolved_items = self.ui.toolbar.items.resolved();
        for unknown in resolved_items.unknown_hidden {
            log::warn!(
                "Unknown toolbar item id {:?} in ui.toolbar.items.hidden; preserving it for forward compatibility",
                unknown
            );
        }
        for unknown in resolved_items.unknown_shown {
            log::warn!(
                "Unknown toolbar item id {:?} in ui.toolbar.items.shown; preserving it for forward compatibility",
                unknown
            );
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

        self.validate_input_hud();
    }

    fn validate_input_hud(&mut self) {
        if !(200..=30_000).contains(&self.ui.input_hud.display_ms) {
            log::warn!(
                "Invalid input HUD display duration {}ms, clamping to 200-30000ms range",
                self.ui.input_hud.display_ms
            );
            self.ui.input_hud.display_ms = self.ui.input_hud.display_ms.clamp(200, 30_000);
        }

        if self.ui.input_hud.fade_ms > 5_000 {
            log::warn!(
                "Invalid input HUD fade duration {}ms, clamping to 0-5000ms range",
                self.ui.input_hud.fade_ms
            );
            self.ui.input_hud.fade_ms = self.ui.input_hud.fade_ms.min(5_000);
        }

        if !(1..=16).contains(&self.ui.input_hud.max_entries) {
            log::warn!(
                "Invalid input HUD max entries {}, clamping to 1-16 range",
                self.ui.input_hud.max_entries
            );
            self.ui.input_hud.max_entries = self.ui.input_hud.max_entries.clamp(1, 16);
        }

        // Sanitize NaN/Inf before clamping (clamp doesn't fix non-finite values)
        if !self.ui.input_hud.font_size.is_finite() {
            log::warn!(
                "Non-finite input HUD font size {:?}, resetting to 18.0",
                self.ui.input_hud.font_size
            );
            self.ui.input_hud.font_size = 18.0;
        } else if !(6.0..=72.0).contains(&self.ui.input_hud.font_size) {
            log::warn!(
                "Invalid input HUD font size {:.1}, clamping to 6.0-72.0 range",
                self.ui.input_hud.font_size
            );
            self.ui.input_hud.font_size = self.ui.input_hud.font_size.clamp(6.0, 72.0);
        }
    }
}
