use crate::domain::{Color, Tool};
use serde::{Deserialize, Serialize};

use super::ToolSettingsSlot;

/// Color and thickness stored independently for a drawing tool.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToolDrawingSettings {
    pub color: Color,
    pub thickness: f64,
}

impl ToolDrawingSettings {
    #[must_use]
    pub fn new(color: Color, thickness: f64) -> Self {
        Self { color, thickness }
    }
}

/// Independent color/thickness settings for tools that draw with them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerToolDrawingSettings {
    pub pen: ToolDrawingSettings,
    pub line: ToolDrawingSettings,
    pub rect: ToolDrawingSettings,
    pub ellipse: ToolDrawingSettings,
    pub arrow: ToolDrawingSettings,
    pub blur: ToolDrawingSettings,
    pub marker: ToolDrawingSettings,
    pub step_marker: ToolDrawingSettings,
}

impl PerToolDrawingSettings {
    #[must_use]
    pub fn new(color: Color, thickness: f64) -> Self {
        let settings = ToolDrawingSettings::new(color, thickness);
        Self {
            pen: settings,
            line: settings,
            rect: settings,
            ellipse: settings,
            arrow: settings,
            blur: settings,
            marker: settings,
            step_marker: settings,
        }
    }

    #[must_use]
    pub fn settings_tool(tool: Tool) -> Tool {
        tool.settings_tool()
    }

    #[must_use]
    pub fn get(&self, tool: Tool) -> &ToolDrawingSettings {
        self.slot(tool.settings_slot())
    }

    pub fn get_mut(&mut self, tool: Tool) -> &mut ToolDrawingSettings {
        self.slot_mut(tool.settings_slot())
    }

    pub(crate) fn slot(&self, slot: ToolSettingsSlot) -> &ToolDrawingSettings {
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

    pub(crate) fn slot_mut(&mut self, slot: ToolSettingsSlot) -> &mut ToolDrawingSettings {
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

    #[must_use]
    pub fn clamp_thicknesses(mut self, min: f64, max: f64) -> Self {
        for slot in ToolSettingsSlot::ALL {
            let settings = self.slot_mut(slot);
            settings.thickness = settings.thickness.clamp(min, max);
        }
        self
    }
}
