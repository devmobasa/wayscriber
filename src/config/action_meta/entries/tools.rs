use super::ActionMeta;

pub const ENTRIES: &[ActionMeta] = &[
    meta!(
        EnterTextMode,
        "Text Mode",
        Some("Text"),
        "Add text annotations",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_text
    ),
    meta!(
        EnterStickyNoteMode,
        "Sticky Note",
        Some("Note"),
        "Add sticky note",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_note
    ),
    meta!(
        SelectSelectionTool,
        "Selection Tool",
        Some("Select"),
        "Select and move items",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_select
    ),
    meta!(
        SelectPenTool,
        "Pen Tool",
        Some("Pen"),
        "Freehand drawing",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_pen
    ),
    meta!(
        SelectLineTool,
        "Line Tool",
        Some("Line"),
        "Draw straight lines",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_line
    ),
    meta!(
        SelectRectTool,
        "Rectangle Tool",
        Some("Rect"),
        "Draw rectangles",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_rect
    ),
    meta!(
        SelectEllipseTool,
        "Ellipse Tool",
        Some("Circle"),
        "Draw ellipses and circles",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_circle
    ),
    meta!(
        SelectTriangleTool,
        "Triangle Tool",
        Some("Triangle"),
        "Draw triangles",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        SelectParallelogramTool,
        "Parallelogram Tool",
        Some("Parallelogram"),
        "Draw parallelograms",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        SelectRhombusTool,
        "Rhombus Tool",
        Some("Rhombus"),
        "Draw rhombuses",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        SelectRegularPolygonTool,
        "Regular Polygon Tool",
        Some("Polygon"),
        "Draw regular polygons",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_polygon
    ),
    meta!(
        SelectFreeformPolygonTool,
        "Freeform Polygon Tool",
        Some("Freeform"),
        "Build polygons by clicking vertices",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        SelectArrowTool,
        "Arrow Tool",
        Some("Arrow"),
        "Draw arrows",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_arrow
    ),
    meta!(
        SelectBlurTool,
        "Blur Tool",
        Some("Blur"),
        "Blur sensitive regions on captured backgrounds",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_blur
    ),
    meta!(
        SelectHighlightTool,
        "Highlight Tool",
        Some("Highlight"),
        "Highlight areas",
        Tools,
        true,
        false,
        true
    ),
    meta!(
        ToggleHighlightTool,
        "Toggle Highlight",
        Some("Highlight"),
        "Toggle highlight tool and click highlight",
        Tools,
        false,
        true,
        true
    ),
    meta!(
        SelectMarkerTool,
        "Marker Tool",
        Some("Marker"),
        "Semi-transparent marker",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_marker
    ),
    meta!(
        SelectStepMarkerTool,
        "Step Marker Tool",
        Some("Steps"),
        "Place numbered step markers",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_step_marker
    ),
    meta!(
        SelectEraserTool,
        "Eraser Tool",
        Some("Eraser"),
        "Erase drawings",
        Tools,
        true,
        true,
        true,
        icon: crate::toolbar_icons::draw_icon_eraser
    ),
    meta!(
        ToggleEraserMode,
        "Toggle Eraser Mode",
        None,
        "Switch to/from eraser",
        Tools,
        true,
        false,
        true
    ),
];
