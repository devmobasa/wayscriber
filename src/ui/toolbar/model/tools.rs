use crate::config::ToolbarItemId;
use crate::input::Tool;
use crate::ui::toolbar::ToolbarSnapshot;

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

pub(crate) fn visible_top_tool_buttons(
    simple: bool,
    snapshot: &ToolbarSnapshot,
) -> impl Iterator<Item = Tool> + '_ {
    visible_tools(top_tool_buttons(simple), snapshot)
}

pub(crate) fn visible_tools<'a>(
    tools: &'static [Tool],
    snapshot: &'a ToolbarSnapshot,
) -> impl Iterator<Item = Tool> + 'a {
    tools
        .iter()
        .copied()
        .filter(move |tool| tool_visible(snapshot, *tool))
}

pub(crate) fn visible_tool_count(tools: &'static [Tool], snapshot: &ToolbarSnapshot) -> usize {
    visible_tools(tools, snapshot).count()
}

pub(crate) fn tool_visible(snapshot: &ToolbarSnapshot, tool: Tool) -> bool {
    toolbar_item_visible(snapshot, toolbar_item_id_for_tool(tool).as_str())
}

pub(crate) fn toolbar_item_visible(snapshot: &ToolbarSnapshot, id: &'static str) -> bool {
    !snapshot.toolbar_item_hidden(ToolbarItemId::from_known(id))
}

pub(crate) fn top_shape_picker_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, "top.utility.shape-picker")
}

pub(crate) fn top_fill_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, "top.utility.fill")
}

pub(crate) fn top_text_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, "top.utility.text")
}

pub(crate) fn top_sticky_note_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, "top.utility.sticky-note")
}

pub(crate) fn top_clear_canvas_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, "top.utility.clear-canvas")
}

pub(crate) fn top_highlight_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, "top.utility.highlight")
}

pub(crate) fn top_highlight_ring_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, "top.utility.highlight-ring")
}

pub(crate) fn top_icon_mode_toggle_visible(snapshot: &ToolbarSnapshot) -> bool {
    if snapshot.use_icons {
        toolbar_item_visible(snapshot, "top.utility.icon-mode-text")
    } else {
        toolbar_item_visible(snapshot, "top.utility.icon-mode-icons")
    }
}

fn toolbar_item_id_for_tool(tool: Tool) -> ToolbarItemId {
    ToolbarItemId::from_known(match tool {
        Tool::Select => "top.tool.select",
        Tool::Pen => "top.tool.pen",
        Tool::Line => "top.tool.line",
        Tool::Rect => "top.tool.rect",
        Tool::Ellipse => "top.tool.ellipse",
        Tool::Triangle => "top.tool.triangle",
        Tool::Parallelogram => "top.tool.parallelogram",
        Tool::Rhombus => "top.tool.rhombus",
        Tool::RegularPolygon => "top.tool.regular-polygon",
        Tool::FreeformPolygon => "top.tool.freeform-polygon",
        Tool::Arrow => "top.tool.arrow",
        Tool::Blur => "top.tool.blur",
        Tool::Marker => "top.tool.marker",
        Tool::Highlight => "top.utility.highlight",
        Tool::StepMarker => "top.tool.step-marker",
        Tool::Eraser => "top.tool.eraser",
    })
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
