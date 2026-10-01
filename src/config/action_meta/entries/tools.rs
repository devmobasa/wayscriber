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
        icon: crate::config::action_meta::ActionIcon::Text
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
        icon: crate::config::action_meta::ActionIcon::StickyNote
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
        icon: crate::config::action_meta::ActionIcon::Select
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
        icon: crate::config::action_meta::ActionIcon::Pen
    ),
    meta!(
        SelectLiveShapeTool,
        "Shape Pen Tool",
        Some("Shape Pen"),
        "Turn confident ink into lines, ellipses, rectangles, and triangles",
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
        true,
        icon: crate::config::action_meta::ActionIcon::Line
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
        icon: crate::config::action_meta::ActionIcon::Rect
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
        icon: crate::config::action_meta::ActionIcon::Ellipse
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
        icon: crate::config::action_meta::ActionIcon::FreeformPolygon
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
        icon: crate::config::action_meta::ActionIcon::Arrow
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
        icon: crate::config::action_meta::ActionIcon::Blur
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
        SelectLaserTool,
        "Laser Pointer Tool",
        Some("Laser"),
        "Point with glowing ink that fades away and is never saved",
        Tools,
        true,
        true,
        true,
        &[
            "laser",
            "pointer",
            "presenter",
            "presentation",
            "fading ink",
            "disappearing ink",
        ]
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
        icon: crate::config::action_meta::ActionIcon::Marker
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
        icon: crate::config::action_meta::ActionIcon::StepMarker
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
        icon: crate::config::action_meta::ActionIcon::Eraser
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
    meta!(
        SelectSpotlightTool,
        "Spotlight Tool",
        None,
        "Dim everything but a region",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        CycleFontFamily,
        "Cycle Font Family",
        None,
        "Step the text font through the configured list",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        OpenFontPicker,
        "Font Picker",
        None,
        "Pick a text font from every one installed",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        IncreasePenSmoothing,
        "Increase Pen Smoothing",
        None,
        "Clean up finished strokes more",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        DecreasePenSmoothing,
        "Decrease Pen Smoothing",
        None,
        "Keep more of the drawn path",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        IncreaseShapeRecognitionSensitivity,
        "Increase Shape Pen Sensitivity",
        None,
        "Recognize rougher strokes as shapes",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        DecreaseShapeRecognitionSensitivity,
        "Decrease Shape Pen Sensitivity",
        None,
        "Keep more strokes as ink",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        CycleBlurStyle,
        "Cycle Blur Style",
        None,
        "Blur, pixelate, secure, black out",
        Tools,
        true,
        true,
        true
    ),
    meta!(
        CycleArrowStyle,
        "Cycle Arrow Style",
        None,
        "Standard, pointy, curved, double",
        Tools,
        true,
        true,
        true
    ),
];
