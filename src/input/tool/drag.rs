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
    /// Triangle generated from drag bounds.
    Triangle,
    /// Parallelogram generated from drag bounds.
    Parallelogram,
    /// Rhombus/diamond generated from drag bounds.
    Rhombus,
    /// Regular polygon generated from drag bounds.
    RegularPolygon,
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
    pub fn from_tool(tool: Tool) -> Option<Self> {
        let drag_tool = tool.drag_tool();
        debug_assert_eq!(
            drag_tool,
            DragBindableTool::from_tool(tool).map(DragBindableTool::to_drag_tool)
        );
        drag_tool
    }

    pub fn as_tool(self) -> Option<Tool> {
        DragBindableTool::from_drag_tool(self).map(DragBindableTool::to_tool)
    }
}

/// Config-facing tool list for legacy flat drag fields.
///
/// It intentionally excludes `DragTool::Default` and non-drag selectable tools.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DragBindableTool {
    Select,
    Pen,
    Line,
    Rect,
    Ellipse,
    Triangle,
    Parallelogram,
    Rhombus,
    RegularPolygon,
    Arrow,
    Blur,
    Marker,
    Highlight,
    StepMarker,
    Eraser,
}

impl DragBindableTool {
    pub fn to_drag_tool(self) -> DragTool {
        match self {
            Self::Select => DragTool::Select,
            Self::Pen => DragTool::Pen,
            Self::Line => DragTool::Line,
            Self::Rect => DragTool::Rect,
            Self::Ellipse => DragTool::Ellipse,
            Self::Triangle => DragTool::Triangle,
            Self::Parallelogram => DragTool::Parallelogram,
            Self::Rhombus => DragTool::Rhombus,
            Self::RegularPolygon => DragTool::RegularPolygon,
            Self::Arrow => DragTool::Arrow,
            Self::Blur => DragTool::Blur,
            Self::Marker => DragTool::Marker,
            Self::Highlight => DragTool::Highlight,
            Self::StepMarker => DragTool::StepMarker,
            Self::Eraser => DragTool::Eraser,
        }
    }

    pub fn to_tool(self) -> Tool {
        match self {
            Self::Select => Tool::Select,
            Self::Pen => Tool::Pen,
            Self::Line => Tool::Line,
            Self::Rect => Tool::Rect,
            Self::Ellipse => Tool::Ellipse,
            Self::Triangle => Tool::Triangle,
            Self::Parallelogram => Tool::Parallelogram,
            Self::Rhombus => Tool::Rhombus,
            Self::RegularPolygon => Tool::RegularPolygon,
            Self::Arrow => Tool::Arrow,
            Self::Blur => Tool::Blur,
            Self::Marker => Tool::Marker,
            Self::Highlight => Tool::Highlight,
            Self::StepMarker => Tool::StepMarker,
            Self::Eraser => Tool::Eraser,
        }
    }

    pub fn from_tool(tool: Tool) -> Option<Self> {
        match tool {
            Tool::Select => Some(Self::Select),
            Tool::Pen => Some(Self::Pen),
            Tool::Line => Some(Self::Line),
            Tool::Rect => Some(Self::Rect),
            Tool::Ellipse => Some(Self::Ellipse),
            Tool::Triangle => Some(Self::Triangle),
            Tool::Parallelogram => Some(Self::Parallelogram),
            Tool::Rhombus => Some(Self::Rhombus),
            Tool::RegularPolygon => Some(Self::RegularPolygon),
            Tool::FreeformPolygon => None,
            Tool::Arrow => Some(Self::Arrow),
            Tool::Blur => Some(Self::Blur),
            Tool::Marker => Some(Self::Marker),
            Tool::Highlight => Some(Self::Highlight),
            Tool::StepMarker => Some(Self::StepMarker),
            Tool::Eraser => Some(Self::Eraser),
        }
    }

    pub fn from_drag_tool(tool: DragTool) -> Option<Self> {
        match tool {
            DragTool::Default => None,
            DragTool::Select => Some(Self::Select),
            DragTool::Pen => Some(Self::Pen),
            DragTool::Line => Some(Self::Line),
            DragTool::Rect => Some(Self::Rect),
            DragTool::Ellipse => Some(Self::Ellipse),
            DragTool::Triangle => Some(Self::Triangle),
            DragTool::Parallelogram => Some(Self::Parallelogram),
            DragTool::Rhombus => Some(Self::Rhombus),
            DragTool::RegularPolygon => Some(Self::RegularPolygon),
            DragTool::Arrow => Some(Self::Arrow),
            DragTool::Blur => Some(Self::Blur),
            DragTool::Marker => Some(Self::Marker),
            DragTool::Highlight => Some(Self::Highlight),
            DragTool::StepMarker => Some(Self::StepMarker),
            DragTool::Eraser => Some(Self::Eraser),
        }
    }
}
