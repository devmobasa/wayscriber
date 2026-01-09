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
        true
    ),
    meta!(
        EnterStickyNoteMode,
        "Sticky Note",
        Some("Note"),
        "Add sticky note",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        SelectPenTool,
        "Pen Tool",
        Some("Pen"),
        "Freehand drawing",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        SelectCursorTool,
        "Select Tool",
        Some("Select"),
        "Cursor/select tool",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        SelectLineTool,
        "Line Tool",
        Some("Line"),
        "Draw straight lines",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        SelectRectTool,
        "Rectangle Tool",
        Some("Rect"),
        "Draw rectangles",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        SelectEllipseTool,
        "Ellipse Tool",
        Some("Circle"),
        "Draw ellipses and circles",
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
        true
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
        true
    ),
    meta!(
        SelectEraserTool,
        "Eraser Tool",
        Some("Eraser"),
        "Erase drawings",
        Tools,
        true,
        true,
        true
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
