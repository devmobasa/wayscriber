use super::Config;
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

use super::super::types::{PRESET_SLOTS_MAX, PRESET_SLOTS_MIN, ToolPresetConfig};

impl Config {
    pub(super) fn validate_presets(&mut self) {
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
            let clamp_size = |value: &mut f64, label: &str| {
                if !(MIN_STROKE_THICKNESS..=MAX_STROKE_THICKNESS).contains(&*value) {
                    log::warn!(
                        "Invalid preset {} {:.1} in slot {}, clamping to {:.1}-{:.1} range",
                        label,
                        *value,
                        slot,
                        MIN_STROKE_THICKNESS,
                        MAX_STROKE_THICKNESS
                    );
                    *value = value.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
                }
            };

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

            if let Some(tool_settings) = preset.tool_settings.as_mut() {
                clamp_size(&mut tool_settings.pen.size, "pen size");
                clamp_size(&mut tool_settings.line.size, "line size");
                clamp_size(&mut tool_settings.rect.size, "rect size");
                clamp_size(&mut tool_settings.ellipse.size, "ellipse size");
                clamp_size(&mut tool_settings.arrow.size, "arrow size");
                clamp_size(&mut tool_settings.blur.size, "blur size");
                clamp_size(&mut tool_settings.marker.size, "marker size");
                clamp_size(&mut tool_settings.step_marker.size, "step marker size");
                clamp_size(&mut tool_settings.eraser_size, "eraser size");
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
    }
}
