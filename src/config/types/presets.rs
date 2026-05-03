use crate::config::{MouseDragToolsConfig, enums::ColorSpec};
use crate::draw::{Color, EraserKind};
use crate::input::{
    EraserMode, Tool,
    tool::{PerToolDrawingSettings, ToolDrawingSettings},
};
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

    /// Optional full per-tool color/size profile captured with this preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_settings: Option<PresetToolStatesConfig>,

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

    /// Optional per-button drag tool bindings to apply with this preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_tools: Option<MouseDragToolsConfig>,
}

impl ToolPresetConfig {
    pub fn preview_color_spec(&self) -> ColorSpec {
        self.tool_settings
            .as_ref()
            .map(|settings| settings.color_spec_for_tool(self.tool))
            .unwrap_or_else(|| self.color.clone())
    }

    pub fn preview_color(&self) -> Color {
        self.preview_color_spec().to_color()
    }

    pub fn preview_size(&self) -> f64 {
        self.tool_settings
            .as_ref()
            .map(|settings| settings.size_for_tool(self.tool))
            .unwrap_or(self.size)
    }
}

/// Color and size for one tool within a full preset profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PresetToolSettingConfig {
    pub color: ColorSpec,
    pub size: f64,
}

impl PresetToolSettingConfig {
    pub fn from_runtime(settings: ToolDrawingSettings) -> Self {
        Self {
            color: settings.color.into(),
            size: settings.thickness,
        }
    }

    pub fn to_runtime(&self) -> ToolDrawingSettings {
        ToolDrawingSettings::new(self.color.to_color(), self.size)
    }
}

/// Full drawing tool profile captured by a preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PresetToolStatesConfig {
    pub pen: PresetToolSettingConfig,
    pub line: PresetToolSettingConfig,
    pub rect: PresetToolSettingConfig,
    pub ellipse: PresetToolSettingConfig,
    pub arrow: PresetToolSettingConfig,
    pub blur: PresetToolSettingConfig,
    pub marker: PresetToolSettingConfig,
    pub step_marker: PresetToolSettingConfig,
    pub eraser_size: f64,
}

impl PresetToolStatesConfig {
    pub fn from_runtime(settings: &PerToolDrawingSettings, eraser_size: f64) -> Self {
        Self {
            pen: PresetToolSettingConfig::from_runtime(settings.pen),
            line: PresetToolSettingConfig::from_runtime(settings.line),
            rect: PresetToolSettingConfig::from_runtime(settings.rect),
            ellipse: PresetToolSettingConfig::from_runtime(settings.ellipse),
            arrow: PresetToolSettingConfig::from_runtime(settings.arrow),
            blur: PresetToolSettingConfig::from_runtime(settings.blur),
            marker: PresetToolSettingConfig::from_runtime(settings.marker),
            step_marker: PresetToolSettingConfig::from_runtime(settings.step_marker),
            eraser_size,
        }
    }

    pub fn to_runtime(&self) -> PerToolDrawingSettings {
        PerToolDrawingSettings {
            pen: self.pen.to_runtime(),
            line: self.line.to_runtime(),
            rect: self.rect.to_runtime(),
            ellipse: self.ellipse.to_runtime(),
            arrow: self.arrow.to_runtime(),
            blur: self.blur.to_runtime(),
            marker: self.marker.to_runtime(),
            step_marker: self.step_marker.to_runtime(),
        }
    }

    pub fn color_spec_for_tool(&self, tool: Tool) -> ColorSpec {
        match tool {
            Tool::Pen | Tool::Select | Tool::Highlight | Tool::Eraser => self.pen.color.clone(),
            Tool::Line => self.line.color.clone(),
            Tool::Rect => self.rect.color.clone(),
            Tool::Ellipse => self.ellipse.color.clone(),
            Tool::Arrow => self.arrow.color.clone(),
            Tool::Blur => self.blur.color.clone(),
            Tool::Marker => self.marker.color.clone(),
            Tool::StepMarker => self.step_marker.color.clone(),
        }
    }

    pub fn size_for_tool(&self, tool: Tool) -> f64 {
        match tool {
            Tool::Eraser => self.eraser_size,
            Tool::Pen | Tool::Select | Tool::Highlight => self.pen.size,
            Tool::Line => self.line.size,
            Tool::Rect => self.rect.size,
            Tool::Ellipse => self.ellipse.size,
            Tool::Arrow => self.arrow.size,
            Tool::Blur => self.blur.size,
            Tool::Marker => self.marker.size,
            Tool::StepMarker => self.step_marker.size,
        }
    }

    #[allow(dead_code)]
    pub fn set_preview_tool(&mut self, tool: Tool, color: ColorSpec, size: f64) {
        match tool {
            Tool::Eraser => {
                self.pen.color = color;
                self.eraser_size = size;
            }
            Tool::Pen | Tool::Select | Tool::Highlight => {
                self.pen.color = color;
                self.pen.size = size;
            }
            Tool::Line => {
                self.line.color = color;
                self.line.size = size;
            }
            Tool::Rect => {
                self.rect.color = color;
                self.rect.size = size;
            }
            Tool::Ellipse => {
                self.ellipse.color = color;
                self.ellipse.size = size;
            }
            Tool::Arrow => {
                self.arrow.color = color;
                self.arrow.size = size;
            }
            Tool::Blur => {
                self.blur.color = color;
                self.blur.size = size;
            }
            Tool::Marker => {
                self.marker.color = color;
                self.marker.size = size;
            }
            Tool::StepMarker => {
                self.step_marker.color = color;
                self.step_marker.size = size;
            }
        }
    }
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
