use crate::config::Action;

use super::{DragTool, Tool, ToolControlGroup, ToolProfile, ToolSettingsSlot, ToolSizeSource};

/// Static catalog facts for one built-in drawing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolDescriptor {
    pub(crate) tool: Tool,
    pub(crate) short_label: &'static str,
    pub(crate) display_label: &'static str,
    pub(crate) action: Option<Action>,
    pub(crate) drag_tool: DragTool,
    pub(crate) profile: ToolProfile,
    pub(crate) press: ToolPressBehavior,
    pub(crate) motion: ToolMotionBehavior,
    pub(crate) drawing: ToolDrawingBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolPressBehavior {
    Selection,
    HighlightNoop,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolDrawingBehavior {
    None,
    Path {
        kind: ToolPathKind,
        pressure: ToolPressureBehavior,
    },
    Line,
    Rect,
    Ellipse,
    Arrow,
    BlurRect,
    StepMarker,
    Eraser,
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

const DESCRIPTORS: [ToolDescriptor; 11] = [
    ToolDescriptor {
        tool: Tool::Select,
        short_label: "Select",
        display_label: "Selection Tool",
        action: Some(Action::SelectSelectionTool),
        drag_tool: DragTool::Select,
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
        short_label: "Pen",
        display_label: "Pen Tool",
        action: Some(Action::SelectPenTool),
        drag_tool: DragTool::Pen,
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
        tool: Tool::Line,
        short_label: "Line",
        display_label: "Line Tool",
        action: Some(Action::SelectLineTool),
        drag_tool: DragTool::Line,
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
        short_label: "Rect",
        display_label: "Rectangle Tool",
        action: Some(Action::SelectRectTool),
        drag_tool: DragTool::Rect,
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
        short_label: "Circle",
        display_label: "Ellipse Tool",
        action: Some(Action::SelectEllipseTool),
        drag_tool: DragTool::Ellipse,
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
        tool: Tool::Arrow,
        short_label: "Arrow",
        display_label: "Arrow Tool",
        action: Some(Action::SelectArrowTool),
        drag_tool: DragTool::Arrow,
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
        short_label: "Blur",
        display_label: "Blur Tool",
        action: Some(Action::SelectBlurTool),
        drag_tool: DragTool::Blur,
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
        tool: Tool::Marker,
        short_label: "Marker",
        display_label: "Marker Tool",
        action: Some(Action::SelectMarkerTool),
        drag_tool: DragTool::Marker,
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
        short_label: "Highlight",
        display_label: "Highlight Tool",
        action: Some(Action::SelectHighlightTool),
        drag_tool: DragTool::Highlight,
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
        tool: Tool::StepMarker,
        short_label: "Steps",
        display_label: "Step Marker Tool",
        action: Some(Action::SelectStepMarkerTool),
        drag_tool: DragTool::StepMarker,
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
        short_label: "Eraser",
        display_label: "Eraser Tool",
        action: Some(Action::SelectEraserTool),
        drag_tool: DragTool::Eraser,
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
    pub(crate) const ALL: [Self; 11] = [
        Self::Select,
        Self::Pen,
        Self::Line,
        Self::Rect,
        Self::Ellipse,
        Self::Arrow,
        Self::Blur,
        Self::Marker,
        Self::Highlight,
        Self::StepMarker,
        Self::Eraser,
    ];

    pub(crate) fn descriptor(self) -> &'static ToolDescriptor {
        match self {
            Self::Select => &DESCRIPTORS[0],
            Self::Pen => &DESCRIPTORS[1],
            Self::Line => &DESCRIPTORS[2],
            Self::Rect => &DESCRIPTORS[3],
            Self::Ellipse => &DESCRIPTORS[4],
            Self::Arrow => &DESCRIPTORS[5],
            Self::Blur => &DESCRIPTORS[6],
            Self::Marker => &DESCRIPTORS[7],
            Self::Highlight => &DESCRIPTORS[8],
            Self::StepMarker => &DESCRIPTORS[9],
            Self::Eraser => &DESCRIPTORS[10],
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
        self.descriptor().short_label
    }

    pub(crate) fn display_label(self) -> &'static str {
        self.descriptor().display_label
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
}
