use crate::config::enums::ColorSpec;
use crate::draw::EraserKind;
use crate::input::{EraserMode, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PRESET_SLOTS_MIN: usize = 3;
pub const PRESET_SLOTS_MAX: usize = 5;

/// Tool preset configuration for quick slot switching.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolPresetConfig {
    /// Optional label for UI display.
    #[serde(default)]
    pub name: Option<String>,

    /// Tool to activate when applying the preset.
    pub tool: Tool,

    /// Drawing color to apply.
    pub color: ColorSpec,

    /// Tool size (thickness or eraser size depending on tool).
    pub size: f64,

    /// Optional eraser brush shape override.
    #[serde(default)]
    pub eraser_kind: Option<EraserKind>,

    /// Optional eraser mode override.
    #[serde(default)]
    pub eraser_mode: Option<EraserMode>,

    /// Optional marker opacity override.
    #[serde(default)]
    pub marker_opacity: Option<f64>,

    /// Optional fill state override.
    #[serde(default)]
    pub fill_enabled: Option<bool>,

    /// Optional font size override.
    #[serde(default)]
    pub font_size: Option<f64>,

    /// Optional text background override.
    #[serde(default)]
    pub text_background_enabled: Option<bool>,

    /// Optional arrow length override.
    #[serde(default)]
    pub arrow_length: Option<f64>,

    /// Optional arrow angle override.
    #[serde(default)]
    pub arrow_angle: Option<f64>,

    /// Optional arrow head placement override.
    #[serde(default)]
    pub arrow_head_at_end: Option<bool>,

    /// Optional status bar visibility override.
    #[serde(default)]
    pub show_status_bar: Option<bool>,
}

/// Preset slot configuration for quick tool switching.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PresetSlotsConfig {
    /// Number of visible preset slots (3-5).
    #[serde(default = "default_preset_slot_count")]
    pub slot_count: usize,

    /// Preset slot 1.
    #[serde(default)]
    pub slot_1: Option<ToolPresetConfig>,

    /// Preset slot 2.
    #[serde(default)]
    pub slot_2: Option<ToolPresetConfig>,

    /// Preset slot 3.
    #[serde(default)]
    pub slot_3: Option<ToolPresetConfig>,

    /// Preset slot 4.
    #[serde(default)]
    pub slot_4: Option<ToolPresetConfig>,

    /// Preset slot 5.
    #[serde(default)]
    pub slot_5: Option<ToolPresetConfig>,
}

impl PresetSlotsConfig {
    pub fn get_slot(&self, slot: usize) -> Option<&ToolPresetConfig> {
        match slot {
            1 => self.slot_1.as_ref(),
            2 => self.slot_2.as_ref(),
            3 => self.slot_3.as_ref(),
            4 => self.slot_4.as_ref(),
            5 => self.slot_5.as_ref(),
            _ => None,
        }
    }

    pub fn set_slot(&mut self, slot: usize, preset: Option<ToolPresetConfig>) {
        match slot {
            1 => self.slot_1 = preset,
            2 => self.slot_2 = preset,
            3 => self.slot_3 = preset,
            4 => self.slot_4 = preset,
            5 => self.slot_5 = preset,
            _ => {}
        }
    }
}

impl Default for PresetSlotsConfig {
    fn default() -> Self {
        Self {
            slot_count: default_preset_slot_count(),
            slot_1: None,
            slot_2: None,
            slot_3: None,
            slot_4: None,
            slot_5: None,
        }
    }
}

fn default_preset_slot_count() -> usize {
    PRESET_SLOTS_MAX
}
