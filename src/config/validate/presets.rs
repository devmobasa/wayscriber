use super::Config;
use crate::draw::{REGULAR_POLYGON_MAX_SIDES, REGULAR_POLYGON_MIN_SIDES, clamp_regular_sides};
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

use super::super::types::{PRESET_SLOTS_MAX, PRESET_SLOTS_MIN, ToolPresetConfig};

#[derive(Clone, Copy)]
struct PresetFloatRange {
    label: &'static str,
    min: f64,
    max: f64,
    precision: usize,
}

const MARKER_OPACITY: PresetFloatRange = PresetFloatRange {
    label: "marker_opacity",
    min: 0.05,
    max: 0.9,
    precision: 2,
};
const FONT_SIZE: PresetFloatRange = PresetFloatRange {
    label: "font_size",
    min: 8.0,
    max: 72.0,
    precision: 1,
};
const ARROW_LENGTH: PresetFloatRange = PresetFloatRange {
    label: "arrow_length",
    min: 5.0,
    max: 50.0,
    precision: 1,
};
const ARROW_ANGLE: PresetFloatRange = PresetFloatRange {
    label: "arrow_angle",
    min: 15.0,
    max: 60.0,
    precision: 1,
};

impl Config {
    pub(super) fn validate_presets(&mut self) {
        validate_slot_count(&mut self.presets.slot_count);
        for (slot, preset) in [
            (1, self.presets.slot_1.as_mut()),
            (2, self.presets.slot_2.as_mut()),
            (3, self.presets.slot_3.as_mut()),
            (4, self.presets.slot_4.as_mut()),
            (5, self.presets.slot_5.as_mut()),
        ] {
            if let Some(preset) = preset {
                validate_preset(slot, preset);
            }
        }
    }
}

fn validate_slot_count(slot_count: &mut usize) {
    if (PRESET_SLOTS_MIN..=PRESET_SLOTS_MAX).contains(slot_count) {
        return;
    }
    log::warn!(
        "Invalid preset slot_count {}, clamping to {}-{} range",
        *slot_count,
        PRESET_SLOTS_MIN,
        PRESET_SLOTS_MAX
    );
    *slot_count = (*slot_count).clamp(PRESET_SLOTS_MIN, PRESET_SLOTS_MAX);
}

fn validate_preset(slot: usize, preset: &mut ToolPresetConfig) {
    clamp_stroke_size(slot, "size", &mut preset.size);
    if let Some(tool_settings) = preset.tool_settings.as_mut() {
        for (label, size) in [
            ("pen size", &mut tool_settings.pen.size),
            ("line size", &mut tool_settings.line.size),
            ("rect size", &mut tool_settings.rect.size),
            ("ellipse size", &mut tool_settings.ellipse.size),
            ("arrow size", &mut tool_settings.arrow.size),
            ("blur size", &mut tool_settings.blur.size),
            ("marker size", &mut tool_settings.marker.size),
            ("step marker size", &mut tool_settings.step_marker.size),
            ("eraser size", &mut tool_settings.eraser_size),
        ] {
            clamp_stroke_size(slot, label, size);
        }
    }

    clamp_optional_float(slot, &mut preset.marker_opacity, MARKER_OPACITY);
    clamp_optional_float(slot, &mut preset.font_size, FONT_SIZE);
    clamp_optional_float(slot, &mut preset.arrow_length, ARROW_LENGTH);
    clamp_optional_float(slot, &mut preset.arrow_angle, ARROW_ANGLE);
    clamp_polygon_sides(slot, &mut preset.polygon_sides);
}

fn clamp_stroke_size(slot: usize, label: &str, value: &mut f64) {
    if (MIN_STROKE_THICKNESS..=MAX_STROKE_THICKNESS).contains(value) {
        return;
    }
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

fn clamp_optional_float(slot: usize, value: &mut Option<f64>, range: PresetFloatRange) {
    let Some(value) = value.as_mut() else {
        return;
    };
    if (range.min..=range.max).contains(value) {
        return;
    }
    log::warn!(
        "Invalid {} {:.*} in preset slot {}, clamping to {:.*}-{:.*} range",
        range.label,
        range.precision,
        *value,
        slot,
        range.precision,
        range.min,
        range.precision,
        range.max
    );
    *value = value.clamp(range.min, range.max);
}

fn clamp_polygon_sides(slot: usize, sides: &mut Option<u8>) {
    let Some(sides) = sides.as_mut() else {
        return;
    };
    if (REGULAR_POLYGON_MIN_SIDES..=REGULAR_POLYGON_MAX_SIDES).contains(sides) {
        return;
    }
    log::warn!(
        "Invalid polygon_sides {} in preset slot {}, clamping to {}-{} range",
        *sides,
        slot,
        REGULAR_POLYGON_MIN_SIDES,
        REGULAR_POLYGON_MAX_SIDES
    );
    *sides = clamp_regular_sides(*sides);
}
