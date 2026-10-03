//! Adapters between persisted preset values and live per-tool drawing settings.

use super::{PerToolDrawingSettings, ToolDrawingSettings, ToolSettingsSlot, ToolSizeSource};
use crate::config::{ColorSpec, PresetToolSettingConfig, PresetToolStatesConfig};
use crate::domain::Tool;

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
        self.setting_for_slot(tool.settings_slot()).color.clone()
    }

    pub fn size_for_tool(&self, tool: Tool) -> f64 {
        let profile = tool.profile();
        match profile.size_source {
            ToolSizeSource::EraserSize => self.eraser_size,
            ToolSizeSource::DrawingThickness => self.setting_for_slot(profile.settings_slot).size,
        }
    }

    #[allow(dead_code)]
    pub fn set_preview_tool(&mut self, tool: Tool, color: ColorSpec, size: f64) {
        let profile = tool.profile();
        self.setting_for_slot_mut(profile.settings_slot).color = color;
        match profile.size_source {
            ToolSizeSource::EraserSize => self.eraser_size = size,
            ToolSizeSource::DrawingThickness => {
                self.setting_for_slot_mut(profile.settings_slot).size = size;
            }
        }
    }

    fn setting_for_slot(&self, slot: ToolSettingsSlot) -> &PresetToolSettingConfig {
        match slot {
            ToolSettingsSlot::Pen => &self.pen,
            ToolSettingsSlot::Line => &self.line,
            ToolSettingsSlot::Rect => &self.rect,
            ToolSettingsSlot::Ellipse => &self.ellipse,
            ToolSettingsSlot::Arrow => &self.arrow,
            ToolSettingsSlot::Blur => &self.blur,
            ToolSettingsSlot::Marker => &self.marker,
            ToolSettingsSlot::StepMarker => &self.step_marker,
        }
    }

    fn setting_for_slot_mut(&mut self, slot: ToolSettingsSlot) -> &mut PresetToolSettingConfig {
        match slot {
            ToolSettingsSlot::Pen => &mut self.pen,
            ToolSettingsSlot::Line => &mut self.line,
            ToolSettingsSlot::Rect => &mut self.rect,
            ToolSettingsSlot::Ellipse => &mut self.ellipse,
            ToolSettingsSlot::Arrow => &mut self.arrow,
            ToolSettingsSlot::Blur => &mut self.blur,
            ToolSettingsSlot::Marker => &mut self.marker,
            ToolSettingsSlot::StepMarker => &mut self.step_marker,
        }
    }
}
