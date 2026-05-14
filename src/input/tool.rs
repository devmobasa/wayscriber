//! Drawing tool selection.

use crate::draw::Color;
use serde::{Deserialize, Serialize};

/// Drawing tool selection.
///
/// The active tool determines what shape is created when the user drags the mouse.
/// Drag modifier mappings are configurable via `[drawing]` drag-tool fields.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tool {
    /// Select/cursor tool - interact with UI without drawing
    Select,
    /// Freehand drawing - follows mouse path (default, no modifiers)
    Pen,
    /// Straight line - between start and end points (Shift)
    Line,
    /// Rectangle outline - from corner to corner (Ctrl)
    Rect,
    /// Ellipse/circle outline - from center outward (Tab)
    Ellipse,
    /// Arrow with directional head (Ctrl+Shift)
    Arrow,
    /// Privacy blur rectangle over the captured background
    Blur,
    /// Semi-transparent marker stroke for highlighting text
    Marker,
    /// Highlight-only tool (no drawing, emits click highlight)
    Highlight,
    /// Numbered step marker tool (places auto-incrementing bubbles)
    StepMarker,
    /// Eraser brush that removes content within its stroke
    Eraser,
    // Note: Text mode uses DrawingState::TextInput instead of Tool::Text
}

/// The stored color/thickness slot used by a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolSettingsSlot {
    Pen,
    Line,
    Rect,
    Ellipse,
    Arrow,
    Blur,
    Marker,
    StepMarker,
}

impl ToolSettingsSlot {
    pub(crate) const ALL: [Self; 8] = [
        Self::Pen,
        Self::Line,
        Self::Rect,
        Self::Ellipse,
        Self::Arrow,
        Self::Blur,
        Self::Marker,
        Self::StepMarker,
    ];

    pub(crate) fn representative_tool(self) -> Tool {
        match self {
            Self::Pen => Tool::Pen,
            Self::Line => Tool::Line,
            Self::Rect => Tool::Rect,
            Self::Ellipse => Tool::Ellipse,
            Self::Arrow => Tool::Arrow,
            Self::Blur => Tool::Blur,
            Self::Marker => Tool::Marker,
            Self::StepMarker => Tool::StepMarker,
        }
    }
}

/// Where a tool's visible size value is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolSizeSource {
    DrawingThickness,
    EraserSize,
}

/// Side-toolbar control family exposed by a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolControlGroup {
    None,
    Stroke,
    Marker,
    Eraser,
    Shape,
    Arrow,
    StepMarker,
}

/// Catalog entry describing the settings and controls for one drawing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolProfile {
    pub(crate) settings_slot: ToolSettingsSlot,
    pub(crate) size_source: ToolSizeSource,
    pub(crate) control_group: ToolControlGroup,
    pub(crate) needs_color: bool,
    pub(crate) thickness_label: &'static str,
}

impl ToolProfile {
    pub(crate) fn needs_thickness_control(self) -> bool {
        !matches!(self.control_group, ToolControlGroup::None)
    }

    pub(crate) fn show_fill_toggle(self) -> bool {
        matches!(self.control_group, ToolControlGroup::Shape)
    }

    pub(crate) fn show_arrow_labels(self) -> bool {
        matches!(self.control_group, ToolControlGroup::Arrow)
    }

    pub(crate) fn show_step_counter(self) -> bool {
        matches!(self.control_group, ToolControlGroup::StepMarker)
    }

    pub(crate) fn show_eraser_mode(self) -> bool {
        matches!(self.control_group, ToolControlGroup::Eraser)
    }

    pub(crate) fn show_marker_opacity(self) -> bool {
        matches!(self.control_group, ToolControlGroup::Marker)
    }
}

impl Tool {
    pub(crate) fn profile(self) -> ToolProfile {
        match self {
            Self::Select | Self::Highlight => ToolProfile {
                settings_slot: ToolSettingsSlot::Pen,
                size_source: ToolSizeSource::DrawingThickness,
                control_group: ToolControlGroup::None,
                needs_color: false,
                thickness_label: "",
            },
            Self::Pen | Self::Line => ToolProfile {
                settings_slot: if self == Self::Pen {
                    ToolSettingsSlot::Pen
                } else {
                    ToolSettingsSlot::Line
                },
                size_source: ToolSizeSource::DrawingThickness,
                control_group: ToolControlGroup::Stroke,
                needs_color: true,
                thickness_label: "Thickness",
            },
            Self::Blur => ToolProfile {
                settings_slot: ToolSettingsSlot::Blur,
                size_source: ToolSizeSource::DrawingThickness,
                control_group: ToolControlGroup::Stroke,
                needs_color: false,
                thickness_label: "Blur",
            },
            Self::Marker => ToolProfile {
                settings_slot: ToolSettingsSlot::Marker,
                size_source: ToolSizeSource::DrawingThickness,
                control_group: ToolControlGroup::Marker,
                needs_color: true,
                thickness_label: "Thickness",
            },
            Self::Eraser => ToolProfile {
                settings_slot: ToolSettingsSlot::Pen,
                size_source: ToolSizeSource::EraserSize,
                control_group: ToolControlGroup::Eraser,
                needs_color: false,
                thickness_label: "Eraser Size",
            },
            Self::Rect | Self::Ellipse => ToolProfile {
                settings_slot: if self == Self::Rect {
                    ToolSettingsSlot::Rect
                } else {
                    ToolSettingsSlot::Ellipse
                },
                size_source: ToolSizeSource::DrawingThickness,
                control_group: ToolControlGroup::Shape,
                needs_color: true,
                thickness_label: "Thickness",
            },
            Self::Arrow => ToolProfile {
                settings_slot: ToolSettingsSlot::Arrow,
                size_source: ToolSizeSource::DrawingThickness,
                control_group: ToolControlGroup::Arrow,
                needs_color: true,
                thickness_label: "Thickness",
            },
            Self::StepMarker => ToolProfile {
                settings_slot: ToolSettingsSlot::StepMarker,
                size_source: ToolSizeSource::DrawingThickness,
                control_group: ToolControlGroup::StepMarker,
                needs_color: true,
                thickness_label: "Size",
            },
        }
    }

    pub(crate) fn settings_slot(self) -> ToolSettingsSlot {
        self.profile().settings_slot
    }

    pub(crate) fn settings_tool(self) -> Tool {
        self.settings_slot().representative_tool()
    }

    pub(crate) fn uses_eraser_size(self) -> bool {
        matches!(self.profile().size_source, ToolSizeSource::EraserSize)
    }

    pub(crate) fn uses_drawing_thickness(self) -> bool {
        matches!(self.profile().size_source, ToolSizeSource::DrawingThickness)
    }

    pub(crate) fn uses_marker_opacity(self) -> bool {
        self.profile().show_marker_opacity()
    }
}

/// Color and thickness stored independently for a drawing tool.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToolDrawingSettings {
    pub color: Color,
    pub thickness: f64,
}

impl ToolDrawingSettings {
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

    pub fn settings_tool(tool: Tool) -> Tool {
        tool.settings_tool()
    }

    pub fn get(&self, tool: Tool) -> &ToolDrawingSettings {
        self.get_slot(tool.settings_slot())
    }

    pub fn get_mut(&mut self, tool: Tool) -> &mut ToolDrawingSettings {
        self.get_slot_mut(tool.settings_slot())
    }

    pub(crate) fn get_slot(&self, slot: ToolSettingsSlot) -> &ToolDrawingSettings {
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

    pub(crate) fn get_slot_mut(&mut self, slot: ToolSettingsSlot) -> &mut ToolDrawingSettings {
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

    pub fn clamp_thicknesses(mut self, min: f64, max: f64) -> Self {
        for slot in ToolSettingsSlot::ALL {
            let settings = self.get_slot_mut(slot);
            settings.thickness = settings.thickness.clamp(min, max);
        }
        self
    }
}

/// Tool/action selected by a drag binding.
///
/// `Default` preserves a mouse button's built-in behavior, such as right-click
/// context menus or middle-click radial menu toggles.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DragTool {
    /// Preserve the button's built-in behavior.
    Default,
    /// Select/cursor tool.
    Select,
    /// Freehand drawing.
    Pen,
    /// Straight line.
    Line,
    /// Rectangle outline.
    Rect,
    /// Ellipse/circle outline.
    Ellipse,
    /// Arrow with directional head.
    Arrow,
    /// Privacy blur rectangle.
    Blur,
    /// Semi-transparent marker stroke.
    Marker,
    /// Highlight-only tool.
    Highlight,
    /// Numbered step marker tool.
    StepMarker,
    /// Eraser brush.
    Eraser,
}

impl DragTool {
    pub fn from_tool(tool: Tool) -> Self {
        match tool {
            Tool::Select => Self::Select,
            Tool::Pen => Self::Pen,
            Tool::Line => Self::Line,
            Tool::Rect => Self::Rect,
            Tool::Ellipse => Self::Ellipse,
            Tool::Arrow => Self::Arrow,
            Tool::Blur => Self::Blur,
            Tool::Marker => Self::Marker,
            Tool::Highlight => Self::Highlight,
            Tool::StepMarker => Self::StepMarker,
            Tool::Eraser => Self::Eraser,
        }
    }

    pub fn as_tool(self) -> Option<Tool> {
        match self {
            Self::Default => None,
            Self::Select => Some(Tool::Select),
            Self::Pen => Some(Tool::Pen),
            Self::Line => Some(Tool::Line),
            Self::Rect => Some(Tool::Rect),
            Self::Ellipse => Some(Tool::Ellipse),
            Self::Arrow => Some(Tool::Arrow),
            Self::Blur => Some(Tool::Blur),
            Self::Marker => Some(Tool::Marker),
            Self::Highlight => Some(Tool::Highlight),
            Self::StepMarker => Some(Tool::StepMarker),
            Self::Eraser => Some(Tool::Eraser),
        }
    }
}

/// Eraser behavior mode.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum EraserMode {
    /// Brush-style eraser that clears pixels along its stroke.
    #[default]
    Brush,
    /// Stroke eraser that deletes any shape it touches.
    Stroke,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(r: f64) -> Color {
        Color {
            r,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }
    }

    #[test]
    fn tool_profile_maps_compatibility_tools_to_pen_settings() {
        assert_eq!(Tool::Select.settings_slot(), ToolSettingsSlot::Pen);
        assert_eq!(Tool::Highlight.settings_slot(), ToolSettingsSlot::Pen);
        assert_eq!(Tool::Eraser.settings_slot(), ToolSettingsSlot::Pen);
        assert_eq!(
            Tool::Eraser.profile().size_source,
            ToolSizeSource::EraserSize
        );

        for slot in ToolSettingsSlot::ALL {
            assert_eq!(slot.representative_tool().settings_slot(), slot);
        }
    }

    #[test]
    fn tool_profile_describes_toolbar_control_groups() {
        assert!(!Tool::Select.profile().needs_thickness_control());
        assert_eq!(Tool::Blur.profile().thickness_label, "Blur");
        assert!(Tool::Marker.profile().show_marker_opacity());
        assert!(Tool::Eraser.profile().show_eraser_mode());
        assert!(Tool::Rect.profile().show_fill_toggle());
        assert!(Tool::Arrow.profile().show_arrow_labels());
        assert!(Tool::StepMarker.profile().show_step_counter());
    }

    #[test]
    fn per_tool_settings_read_and_write_through_catalog_slot() {
        let mut settings = PerToolDrawingSettings::new(color(1.0), 4.0);
        settings.marker = ToolDrawingSettings::new(color(0.5), 12.0);

        assert_eq!(settings.get(Tool::Eraser), &settings.pen);
        assert_eq!(settings.get(Tool::Marker), &settings.marker);

        settings.get_mut(Tool::Highlight).thickness = 8.0;
        settings.get_mut(Tool::Marker).thickness = 16.0;

        assert_eq!(settings.pen.thickness, 8.0);
        assert_eq!(settings.marker.thickness, 16.0);
    }
}
