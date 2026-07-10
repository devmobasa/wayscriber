use crate::config::{ToolbarItemId, ToolbarItemOrderGroup, toolbar_item_ids as ids};
use crate::input::Tool;
use crate::ui::toolbar::{ToolbarSideSection, ToolbarSnapshot};

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

/// Which pane a side section belongs to. The Draw pane holds per-tool
/// drawing properties; Canvas holds canvas management; Session and Settings
/// are their own panes.
pub(crate) fn pane_for_section(
    section: crate::ui::toolbar::ToolbarSideSection,
) -> crate::ui::toolbar::SidePane {
    use crate::ui::toolbar::{SidePane, ToolbarSideSection as S};
    match section {
        S::Colors
        | S::Presets
        | S::Thickness
        | S::EraserMode
        | S::PolygonSides
        | S::ArrowLabels
        | S::StepMarkers
        | S::MarkerOpacity
        | S::TextSize
        | S::Font => SidePane::Draw,
        S::Actions | S::Boards | S::Pages | S::StepUndo => SidePane::Canvas,
        S::Session => SidePane::Session,
        S::Settings => SidePane::Settings,
    }
}

/// User-ordered side sections filtered to the active pane.
pub(crate) fn ordered_pane_sections(
    snapshot: &ToolbarSnapshot,
) -> Vec<crate::ui::toolbar::ToolbarSideSection> {
    ordered_side_sections(snapshot)
        .into_iter()
        .filter(|section| pane_for_section(*section) == snapshot.active_side_pane)
        .collect()
}

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
    toolbar_item_visible(snapshot, toolbar_item_id_for_tool(tool))
}

pub(crate) fn toolbar_item_visible(snapshot: &ToolbarSnapshot, id: ToolbarItemId) -> bool {
    !snapshot.toolbar_item_hidden(id)
}

pub(crate) fn top_shape_picker_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, ids::TOP_UTILITY_SHAPE_PICKER)
}

pub(crate) fn top_fill_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, ids::TOP_UTILITY_FILL)
}

pub(crate) fn top_text_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, ids::TOP_UTILITY_TEXT)
}

pub(crate) fn top_sticky_note_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, ids::TOP_UTILITY_STICKY_NOTE)
}

pub(crate) fn top_clear_canvas_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, ids::TOP_UTILITY_CLEAR_CANVAS)
}

pub(crate) fn top_screenshot_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, ids::TOP_UTILITY_SCREENSHOT)
}

pub(crate) fn top_highlight_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, ids::TOP_UTILITY_HIGHLIGHT)
}

pub(crate) fn top_highlight_ring_visible(snapshot: &ToolbarSnapshot) -> bool {
    toolbar_item_visible(snapshot, ids::TOP_UTILITY_HIGHLIGHT_RING)
}

pub(crate) fn top_icon_mode_toggle_visible(snapshot: &ToolbarSnapshot) -> bool {
    if snapshot.use_icons {
        toolbar_item_visible(snapshot, ids::TOP_UTILITY_ICON_MODE_TEXT)
    } else {
        toolbar_item_visible(snapshot, ids::TOP_UTILITY_ICON_MODE_ICONS)
    }
}

pub(crate) fn toolbar_item_id_for_tool(tool: Tool) -> ToolbarItemId {
    match tool {
        Tool::Select => ids::TOP_TOOL_SELECT,
        Tool::Pen => ids::TOP_TOOL_PEN,
        Tool::Line => ids::TOP_TOOL_LINE,
        Tool::Rect => ids::TOP_TOOL_RECT,
        Tool::Ellipse => ids::TOP_TOOL_ELLIPSE,
        Tool::Triangle => ids::TOP_TOOL_TRIANGLE,
        Tool::Parallelogram => ids::TOP_TOOL_PARALLELOGRAM,
        Tool::Rhombus => ids::TOP_TOOL_RHOMBUS,
        Tool::RegularPolygon => ids::TOP_TOOL_REGULAR_POLYGON,
        Tool::FreeformPolygon => ids::TOP_TOOL_FREEFORM_POLYGON,
        Tool::Arrow => ids::TOP_TOOL_ARROW,
        Tool::Blur => ids::TOP_TOOL_BLUR,
        Tool::Marker => ids::TOP_TOOL_MARKER,
        Tool::Highlight => ids::TOP_UTILITY_HIGHLIGHT,
        Tool::StepMarker => ids::TOP_TOOL_STEP_MARKER,
        Tool::Eraser => ids::TOP_TOOL_ERASER,
    }
}

fn tool_for_toolbar_item_id(id: ToolbarItemId) -> Option<Tool> {
    [
        (ids::TOP_TOOL_SELECT, Tool::Select),
        (ids::TOP_TOOL_PEN, Tool::Pen),
        (ids::TOP_TOOL_LINE, Tool::Line),
        (ids::TOP_TOOL_RECT, Tool::Rect),
        (ids::TOP_TOOL_ELLIPSE, Tool::Ellipse),
        (ids::TOP_TOOL_TRIANGLE, Tool::Triangle),
        (ids::TOP_TOOL_PARALLELOGRAM, Tool::Parallelogram),
        (ids::TOP_TOOL_RHOMBUS, Tool::Rhombus),
        (ids::TOP_TOOL_REGULAR_POLYGON, Tool::RegularPolygon),
        (ids::TOP_TOOL_FREEFORM_POLYGON, Tool::FreeformPolygon),
        (ids::TOP_TOOL_ARROW, Tool::Arrow),
        (ids::TOP_TOOL_BLUR, Tool::Blur),
        (ids::TOP_TOOL_MARKER, Tool::Marker),
        (ids::TOP_TOOL_STEP_MARKER, Tool::StepMarker),
        (ids::TOP_TOOL_ERASER, Tool::Eraser),
    ]
    .into_iter()
    .find_map(|(candidate, tool)| (candidate == id).then_some(tool))
}

fn ordered_tools(snapshot: &ToolbarSnapshot) -> Vec<Tool> {
    snapshot
        .resolved_toolbar_items
        .order
        .ordered_ids(ToolbarItemOrderGroup::TopTools)
        .iter()
        .filter_map(|id| tool_for_toolbar_item_id(*id))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopUtilityButton {
    Text,
    StickyNote,
    Screenshot,
    ClearCanvas,
    Highlight,
    IconMode,
}

const DEFAULT_TOP_UTILITY_BUTTONS: [TopUtilityButton; 5] = [
    TopUtilityButton::Text,
    TopUtilityButton::StickyNote,
    TopUtilityButton::Screenshot,
    TopUtilityButton::ClearCanvas,
    TopUtilityButton::Highlight,
];

impl TopUtilityButton {
    pub(crate) fn id(self, snapshot: &ToolbarSnapshot) -> ToolbarItemId {
        match self {
            Self::Text => ids::TOP_UTILITY_TEXT,
            Self::StickyNote => ids::TOP_UTILITY_STICKY_NOTE,
            Self::Screenshot => ids::TOP_UTILITY_SCREENSHOT,
            Self::ClearCanvas => ids::TOP_UTILITY_CLEAR_CANVAS,
            Self::Highlight => ids::TOP_UTILITY_HIGHLIGHT,
            Self::IconMode if snapshot.use_icons => ids::TOP_UTILITY_ICON_MODE_TEXT,
            Self::IconMode => ids::TOP_UTILITY_ICON_MODE_ICONS,
        }
    }
}

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
        if let Some(button) = top_utility_button_for_id(*id, snapshot)
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

fn top_utility_button_for_id(
    id: ToolbarItemId,
    snapshot: &ToolbarSnapshot,
) -> Option<TopUtilityButton> {
    if id == ids::TOP_UTILITY_TEXT {
        Some(TopUtilityButton::Text)
    } else if id == ids::TOP_UTILITY_STICKY_NOTE {
        Some(TopUtilityButton::StickyNote)
    } else if id == ids::TOP_UTILITY_SCREENSHOT {
        Some(TopUtilityButton::Screenshot)
    } else if id == ids::TOP_UTILITY_CLEAR_CANVAS {
        Some(TopUtilityButton::ClearCanvas)
    } else if id == ids::TOP_UTILITY_HIGHLIGHT {
        Some(TopUtilityButton::Highlight)
    } else if id == ids::TOP_UTILITY_ICON_MODE_ICONS
        || id == ids::TOP_UTILITY_ICON_MODE_TEXT
        || id == TopUtilityButton::IconMode.id(snapshot)
    {
        Some(TopUtilityButton::IconMode)
    } else {
        None
    }
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
        .filter_map(|id| side_section_for_toolbar_item_id(*id))
        .collect()
}

fn side_section_for_toolbar_item_id(id: ToolbarItemId) -> Option<ToolbarSideSection> {
    [
        (ids::SIDE_GROUP_COLORS, ToolbarSideSection::Colors),
        (ids::SIDE_GROUP_PRESETS, ToolbarSideSection::Presets),
        (ids::SIDE_GROUP_THICKNESS, ToolbarSideSection::Thickness),
        (ids::SIDE_GROUP_ERASER_MODE, ToolbarSideSection::EraserMode),
        (
            ids::SIDE_GROUP_POLYGON_SIDES,
            ToolbarSideSection::PolygonSides,
        ),
        (
            ids::SIDE_GROUP_ARROW_LABELS,
            ToolbarSideSection::ArrowLabels,
        ),
        (
            ids::SIDE_GROUP_STEP_MARKERS,
            ToolbarSideSection::StepMarkers,
        ),
        (
            ids::SIDE_GROUP_MARKER_OPACITY,
            ToolbarSideSection::MarkerOpacity,
        ),
        (ids::SIDE_GROUP_TEXT_SIZE, ToolbarSideSection::TextSize),
        (ids::SIDE_GROUP_FONT, ToolbarSideSection::Font),
        (ids::SIDE_GROUP_ACTIONS, ToolbarSideSection::Actions),
        (ids::SIDE_GROUP_BOARDS, ToolbarSideSection::Boards),
        (ids::SIDE_GROUP_PAGES, ToolbarSideSection::Pages),
        (ids::SIDE_GROUP_STEP_UNDO, ToolbarSideSection::StepUndo),
        (ids::SIDE_GROUP_SESSION, ToolbarSideSection::Session),
        (ids::SIDE_GROUP_SETTINGS, ToolbarSideSection::Settings),
    ]
    .into_iter()
    .find_map(|(candidate, section)| (candidate == id).then_some(section))
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
