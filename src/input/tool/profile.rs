use super::Tool;

/// The stored color/thickness slot used by a tool.
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

/// Where a tool's visible size value is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolSizeSource {
    DrawingThickness,
    EraserSize,
}

/// Side-toolbar control family exposed by a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolControlGroup {
    None,
    Stroke,
    Marker,
    Eraser,
    Shape,
    Arrow,
    StepMarker,
}

/// Catalog entry describing the settings and controls for one drawing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolProfile {
    pub(crate) settings_slot: ToolSettingsSlot,
    pub(crate) size_source: ToolSizeSource,
    pub(crate) control_group: ToolControlGroup,
    pub(crate) needs_color: bool,
    pub(crate) thickness_label: &'static str,
}

impl ToolProfile {
    pub(crate) fn needs_thickness_control(self) -> bool {
        !matches!(self.control_group, ToolControlGroup::None)
    }

    pub(crate) fn show_fill_toggle(self) -> bool {
        matches!(self.control_group, ToolControlGroup::Shape)
    }

    pub(crate) fn show_arrow_labels(self) -> bool {
        matches!(self.control_group, ToolControlGroup::Arrow)
    }

    pub(crate) fn show_step_counter(self) -> bool {
        matches!(self.control_group, ToolControlGroup::StepMarker)
    }

    pub(crate) fn show_eraser_mode(self) -> bool {
        matches!(self.control_group, ToolControlGroup::Eraser)
    }

    pub(crate) fn show_marker_opacity(self) -> bool {
        matches!(self.control_group, ToolControlGroup::Marker)
    }
}
