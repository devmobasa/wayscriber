use crate::config::Action;
use crate::draw::Color;
use crate::draw::shape::PolygonTemplate;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tool {
    Select,
    Pen,
    Line,
    Rect,
    Ellipse,
    Triangle,
    Parallelogram,
    Rhombus,
    RegularPolygon,
    FreeformPolygon,
    Arrow,
    Blur,
    Marker,
    Highlight,
    StepMarker,
    Eraser,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DragTool {
    Default,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolSettingsSlot {
    Pen,
    Line,
    Rect,
    Ellipse,
    Arrow,
    Blur,
    Marker,
    StepMarker,
}

impl ToolSettingsSlot {
    pub(crate) const ALL: [Self; 8] = [
        Self::Pen,
        Self::Line,
        Self::Rect,
        Self::Ellipse,
        Self::Arrow,
        Self::Blur,
        Self::Marker,
        Self::StepMarker,
    ];

    pub(crate) fn representative_tool(self) -> Tool {
        match self {
            Self::Pen => Tool::Pen,
            Self::Line => Tool::Line,
            Self::Rect => Tool::Rect,
            Self::Ellipse => Tool::Ellipse,
            Self::Arrow => Tool::Arrow,
            Self::Blur => Tool::Blur,
            Self::Marker => Tool::Marker,
            Self::StepMarker => Tool::StepMarker,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolSizeSource {
    DrawingThickness,
    EraserSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolProfile {
    pub(crate) settings_slot: ToolSettingsSlot,
    pub(crate) size_source: ToolSizeSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolDrawingBehavior {
    None,
    Polygon(PolygonTemplate),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolDescriptor {
    tool: Tool,
    action: Option<Action>,
    drag_tool: Option<DragTool>,
    profile: ToolProfile,
    drawing: ToolDrawingBehavior,
}

const fn profile(settings_slot: ToolSettingsSlot, size_source: ToolSizeSource) -> ToolProfile {
    ToolProfile {
        settings_slot,
        size_source,
    }
}

const fn descriptor(
    tool: Tool,
    action: Option<Action>,
    drag_tool: Option<DragTool>,
    settings_slot: ToolSettingsSlot,
    size_source: ToolSizeSource,
    drawing: ToolDrawingBehavior,
) -> ToolDescriptor {
    ToolDescriptor {
        tool,
        action,
        drag_tool,
        profile: profile(settings_slot, size_source),
        drawing,
    }
}

const DESCRIPTORS: [ToolDescriptor; 16] = [
    descriptor(
        Tool::Select,
        Some(Action::SelectSelectionTool),
        Some(DragTool::Select),
        ToolSettingsSlot::Pen,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::Pen,
        Some(Action::SelectPenTool),
        Some(DragTool::Pen),
        ToolSettingsSlot::Pen,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::Line,
        Some(Action::SelectLineTool),
        Some(DragTool::Line),
        ToolSettingsSlot::Line,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::Rect,
        Some(Action::SelectRectTool),
        Some(DragTool::Rect),
        ToolSettingsSlot::Rect,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::Ellipse,
        Some(Action::SelectEllipseTool),
        Some(DragTool::Ellipse),
        ToolSettingsSlot::Ellipse,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::Triangle,
        Some(Action::SelectTriangleTool),
        Some(DragTool::Triangle),
        ToolSettingsSlot::Rect,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::Polygon(PolygonTemplate::Triangle),
    ),
    descriptor(
        Tool::Parallelogram,
        Some(Action::SelectParallelogramTool),
        Some(DragTool::Parallelogram),
        ToolSettingsSlot::Rect,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::Polygon(PolygonTemplate::Parallelogram),
    ),
    descriptor(
        Tool::Rhombus,
        Some(Action::SelectRhombusTool),
        Some(DragTool::Rhombus),
        ToolSettingsSlot::Rect,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::Polygon(PolygonTemplate::Rhombus),
    ),
    descriptor(
        Tool::RegularPolygon,
        Some(Action::SelectRegularPolygonTool),
        Some(DragTool::RegularPolygon),
        ToolSettingsSlot::Rect,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::Polygon(PolygonTemplate::Regular),
    ),
    descriptor(
        Tool::FreeformPolygon,
        Some(Action::SelectFreeformPolygonTool),
        None,
        ToolSettingsSlot::Rect,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::Arrow,
        Some(Action::SelectArrowTool),
        Some(DragTool::Arrow),
        ToolSettingsSlot::Arrow,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::Blur,
        Some(Action::SelectBlurTool),
        Some(DragTool::Blur),
        ToolSettingsSlot::Blur,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::Marker,
        Some(Action::SelectMarkerTool),
        Some(DragTool::Marker),
        ToolSettingsSlot::Marker,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::Highlight,
        Some(Action::SelectHighlightTool),
        Some(DragTool::Highlight),
        ToolSettingsSlot::Pen,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::StepMarker,
        Some(Action::SelectStepMarkerTool),
        Some(DragTool::StepMarker),
        ToolSettingsSlot::StepMarker,
        ToolSizeSource::DrawingThickness,
        ToolDrawingBehavior::None,
    ),
    descriptor(
        Tool::Eraser,
        Some(Action::SelectEraserTool),
        Some(DragTool::Eraser),
        ToolSettingsSlot::Pen,
        ToolSizeSource::EraserSize,
        ToolDrawingBehavior::None,
    ),
];

impl Tool {
    pub(crate) const ALL: [Self; 16] = [
        Self::Select,
        Self::Pen,
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
        Self::Marker,
        Self::Highlight,
        Self::StepMarker,
        Self::Eraser,
    ];

    pub(crate) fn descriptor(self) -> &'static ToolDescriptor {
        &DESCRIPTORS[self as usize]
    }

    pub(crate) fn profile(self) -> ToolProfile {
        self.descriptor().profile
    }

    pub(crate) fn action(self) -> Option<Action> {
        self.descriptor().action
    }

    pub(crate) fn drag_tool(self) -> Option<DragTool> {
        self.descriptor().drag_tool
    }

    pub(crate) fn from_select_action(action: Action) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|tool| tool.action() == Some(action))
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
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToolDrawingSettings {
    pub color: Color,
    pub thickness: f64,
}

impl ToolDrawingSettings {
    pub fn new(color: Color, thickness: f64) -> Self {
        Self { color, thickness }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerToolDrawingSettings {
    pub pen: ToolDrawingSettings,
    pub line: ToolDrawingSettings,
    pub rect: ToolDrawingSettings,
    pub ellipse: ToolDrawingSettings,
    pub arrow: ToolDrawingSettings,
    pub blur: ToolDrawingSettings,
    pub marker: ToolDrawingSettings,
    pub step_marker: ToolDrawingSettings,
}

impl PerToolDrawingSettings {
    pub fn new(color: Color, thickness: f64) -> Self {
        let settings = ToolDrawingSettings::new(color, thickness);
        Self {
            pen: settings,
            line: settings,
            rect: settings,
            ellipse: settings,
            arrow: settings,
            blur: settings,
            marker: settings,
            step_marker: settings,
        }
    }

    pub fn settings_tool(tool: Tool) -> Tool {
        tool.settings_tool()
    }

    pub fn get(&self, tool: Tool) -> &ToolDrawingSettings {
        self.get_slot(tool.settings_slot())
    }

    pub fn get_mut(&mut self, tool: Tool) -> &mut ToolDrawingSettings {
        self.get_slot_mut(tool.settings_slot())
    }

    fn get_slot(&self, slot: ToolSettingsSlot) -> &ToolDrawingSettings {
        match slot {
            ToolSettingsSlot::Pen => &self.pen,
            ToolSettingsSlot::Line => &self.line,
            ToolSettingsSlot::Rect => &self.rect,
            ToolSettingsSlot::Ellipse => &self.ellipse,
            ToolSettingsSlot::Arrow => &self.arrow,
            ToolSettingsSlot::Blur => &self.blur,
            ToolSettingsSlot::Marker => &self.marker,
            ToolSettingsSlot::StepMarker => &self.step_marker,
        }
    }

    fn get_slot_mut(&mut self, slot: ToolSettingsSlot) -> &mut ToolDrawingSettings {
        match slot {
            ToolSettingsSlot::Pen => &mut self.pen,
            ToolSettingsSlot::Line => &mut self.line,
            ToolSettingsSlot::Rect => &mut self.rect,
            ToolSettingsSlot::Ellipse => &mut self.ellipse,
            ToolSettingsSlot::Arrow => &mut self.arrow,
            ToolSettingsSlot::Blur => &mut self.blur,
            ToolSettingsSlot::Marker => &mut self.marker,
            ToolSettingsSlot::StepMarker => &mut self.step_marker,
        }
    }

    pub fn clamp_thicknesses(mut self, min: f64, max: f64) -> Self {
        for slot in ToolSettingsSlot::ALL {
            self.get_slot_mut(slot).thickness = self.get_slot(slot).thickness.clamp(min, max);
        }
        self
    }
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum EraserMode {
    #[default]
    Brush,
    Stroke,
}
