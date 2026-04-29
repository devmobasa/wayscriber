//! Drawing tool selection.

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
