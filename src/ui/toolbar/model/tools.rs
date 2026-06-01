use crate::input::Tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticToolIcon {
    Select,
    Pen,
    Line,
    Rect,
    Circle,
    Triangle,
    Parallelogram,
    Rhombus,
    Polygon,
    FreeformPolygon,
    Arrow,
    Blur,
    Marker,
    Highlight,
    StepMarker,
    Eraser,
}

const SIMPLE_TOOL_BUTTONS: [Tool; 5] = [
    Tool::Select,
    Tool::Pen,
    Tool::Marker,
    Tool::StepMarker,
    Tool::Eraser,
];

const FULL_TOOL_BUTTONS: [Tool; 10] = [
    Tool::Select,
    Tool::Pen,
    Tool::Marker,
    Tool::StepMarker,
    Tool::Eraser,
    Tool::Line,
    Tool::Rect,
    Tool::Ellipse,
    Tool::Arrow,
    Tool::Blur,
];

const COMMON_SHAPE_TOOLS: [Tool; 5] = [
    Tool::Line,
    Tool::Rect,
    Tool::Ellipse,
    Tool::Arrow,
    Tool::Blur,
];

const SHAPE_TOOLS: [Tool; 10] = [
    Tool::Line,
    Tool::Rect,
    Tool::Ellipse,
    Tool::Arrow,
    Tool::Blur,
    Tool::Triangle,
    Tool::Parallelogram,
    Tool::Rhombus,
    Tool::RegularPolygon,
    Tool::FreeformPolygon,
];

const POLYGON_TOOLS: [Tool; 5] = [
    Tool::Triangle,
    Tool::Parallelogram,
    Tool::Rhombus,
    Tool::RegularPolygon,
    Tool::FreeformPolygon,
];

pub(crate) fn top_tool_buttons(simple: bool) -> &'static [Tool] {
    if simple {
        &SIMPLE_TOOL_BUTTONS
    } else {
        &FULL_TOOL_BUTTONS
    }
}

pub(crate) fn shape_tools() -> &'static [Tool] {
    &SHAPE_TOOLS
}

pub(crate) fn common_shape_tools() -> &'static [Tool] {
    &COMMON_SHAPE_TOOLS
}

pub(crate) fn polygon_tools() -> &'static [Tool] {
    &POLYGON_TOOLS
}

pub(crate) fn is_polygon_tool(tool: Tool) -> bool {
    polygon_tools().contains(&tool)
}

pub(crate) fn semantic_icon_for_tool(tool: Tool) -> SemanticToolIcon {
    match tool {
        Tool::Select => SemanticToolIcon::Select,
        Tool::Pen => SemanticToolIcon::Pen,
        Tool::Line => SemanticToolIcon::Line,
        Tool::Rect => SemanticToolIcon::Rect,
        Tool::Ellipse => SemanticToolIcon::Circle,
        Tool::Triangle => SemanticToolIcon::Triangle,
        Tool::Parallelogram => SemanticToolIcon::Parallelogram,
        Tool::Rhombus => SemanticToolIcon::Rhombus,
        Tool::RegularPolygon => SemanticToolIcon::Polygon,
        Tool::FreeformPolygon => SemanticToolIcon::FreeformPolygon,
        Tool::Arrow => SemanticToolIcon::Arrow,
        Tool::Blur => SemanticToolIcon::Blur,
        Tool::Marker => SemanticToolIcon::Marker,
        Tool::Highlight => SemanticToolIcon::Highlight,
        Tool::StepMarker => SemanticToolIcon::StepMarker,
        Tool::Eraser => SemanticToolIcon::Eraser,
    }
}

pub(crate) fn default_drag_hint(tool: Tool) -> Option<&'static str> {
    match tool {
        Tool::Line => Some("Shift+Drag"),
        Tool::Rect => Some("Ctrl+Drag"),
        Tool::Ellipse => Some("Tab+Drag"),
        Tool::Arrow => Some("Ctrl+Shift+Drag"),
        _ => None,
    }
}

pub(crate) fn is_shape_tool(tool: Tool) -> bool {
    shape_tools().contains(&tool) || polygon_tools().contains(&tool)
}

pub(crate) fn is_fill_tool(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Rect
            | Tool::Ellipse
            | Tool::Triangle
            | Tool::Parallelogram
            | Tool::Rhombus
            | Tool::RegularPolygon
            | Tool::FreeformPolygon
    )
}

pub(crate) fn fill_tool_active(active_tool: Tool, tool_override: Option<Tool>) -> bool {
    tool_override.is_some_and(is_fill_tool) || is_fill_tool(active_tool)
}

pub(crate) fn current_shape_tool(active_tool: Tool, tool_override: Option<Tool>) -> Option<Tool> {
    tool_override
        .filter(|tool| is_shape_tool(*tool))
        .or_else(|| is_shape_tool(active_tool).then_some(active_tool))
}

pub(crate) fn default_shape_tool() -> Tool {
    Tool::Rect
}

pub(crate) fn default_polygon_tool() -> Tool {
    Tool::RegularPolygon
}
