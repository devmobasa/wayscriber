use crate::config::ToolbarItemOrderGroup;
use crate::input::Tool;
use crate::ui::toolbar::{ToolbarSideSection, ToolbarSnapshot};

use super::catalog::{self, TopUtilityButton};

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

const SHAPE_PICKER_ROW_LEN: usize = 5;

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
    ordered_tools(snapshot)
        .into_iter()
        .filter(move |tool| tools.contains(tool) && tool_visible(snapshot, *tool))
}

pub(crate) fn visible_tool_count(tools: &'static [Tool], snapshot: &ToolbarSnapshot) -> usize {
    visible_tools(tools, snapshot).count()
}

pub(crate) fn tool_visible(snapshot: &ToolbarSnapshot, tool: Tool) -> bool {
    catalog::toolbar_item_visible(snapshot, catalog::toolbar_item_id_for_tool(tool))
}

pub(crate) fn top_shape_picker_visible(snapshot: &ToolbarSnapshot) -> bool {
    catalog::top_shape_picker_visible(snapshot)
}

pub(crate) fn top_fill_visible(snapshot: &ToolbarSnapshot) -> bool {
    catalog::top_fill_visible(snapshot)
}

pub(crate) fn top_text_visible(snapshot: &ToolbarSnapshot) -> bool {
    catalog::top_text_visible(snapshot)
}

pub(crate) fn top_sticky_note_visible(snapshot: &ToolbarSnapshot) -> bool {
    catalog::top_sticky_note_visible(snapshot)
}

pub(crate) fn top_clear_canvas_visible(snapshot: &ToolbarSnapshot) -> bool {
    catalog::top_clear_canvas_visible(snapshot)
}

pub(crate) fn top_screenshot_visible(snapshot: &ToolbarSnapshot) -> bool {
    catalog::top_screenshot_visible(snapshot)
}

pub(crate) fn top_highlight_visible(snapshot: &ToolbarSnapshot) -> bool {
    catalog::top_highlight_visible(snapshot)
}

pub(crate) fn top_highlight_ring_visible(snapshot: &ToolbarSnapshot) -> bool {
    catalog::top_highlight_ring_visible(snapshot)
}

pub(crate) fn top_icon_mode_toggle_visible(snapshot: &ToolbarSnapshot) -> bool {
    catalog::top_icon_mode_toggle_visible(snapshot)
}

fn ordered_tools(snapshot: &ToolbarSnapshot) -> Vec<Tool> {
    snapshot
        .resolved_toolbar_items
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .iter()
        .filter_map(|id| catalog::tool_for_toolbar_item_id(*id))
        .collect()
}

const DEFAULT_TOP_UTILITY_BUTTONS: [TopUtilityButton; 5] = [
    TopUtilityButton::Text,
    TopUtilityButton::StickyNote,
    TopUtilityButton::Screenshot,
    TopUtilityButton::ClearCanvas,
    TopUtilityButton::Highlight,
];

pub(crate) fn visible_top_utility_buttons(
    snapshot: &ToolbarSnapshot,
    simple: bool,
    use_icons: bool,
) -> Vec<TopUtilityButton> {
    let mut ordered = Vec::with_capacity(DEFAULT_TOP_UTILITY_BUTTONS.len());
    for id in snapshot
        .resolved_toolbar_items
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopControls)
    {
        if let Some(button) = catalog::top_utility_button_for_item_id(*id)
            && top_utility_button_visible(snapshot, button, simple, use_icons)
            && !ordered.contains(&button)
        {
            ordered.push(button);
        }
    }
    for button in DEFAULT_TOP_UTILITY_BUTTONS {
        if top_utility_button_visible(snapshot, button, simple, use_icons)
            && !ordered.contains(&button)
        {
            ordered.push(button);
        }
    }
    ordered
}

fn top_utility_button_visible(
    snapshot: &ToolbarSnapshot,
    button: TopUtilityButton,
    simple: bool,
    use_icons: bool,
) -> bool {
    match button {
        TopUtilityButton::Text => top_text_visible(snapshot),
        TopUtilityButton::StickyNote => top_sticky_note_visible(snapshot),
        TopUtilityButton::Screenshot => top_screenshot_visible(snapshot),
        TopUtilityButton::ClearCanvas => !simple && top_clear_canvas_visible(snapshot),
        TopUtilityButton::Highlight => !simple && use_icons && top_highlight_visible(snapshot),
        TopUtilityButton::IconMode => false,
    }
}

pub(crate) fn shape_tools() -> &'static [Tool] {
    &SHAPE_TOOLS
}

pub(crate) fn polygon_tools() -> &'static [Tool] {
    &POLYGON_TOOLS
}

pub(crate) fn visible_shape_picker_rows(
    snapshot: &ToolbarSnapshot,
    is_simple: bool,
) -> Vec<Vec<Tool>> {
    visible_tools(shape_picker_tools(is_simple), snapshot)
        .collect::<Vec<_>>()
        .chunks(SHAPE_PICKER_ROW_LEN)
        .map(|chunk| chunk.to_vec())
        .collect()
}

pub(crate) fn visible_shape_picker_row_count(snapshot: &ToolbarSnapshot, is_simple: bool) -> usize {
    visible_tools(shape_picker_tools(is_simple), snapshot)
        .count()
        .div_ceil(SHAPE_PICKER_ROW_LEN)
}

pub(crate) fn visible_shape_picker_max_row_len(
    snapshot: &ToolbarSnapshot,
    is_simple: bool,
) -> usize {
    visible_tools(shape_picker_tools(is_simple), snapshot)
        .count()
        .min(SHAPE_PICKER_ROW_LEN)
}

fn shape_picker_tools(is_simple: bool) -> &'static [Tool] {
    if is_simple {
        &SHAPE_TOOLS
    } else {
        &POLYGON_TOOLS
    }
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

pub(crate) fn ordered_side_sections(snapshot: &ToolbarSnapshot) -> Vec<ToolbarSideSection> {
    snapshot
        .resolved_toolbar_items
        .order
        .ordered_ids(ToolbarItemOrderGroup::SideSections)
        .iter()
        .filter_map(|id| catalog::side_section_for_toolbar_item_id(*id))
        .collect()
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
