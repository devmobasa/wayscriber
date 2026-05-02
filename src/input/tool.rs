//! Drawing tool selection.

use crate::draw::Color;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Drawing tool selection.
///
/// The active tool determines what shape is created when the user drags the mouse.
/// Drag modifier mappings are configurable via `[drawing]` drag-tool fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
        match tool {
            Tool::Select | Tool::Highlight | Tool::Eraser => Tool::Pen,
            _ => tool,
        }
    }

    pub fn get(&self, tool: Tool) -> &ToolDrawingSettings {
        match tool {
            Tool::Pen | Tool::Select | Tool::Highlight | Tool::Eraser => &self.pen,
            Tool::Line => &self.line,
            Tool::Rect => &self.rect,
            Tool::Ellipse => &self.ellipse,
            Tool::Arrow => &self.arrow,
            Tool::Blur => &self.blur,
            Tool::Marker => &self.marker,
            Tool::StepMarker => &self.step_marker,
        }
    }

    pub fn get_mut(&mut self, tool: Tool) -> &mut ToolDrawingSettings {
        match tool {
            Tool::Pen | Tool::Select | Tool::Highlight | Tool::Eraser => &mut self.pen,
            Tool::Line => &mut self.line,
            Tool::Rect => &mut self.rect,
            Tool::Ellipse => &mut self.ellipse,
            Tool::Arrow => &mut self.arrow,
            Tool::Blur => &mut self.blur,
            Tool::Marker => &mut self.marker,
            Tool::StepMarker => &mut self.step_marker,
        }
    }

    pub fn clamp_thicknesses(mut self, min: f64, max: f64) -> Self {
        self.pen.thickness = self.pen.thickness.clamp(min, max);
        self.line.thickness = self.line.thickness.clamp(min, max);
        self.rect.thickness = self.rect.thickness.clamp(min, max);
        self.ellipse.thickness = self.ellipse.thickness.clamp(min, max);
        self.arrow.thickness = self.arrow.thickness.clamp(min, max);
        self.blur.thickness = self.blur.thickness.clamp(min, max);
        self.marker.thickness = self.marker.thickness.clamp(min, max);
        self.step_marker.thickness = self.step_marker.thickness.clamp(min, max);
        self
    }
}

/// Tool/action selected by a drag binding.
///
/// `Default` preserves a mouse button's built-in behavior, such as right-click
/// context menus or middle-click radial menu toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub enum EraserMode {
    /// Brush-style eraser that clears pixels along its stroke.
    #[default]
    Brush,
    /// Stroke eraser that deletes any shape it touches.
    Stroke,
}
