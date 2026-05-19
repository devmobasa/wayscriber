use serde::{Deserialize, Serialize};

use super::Tool;

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
        tool.descriptor().drag_tool
    }

    pub fn as_tool(self) -> Option<Tool> {
        if self == Self::Default {
            return None;
        }
        Tool::ALL
            .iter()
            .copied()
            .find(|tool| tool.descriptor().drag_tool == self)
    }
}
