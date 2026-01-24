use wayscriber::input::Tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOption {
    Select,
    Pen,
    Line,
    Rect,
    Ellipse,
    Arrow,
    Marker,
    StepMarker,
    Highlight,
    Eraser,
}

impl ToolOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::Select,
            Self::Pen,
            Self::Line,
            Self::Rect,
            Self::Ellipse,
            Self::Arrow,
            Self::Marker,
            Self::StepMarker,
            Self::Highlight,
            Self::Eraser,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Pen => "Pen",
            Self::Line => "Line",
            Self::Rect => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Arrow => "Arrow",
            Self::Marker => "Marker",
            Self::StepMarker => "Step",
            Self::Highlight => "Highlight",
            Self::Eraser => "Eraser",
        }
    }

    pub fn to_tool(self) -> Tool {
        match self {
            Self::Select => Tool::Select,
            Self::Pen => Tool::Pen,
            Self::Line => Tool::Line,
            Self::Rect => Tool::Rect,
            Self::Ellipse => Tool::Ellipse,
            Self::Arrow => Tool::Arrow,
            Self::Marker => Tool::Marker,
            Self::StepMarker => Tool::StepMarker,
            Self::Highlight => Tool::Highlight,
            Self::Eraser => Tool::Eraser,
        }
    }

    pub fn from_tool(tool: Tool) -> Self {
        match tool {
            Tool::Select => Self::Select,
            Tool::Pen => Self::Pen,
            Tool::Line => Self::Line,
            Tool::Rect => Self::Rect,
            Tool::Ellipse => Self::Ellipse,
            Tool::Arrow => Self::Arrow,
            Tool::Marker => Self::Marker,
            Tool::StepMarker => Self::StepMarker,
            Tool::Highlight => Self::Highlight,
            Tool::Eraser => Self::Eraser,
        }
    }
}

impl std::fmt::Display for ToolOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
