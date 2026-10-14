use crate::domain::Action;
use crate::draw::shape::PolygonTemplate;
use crate::i18n::tr;

use super::{Tool, ToolControlGroup, ToolProfile, ToolSettingsSlot, ToolSizeSource};

/// Static catalog facts for one built-in drawing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolDescriptor {
    pub(crate) tool: Tool,
    pub(crate) action: Option<Action>,
    pub(crate) profile: ToolProfile,
    pub(crate) press: ToolPressBehavior,
    pub(crate) motion: ToolMotionBehavior,
    pub(crate) drawing: ToolDrawingBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolPressBehavior {
    Selection,
    HighlightNoop,
    StartFreeformPolygon,
    StartDrawing { request_blur_capture: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolMotionBehavior {
    NoPathAccumulation,
    AccumulatePath { size_source: ToolMotionSizeSource },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolMotionSizeSource {
    ToolSize,
    EraserSize,
    /// The `[laser]` core width, which no tool slot stores.
    LaserWidth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolDrawingBehavior {
    None,
    Path {
        kind: ToolPathKind,
        pressure: ToolPressureBehavior,
    },
    LiveShape,
    Line,
    Rect,
    Ellipse,
    Polygon(PolygonTemplate),
    Arrow,
    BlurRect,
    Spotlight,
    StepMarker,
    Eraser,
    /// Fading pointer ink that is never committed to the frame.
    Laser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolPathKind {
    Freehand,
    Marker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolPressureBehavior {
    None,
    OptionalPressureStroke,
}

const fn profile(
    settings_slot: ToolSettingsSlot,
    size_source: ToolSizeSource,
    control_group: ToolControlGroup,
    needs_color: bool,
    thickness_label: &'static str,
) -> ToolProfile {
    ToolProfile {
        settings_slot,
        size_source,
        control_group,
        needs_color,
        thickness_label,
    }
}

const DESCRIPTORS: [ToolDescriptor; 19] = [
    ToolDescriptor {
        tool: Tool::Select,
        action: Some(Action::SelectSelectionTool),
        profile: profile(
            ToolSettingsSlot::Pen,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::None,
            false,
            "",
        ),
        press: ToolPressBehavior::Selection,
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::None,
    },
    ToolDescriptor {
        tool: Tool::Pen,
        action: Some(Action::SelectPenTool),
        profile: profile(
            ToolSettingsSlot::Pen,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Stroke,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::AccumulatePath {
            size_source: ToolMotionSizeSource::ToolSize,
        },
        drawing: ToolDrawingBehavior::Path {
            kind: ToolPathKind::Freehand,
            pressure: ToolPressureBehavior::OptionalPressureStroke,
        },
    },
    ToolDescriptor {
        tool: Tool::LiveShape,
        action: Some(Action::SelectLiveShapeTool),
        profile: profile(
            ToolSettingsSlot::Pen,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Shape,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::AccumulatePath {
            size_source: ToolMotionSizeSource::ToolSize,
        },
        drawing: ToolDrawingBehavior::LiveShape,
    },
    ToolDescriptor {
        tool: Tool::Line,
        action: Some(Action::SelectLineTool),
        profile: profile(
            ToolSettingsSlot::Line,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Stroke,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::Line,
    },
    ToolDescriptor {
        tool: Tool::Rect,
        action: Some(Action::SelectRectTool),
        profile: profile(
            ToolSettingsSlot::Rect,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Shape,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::Rect,
    },
    ToolDescriptor {
        tool: Tool::Ellipse,
        action: Some(Action::SelectEllipseTool),
        profile: profile(
            ToolSettingsSlot::Ellipse,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Shape,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::Ellipse,
    },
    ToolDescriptor {
        tool: Tool::Triangle,
        action: Some(Action::SelectTriangleTool),
        profile: profile(
            ToolSettingsSlot::Rect,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Shape,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::Polygon(PolygonTemplate::Triangle),
    },
    ToolDescriptor {
        tool: Tool::Parallelogram,
        action: Some(Action::SelectParallelogramTool),
        profile: profile(
            ToolSettingsSlot::Rect,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Shape,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::Polygon(PolygonTemplate::Parallelogram),
    },
    ToolDescriptor {
        tool: Tool::Rhombus,
        action: Some(Action::SelectRhombusTool),
        profile: profile(
            ToolSettingsSlot::Rect,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Shape,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::Polygon(PolygonTemplate::Rhombus),
    },
    ToolDescriptor {
        tool: Tool::RegularPolygon,
        action: Some(Action::SelectRegularPolygonTool),
        profile: profile(
            ToolSettingsSlot::Rect,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Shape,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::Polygon(PolygonTemplate::Regular),
    },
    ToolDescriptor {
        tool: Tool::FreeformPolygon,
        action: Some(Action::SelectFreeformPolygonTool),
        profile: profile(
            ToolSettingsSlot::Rect,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Shape,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartFreeformPolygon,
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::None,
    },
    ToolDescriptor {
        tool: Tool::Arrow,
        action: Some(Action::SelectArrowTool),
        profile: profile(
            ToolSettingsSlot::Arrow,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Arrow,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::Arrow,
    },
    ToolDescriptor {
        tool: Tool::Blur,
        action: Some(Action::SelectBlurTool),
        profile: profile(
            ToolSettingsSlot::Blur,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Stroke,
            false,
            "Blur",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: true,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::BlurRect,
    },
    ToolDescriptor {
        tool: Tool::Spotlight,
        action: Some(Action::SelectSpotlightTool),
        profile: profile(
            // A spotlight's geometry comes entirely from the drag and it has no
            // stroke, so it borrows the blur slot rather than adding a settings
            // slot with nothing in it. Its dedicated controls expose only
            // magnification.
            ToolSettingsSlot::Blur,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Spotlight,
            false,
            "Spotlight",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::Spotlight,
    },
    ToolDescriptor {
        tool: Tool::Marker,
        action: Some(Action::SelectMarkerTool),
        profile: profile(
            ToolSettingsSlot::Marker,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::Marker,
            true,
            "Thickness",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::AccumulatePath {
            size_source: ToolMotionSizeSource::ToolSize,
        },
        drawing: ToolDrawingBehavior::Path {
            kind: ToolPathKind::Marker,
            pressure: ToolPressureBehavior::None,
        },
    },
    ToolDescriptor {
        tool: Tool::Highlight,
        action: Some(Action::SelectHighlightTool),
        profile: profile(
            ToolSettingsSlot::Pen,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::None,
            false,
            "",
        ),
        press: ToolPressBehavior::HighlightNoop,
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::None,
    },
    ToolDescriptor {
        tool: Tool::Laser,
        action: Some(Action::SelectLaserTool),
        profile: profile(
            // Laser ink takes its color and width from `[laser]`, not from a
            // tool slot, so it has no style controls. The pen slot is named
            // only because every descriptor needs one; nothing reads it.
            ToolSettingsSlot::Pen,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::None,
            false,
            "",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::AccumulatePath {
            size_source: ToolMotionSizeSource::LaserWidth,
        },
        drawing: ToolDrawingBehavior::Laser,
    },
    ToolDescriptor {
        tool: Tool::StepMarker,
        action: Some(Action::SelectStepMarkerTool),
        profile: profile(
            ToolSettingsSlot::StepMarker,
            ToolSizeSource::DrawingThickness,
            ToolControlGroup::StepMarker,
            true,
            "Size",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::NoPathAccumulation,
        drawing: ToolDrawingBehavior::StepMarker,
    },
    ToolDescriptor {
        tool: Tool::Eraser,
        action: Some(Action::SelectEraserTool),
        profile: profile(
            ToolSettingsSlot::Pen,
            ToolSizeSource::EraserSize,
            ToolControlGroup::Eraser,
            false,
            "Eraser Size",
        ),
        press: ToolPressBehavior::StartDrawing {
            request_blur_capture: false,
        },
        motion: ToolMotionBehavior::AccumulatePath {
            size_source: ToolMotionSizeSource::EraserSize,
        },
        drawing: ToolDrawingBehavior::Eraser,
    },
];

impl Tool {
    pub(crate) const ALL: [Self; 19] = [
        Self::Select,
        Self::Pen,
        Self::LiveShape,
        Self::Line,
        Self::Rect,
        Self::Ellipse,
        Self::Triangle,
        Self::Parallelogram,
        Self::Rhombus,
        Self::RegularPolygon,
        Self::FreeformPolygon,
        Self::Arrow,
        Self::Blur,
        Self::Spotlight,
        Self::Marker,
        Self::Highlight,
        Self::Laser,
        Self::StepMarker,
        Self::Eraser,
    ];

    pub(crate) fn descriptor(self) -> &'static ToolDescriptor {
        match self {
            Self::Select => &DESCRIPTORS[0],
            Self::Pen => &DESCRIPTORS[1],
            Self::LiveShape => &DESCRIPTORS[2],
            Self::Line => &DESCRIPTORS[3],
            Self::Rect => &DESCRIPTORS[4],
            Self::Ellipse => &DESCRIPTORS[5],
            Self::Triangle => &DESCRIPTORS[6],
            Self::Parallelogram => &DESCRIPTORS[7],
            Self::Rhombus => &DESCRIPTORS[8],
            Self::RegularPolygon => &DESCRIPTORS[9],
            Self::FreeformPolygon => &DESCRIPTORS[10],
            Self::Arrow => &DESCRIPTORS[11],
            Self::Blur => &DESCRIPTORS[12],
            Self::Spotlight => &DESCRIPTORS[13],
            Self::Marker => &DESCRIPTORS[14],
            Self::Highlight => &DESCRIPTORS[15],
            Self::Laser => &DESCRIPTORS[16],
            Self::StepMarker => &DESCRIPTORS[17],
            Self::Eraser => &DESCRIPTORS[18],
        }
    }

    pub(crate) fn profile(self) -> ToolProfile {
        self.descriptor().profile
    }

    pub(crate) fn action(self) -> Option<Action> {
        self.descriptor().action
    }

    pub(crate) fn from_select_action(action: Action) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|tool| tool.action() == Some(action))
    }

    pub(crate) fn short_label(self) -> &'static str {
        match self {
            Self::Select => tr("Select"),
            Self::Pen => tr("Pen"),
            Self::LiveShape => tr("Shape Pen"),
            Self::Line => tr("Line"),
            Self::Rect => tr("Rect"),
            Self::Ellipse => tr("Circle"),
            Self::Triangle => tr("Triangle"),
            Self::Parallelogram => tr("Para"),
            Self::Rhombus => tr("Rhombus"),
            Self::RegularPolygon => tr("Polygon"),
            Self::FreeformPolygon => tr("Freeform"),
            Self::Arrow => tr("Arrow"),
            Self::Blur => tr("Blur"),
            Self::Spotlight => tr("Spotlight"),
            Self::Marker => tr("Marker"),
            Self::Highlight => tr("Highlight"),
            Self::Laser => tr("Laser"),
            Self::StepMarker => tr("Steps"),
            Self::Eraser => tr("Eraser"),
        }
    }

    pub(crate) fn display_label(self) -> &'static str {
        match self {
            Self::Select => tr("Selection Tool"),
            Self::Pen => tr("Pen Tool"),
            Self::LiveShape => tr("Shape Pen Tool"),
            Self::Line => tr("Line Tool"),
            Self::Rect => tr("Rectangle Tool"),
            Self::Ellipse => tr("Ellipse Tool"),
            Self::Triangle => tr("Triangle Tool"),
            Self::Parallelogram => tr("Parallelogram Tool"),
            Self::Rhombus => tr("Rhombus Tool"),
            Self::RegularPolygon => tr("Regular Polygon Tool"),
            Self::FreeformPolygon => tr("Freeform Polygon Tool"),
            Self::Arrow => tr("Arrow Tool"),
            Self::Blur => tr("Blur Tool"),
            Self::Spotlight => tr("Spotlight Tool"),
            Self::Marker => tr("Marker Tool"),
            Self::Highlight => tr("Highlight Tool"),
            Self::Laser => tr("Laser Pointer Tool"),
            Self::StepMarker => tr("Step Marker Tool"),
            Self::Eraser => tr("Eraser Tool"),
        }
    }

    pub(crate) fn press_behavior(self) -> ToolPressBehavior {
        self.descriptor().press
    }

    pub(crate) fn motion_behavior(self) -> ToolMotionBehavior {
        self.descriptor().motion
    }

    pub(crate) fn drawing_behavior(self) -> ToolDrawingBehavior {
        self.descriptor().drawing
    }

    /// Whether `[drawing] pen_smoothing` changes what this tool commits.
    ///
    /// The setting is one number for the whole program, but the control for it
    /// is not: offering a smoothing stepper while the Line or Blur tool is up
    /// would be a control that does nothing to the shape about to be drawn.
    /// Accumulated paths — freehand and marker — are the ones smoothed on
    /// release.
    pub(crate) fn smooths_strokes(self) -> bool {
        matches!(
            self.drawing_behavior(),
            ToolDrawingBehavior::Path { .. } | ToolDrawingBehavior::LiveShape
        )
    }

    pub(crate) fn settings_slot(self) -> ToolSettingsSlot {
        self.profile().settings_slot
    }

    pub(crate) fn settings_tool(self) -> Tool {
        self.settings_slot().representative_tool()
    }

    pub(crate) fn uses_eraser_size(self) -> bool {
        matches!(self.profile().size_source, ToolSizeSource::EraserSize)
    }

    pub(crate) fn uses_drawing_thickness(self) -> bool {
        matches!(self.profile().size_source, ToolSizeSource::DrawingThickness)
    }

    pub(crate) fn uses_marker_opacity(self) -> bool {
        self.profile().show_marker_opacity()
    }

    #[cfg_attr(not(feature = "tablet-input"), allow(dead_code))]
    pub(crate) fn supports_pressure_thickness(self) -> bool {
        matches!(
            self.drawing_behavior(),
            ToolDrawingBehavior::Path {
                pressure: ToolPressureBehavior::OptionalPressureStroke,
                ..
            } | ToolDrawingBehavior::LiveShape
        )
    }
}
