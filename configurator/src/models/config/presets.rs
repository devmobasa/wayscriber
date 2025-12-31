use super::super::color::ColorInput;
use super::super::error::FormError;
use super::super::fields::{
    OverrideOption, PresetEraserKindOption, PresetEraserModeOption, ToolOption,
};
use super::super::util::format_float;
use super::parse::{parse_optional_f64, parse_required_f64};
use wayscriber::config::{
    Config, PRESET_SLOTS_MAX, PRESET_SLOTS_MIN, PresetSlotsConfig, ToolPresetConfig,
};
use wayscriber::input::Tool;

#[derive(Debug, Clone, PartialEq)]
pub struct PresetSlotDraft {
    pub enabled: bool,
    pub name: String,
    pub tool: ToolOption,
    pub color: ColorInput,
    pub size: String,
    pub eraser_kind: PresetEraserKindOption,
    pub eraser_mode: PresetEraserModeOption,
    pub marker_opacity: String,
    pub fill_enabled: OverrideOption,
    pub font_size: String,
    pub text_background_enabled: OverrideOption,
    pub arrow_length: String,
    pub arrow_angle: String,
    pub arrow_head_at_end: OverrideOption,
    pub show_status_bar: OverrideOption,
}

impl PresetSlotDraft {
    pub(super) fn from_config(preset: Option<&ToolPresetConfig>, defaults: &Config) -> Self {
        match preset {
            Some(preset) => Self {
                enabled: true,
                name: preset.name.clone().unwrap_or_default(),
                tool: ToolOption::from_tool(preset.tool),
                color: ColorInput::from_color(&preset.color),
                size: format_float(preset.size),
                eraser_kind: PresetEraserKindOption::from_option(preset.eraser_kind),
                eraser_mode: PresetEraserModeOption::from_option(preset.eraser_mode),
                marker_opacity: preset.marker_opacity.map(format_float).unwrap_or_default(),
                fill_enabled: OverrideOption::from_option(preset.fill_enabled),
                font_size: preset.font_size.map(format_float).unwrap_or_default(),
                text_background_enabled: OverrideOption::from_option(
                    preset.text_background_enabled,
                ),
                arrow_length: preset.arrow_length.map(format_float).unwrap_or_default(),
                arrow_angle: preset.arrow_angle.map(format_float).unwrap_or_default(),
                arrow_head_at_end: OverrideOption::from_option(preset.arrow_head_at_end),
                show_status_bar: OverrideOption::from_option(preset.show_status_bar),
            },
            None => {
                let mut slot = Self::default_from_config(defaults);
                slot.enabled = false;
                slot
            }
        }
    }

    fn default_from_config(defaults: &Config) -> Self {
        Self {
            enabled: true,
            name: String::new(),
            tool: ToolOption::from_tool(Tool::Pen),
            color: ColorInput::from_color(&defaults.drawing.default_color),
            size: format_float(defaults.drawing.default_thickness),
            eraser_kind: PresetEraserKindOption::Default,
            eraser_mode: PresetEraserModeOption::Default,
            marker_opacity: String::new(),
            fill_enabled: OverrideOption::Default,
            font_size: String::new(),
            text_background_enabled: OverrideOption::Default,
            arrow_length: String::new(),
            arrow_angle: String::new(),
            arrow_head_at_end: OverrideOption::Default,
            show_status_bar: OverrideOption::Default,
        }
    }

    pub(super) fn to_config(
        &self,
        slot_index: usize,
        errors: &mut Vec<FormError>,
    ) -> Option<ToolPresetConfig> {
        if !self.enabled {
            return None;
        }

        let name = self.name.trim();
        let name = if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        };

        let field_prefix = format!("presets.slot_{slot_index}.");
        let color_field = format!("{field_prefix}color");
        let color = match self.color.to_color_spec_with_field(&color_field) {
            Ok(color) => Some(color),
            Err(err) => {
                errors.push(err);
                None
            }
        };

        let size = parse_required_f64(&self.size, || format!("{field_prefix}size"), errors);

        let marker_opacity = parse_optional_f64(
            &self.marker_opacity,
            || format!("{field_prefix}marker_opacity"),
            errors,
        );
        let font_size = parse_optional_f64(
            &self.font_size,
            || format!("{field_prefix}font_size"),
            errors,
        );
        let arrow_length = parse_optional_f64(
            &self.arrow_length,
            || format!("{field_prefix}arrow_length"),
            errors,
        );
        let arrow_angle = parse_optional_f64(
            &self.arrow_angle,
            || format!("{field_prefix}arrow_angle"),
            errors,
        );

        let color = color?;
        let size = size?;

        Some(ToolPresetConfig {
            name,
            tool: self.tool.to_tool(),
            color,
            size,
            eraser_kind: self.eraser_kind.to_option(),
            eraser_mode: self.eraser_mode.to_option(),
            marker_opacity,
            fill_enabled: self.fill_enabled.to_option(),
            font_size,
            text_background_enabled: self.text_background_enabled.to_option(),
            arrow_length,
            arrow_angle,
            arrow_head_at_end: self.arrow_head_at_end.to_option(),
            show_status_bar: self.show_status_bar.to_option(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresetsDraft {
    pub slot_count: usize,
    slots: Vec<PresetSlotDraft>,
}

impl PresetsDraft {
    pub fn from_config(config: &Config) -> Self {
        let slots = (1..=PRESET_SLOTS_MAX)
            .map(|slot| PresetSlotDraft::from_config(config.presets.get_slot(slot), config))
            .collect();
        Self {
            slot_count: config.presets.slot_count,
            slots,
        }
    }

    pub fn to_config(&self, errors: &mut Vec<FormError>) -> PresetSlotsConfig {
        let mut config = PresetSlotsConfig::default();

        if !(PRESET_SLOTS_MIN..=PRESET_SLOTS_MAX).contains(&self.slot_count) {
            errors.push(FormError::new(
                "presets.slot_count",
                format!("Expected a value between {PRESET_SLOTS_MIN} and {PRESET_SLOTS_MAX}"),
            ));
        }
        config.slot_count = self.slot_count.clamp(PRESET_SLOTS_MIN, PRESET_SLOTS_MAX);

        for (index, slot) in self.slots.iter().enumerate() {
            let slot_index = index + 1;
            config.set_slot(slot_index, slot.to_config(slot_index, errors));
        }

        config
    }

    pub fn slot(&self, slot: usize) -> Option<&PresetSlotDraft> {
        if slot == 0 {
            return None;
        }
        self.slots.get(slot - 1)
    }

    pub fn slot_mut(&mut self, slot: usize) -> Option<&mut PresetSlotDraft> {
        if slot == 0 {
            return None;
        }
        self.slots.get_mut(slot - 1)
    }
}
