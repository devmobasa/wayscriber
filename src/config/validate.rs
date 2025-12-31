use super::Config;
use super::keybindings::KeybindingsConfig;
use super::types::{PRESET_SLOTS_MAX, PRESET_SLOTS_MIN, SessionStorageMode, ToolPresetConfig};
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

impl Config {
    /// Validates and clamps all configuration values to acceptable ranges.
    ///
    /// This method ensures that user-provided config values won't cause undefined behavior
    /// or rendering issues. Invalid values are clamped to the nearest valid value and a
    /// warning is logged.
    ///
    /// Validated ranges:
    /// - `default_thickness`: 1.0 - 50.0
    /// - `default_font_size`: 8.0 - 72.0
    /// - `arrow.length`: 5.0 - 50.0
    /// - `arrow.angle_degrees`: 15.0 - 60.0
    /// - `buffer_count`: 2 - 4
    pub fn validate_and_clamp(&mut self) {
        // Thickness: 1.0 - 50.0
        if !(MIN_STROKE_THICKNESS..=MAX_STROKE_THICKNESS).contains(&self.drawing.default_thickness)
        {
            log::warn!(
                "Invalid default_thickness {:.1}, clamping to {:.1}-{:.1} range",
                self.drawing.default_thickness,
                MIN_STROKE_THICKNESS,
                MAX_STROKE_THICKNESS
            );
            self.drawing.default_thickness = self
                .drawing
                .default_thickness
                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        }

        // Eraser size: 1.0 - 50.0
        if !(MIN_STROKE_THICKNESS..=MAX_STROKE_THICKNESS)
            .contains(&self.drawing.default_eraser_size)
        {
            log::warn!(
                "Invalid default_eraser_size {:.1}, clamping to {:.1}-{:.1} range",
                self.drawing.default_eraser_size,
                MIN_STROKE_THICKNESS,
                MAX_STROKE_THICKNESS
            );
            self.drawing.default_eraser_size = self
                .drawing
                .default_eraser_size
                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        }

        // Marker opacity: 0.05 - 0.9
        if !(0.05..=0.9).contains(&self.drawing.marker_opacity) {
            log::warn!(
                "Invalid marker_opacity {:.2}, clamping to 0.05-0.90 range",
                self.drawing.marker_opacity
            );
            self.drawing.marker_opacity = self.drawing.marker_opacity.clamp(0.05, 0.9);
        }

        // Font size: 8.0 - 72.0
        if !(8.0..=72.0).contains(&self.drawing.default_font_size) {
            log::warn!(
                "Invalid default_font_size {:.1}, clamping to 8.0-72.0 range",
                self.drawing.default_font_size
            );
            self.drawing.default_font_size = self.drawing.default_font_size.clamp(8.0, 72.0);
        }

        if !(1.0..=20.0).contains(&self.drawing.hit_test_tolerance) {
            log::warn!(
                "Invalid hit_test_tolerance {:.1}, clamping to 1.0-20.0 range",
                self.drawing.hit_test_tolerance
            );
            self.drawing.hit_test_tolerance = self.drawing.hit_test_tolerance.clamp(1.0, 20.0);
        }

        if self.drawing.hit_test_linear_threshold == 0 {
            log::warn!("hit_test_linear_threshold must be at least 1; using default 400");
            self.drawing.hit_test_linear_threshold = 400;
        }

        if !(10..=1000).contains(&self.drawing.undo_stack_limit) {
            log::warn!(
                "Invalid undo_stack_limit {}, clamping to 10-1000 range",
                self.drawing.undo_stack_limit
            );
            self.drawing.undo_stack_limit = self.drawing.undo_stack_limit.clamp(10, 1000);
        }

        if !(PRESET_SLOTS_MIN..=PRESET_SLOTS_MAX).contains(&self.presets.slot_count) {
            log::warn!(
                "Invalid preset slot_count {}, clamping to {}-{} range",
                self.presets.slot_count,
                PRESET_SLOTS_MIN,
                PRESET_SLOTS_MAX
            );
            self.presets.slot_count = self
                .presets
                .slot_count
                .clamp(PRESET_SLOTS_MIN, PRESET_SLOTS_MAX);
        }

        let clamp_preset = |slot: usize, preset: &mut ToolPresetConfig| {
            if !(MIN_STROKE_THICKNESS..=MAX_STROKE_THICKNESS).contains(&preset.size) {
                log::warn!(
                    "Invalid preset size {:.1} in slot {}, clamping to {:.1}-{:.1} range",
                    preset.size,
                    slot,
                    MIN_STROKE_THICKNESS,
                    MAX_STROKE_THICKNESS
                );
                preset.size = preset
                    .size
                    .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
            }

            if let Some(opacity) = preset.marker_opacity.as_mut()
                && !(0.05..=0.9).contains(opacity)
            {
                log::warn!(
                    "Invalid marker_opacity {:.2} in preset slot {}, clamping to 0.05-0.90 range",
                    *opacity,
                    slot
                );
                *opacity = opacity.clamp(0.05, 0.9);
            }

            if let Some(size) = preset.font_size.as_mut()
                && !(8.0..=72.0).contains(size)
            {
                log::warn!(
                    "Invalid font_size {:.1} in preset slot {}, clamping to 8.0-72.0 range",
                    *size,
                    slot
                );
                *size = size.clamp(8.0, 72.0);
            }

            if let Some(length) = preset.arrow_length.as_mut()
                && !(5.0..=50.0).contains(length)
            {
                log::warn!(
                    "Invalid arrow_length {:.1} in preset slot {}, clamping to 5.0-50.0 range",
                    *length,
                    slot
                );
                *length = length.clamp(5.0, 50.0);
            }

            if let Some(angle) = preset.arrow_angle.as_mut()
                && !(15.0..=60.0).contains(angle)
            {
                log::warn!(
                    "Invalid arrow_angle {:.1} in preset slot {}, clamping to 15.0-60.0 range",
                    *angle,
                    slot
                );
                *angle = angle.clamp(15.0, 60.0);
            }
        };

        if let Some(preset) = self.presets.slot_1.as_mut() {
            clamp_preset(1, preset);
        }
        if let Some(preset) = self.presets.slot_2.as_mut() {
            clamp_preset(2, preset);
        }
        if let Some(preset) = self.presets.slot_3.as_mut() {
            clamp_preset(3, preset);
        }
        if let Some(preset) = self.presets.slot_4.as_mut() {
            clamp_preset(4, preset);
        }
        if let Some(preset) = self.presets.slot_5.as_mut() {
            clamp_preset(5, preset);
        }

        #[cfg(tablet)]
        {
            if self.tablet.min_thickness > self.tablet.max_thickness {
                std::mem::swap(
                    &mut self.tablet.min_thickness,
                    &mut self.tablet.max_thickness,
                );
            }
            self.tablet.min_thickness = self
                .tablet
                .min_thickness
                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
            self.tablet.max_thickness = self
                .tablet
                .max_thickness
                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        }

        // History delays: clamp to a reasonable range to avoid long freezes or instant drains.
        const MAX_DELAY_MS: u64 = 5_000;
        const MIN_DELAY_MS: u64 = 50;
        let clamp_delay = |label: &str, value: &mut u64| {
            if *value < MIN_DELAY_MS {
                log::warn!(
                    "{} {}ms too small; clamping to {}ms",
                    label,
                    *value,
                    MIN_DELAY_MS
                );
                *value = MIN_DELAY_MS;
            }
            if *value > MAX_DELAY_MS {
                log::warn!(
                    "{} {}ms too large; clamping to {}ms",
                    label,
                    *value,
                    MAX_DELAY_MS
                );
                *value = MAX_DELAY_MS;
            }
        };
        clamp_delay("undo_all_delay_ms", &mut self.history.undo_all_delay_ms);
        clamp_delay("redo_all_delay_ms", &mut self.history.redo_all_delay_ms);
        clamp_delay(
            "custom_undo_delay_ms",
            &mut self.history.custom_undo_delay_ms,
        );
        clamp_delay(
            "custom_redo_delay_ms",
            &mut self.history.custom_redo_delay_ms,
        );

        // Custom history step counts: clamp to sane bounds.
        const MIN_STEPS: usize = 1;
        const MAX_STEPS: usize = 500;
        let clamp_steps = |label: &str, value: &mut usize| {
            if *value < MIN_STEPS {
                log::warn!("{} {} too small; clamping to {}", label, *value, MIN_STEPS);
                *value = MIN_STEPS;
            }
            if *value > MAX_STEPS {
                log::warn!("{} {} too large; clamping to {}", label, *value, MAX_STEPS);
                *value = MAX_STEPS;
            }
        };
        clamp_steps("custom_undo_steps", &mut self.history.custom_undo_steps);
        clamp_steps("custom_redo_steps", &mut self.history.custom_redo_steps);

        // Arrow length: 5.0 - 50.0
        if !(5.0..=50.0).contains(&self.arrow.length) {
            log::warn!(
                "Invalid arrow length {:.1}, clamping to 5.0-50.0 range",
                self.arrow.length
            );
            self.arrow.length = self.arrow.length.clamp(5.0, 50.0);
        }

        // Arrow angle: 15.0 - 60.0 degrees
        if !(15.0..=60.0).contains(&self.arrow.angle_degrees) {
            log::warn!(
                "Invalid arrow angle {:.1} deg, clamping to 15.0-60.0 deg range",
                self.arrow.angle_degrees
            );
            self.arrow.angle_degrees = self.arrow.angle_degrees.clamp(15.0, 60.0);
        }

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

        // Validate font weight is reasonable
        let valid_weight = matches!(
            self.drawing.font_weight.to_lowercase().as_str(),
            "normal" | "bold" | "light" | "ultralight" | "heavy" | "ultrabold"
        ) || self
            .drawing
            .font_weight
            .parse::<u32>()
            .is_ok_and(|w| (100..=900).contains(&w));

        if !valid_weight {
            log::warn!(
                "Invalid font_weight '{}', falling back to 'bold'",
                self.drawing.font_weight
            );
            self.drawing.font_weight = "bold".to_string();
        }

        // Validate font style
        if !matches!(
            self.drawing.font_style.to_lowercase().as_str(),
            "normal" | "italic" | "oblique"
        ) {
            log::warn!(
                "Invalid font_style '{}', falling back to 'normal'",
                self.drawing.font_style
            );
            self.drawing.font_style = "normal".to_string();
        }

        // Validate board mode default
        if !matches!(
            self.board.default_mode.to_lowercase().as_str(),
            "transparent" | "whiteboard" | "blackboard"
        ) {
            log::warn!(
                "Invalid board default_mode '{}', falling back to 'transparent'",
                self.board.default_mode
            );
            self.board.default_mode = "transparent".to_string();
        }

        // Validate board color RGB values (0.0-1.0)
        for i in 0..3 {
            if !(0.0..=1.0).contains(&self.board.whiteboard_color[i]) {
                log::warn!(
                    "Invalid whiteboard_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.whiteboard_color[i]
                );
                self.board.whiteboard_color[i] = self.board.whiteboard_color[i].clamp(0.0, 1.0);
            }
            if !(0.0..=1.0).contains(&self.board.blackboard_color[i]) {
                log::warn!(
                    "Invalid blackboard_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.blackboard_color[i]
                );
                self.board.blackboard_color[i] = self.board.blackboard_color[i].clamp(0.0, 1.0);
            }
            if !(0.0..=1.0).contains(&self.board.whiteboard_pen_color[i]) {
                log::warn!(
                    "Invalid whiteboard_pen_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.whiteboard_pen_color[i]
                );
                self.board.whiteboard_pen_color[i] =
                    self.board.whiteboard_pen_color[i].clamp(0.0, 1.0);
            }
            if !(0.0..=1.0).contains(&self.board.blackboard_pen_color[i]) {
                log::warn!(
                    "Invalid blackboard_pen_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.blackboard_pen_color[i]
                );
                self.board.blackboard_pen_color[i] =
                    self.board.blackboard_pen_color[i].clamp(0.0, 1.0);
            }
        }

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

        // Validate keybindings (try to build action map to catch parse errors)
        if let Err(e) = self.keybindings.build_action_map() {
            log::warn!("Invalid keybinding configuration: {}. Using defaults.", e);
            self.keybindings = KeybindingsConfig::default();
        }

        if self.session.max_shapes_per_frame == 0 {
            log::warn!("session.max_shapes_per_frame must be positive; using 1 instead");
            self.session.max_shapes_per_frame = 1;
        }

        if self.session.max_file_size_mb == 0 {
            log::warn!("session.max_file_size_mb must be positive; using 1 MB instead");
            self.session.max_file_size_mb = 1;
        } else if self.session.max_file_size_mb > 1024 {
            log::warn!(
                "session.max_file_size_mb {} too large, clamping to 1024",
                self.session.max_file_size_mb
            );
            self.session.max_file_size_mb = 1024;
        }

        if self.session.auto_compress_threshold_kb == 0 {
            log::warn!("session.auto_compress_threshold_kb must be positive; using 1 KiB");
            self.session.auto_compress_threshold_kb = 1;
        }

        if matches!(self.session.storage, SessionStorageMode::Custom) {
            let custom = self
                .session
                .custom_directory
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            if custom.is_none() {
                log::warn!(
                    "session.storage set to 'custom' but session.custom_directory missing or empty; falling back to 'auto'"
                );
                self.session.storage = SessionStorageMode::Auto;
                self.session.custom_directory = None;
            }
        }
    }
}
