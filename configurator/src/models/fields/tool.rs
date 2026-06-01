use wayscriber::config::ColorSpec;
use wayscriber::input::{DragBindableTool, DragTool, Tool};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragMouseButton {
    Left,
    Right,
    Middle,
}

impl DragMouseButton {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left button",
            Self::Right => "Right button",
            Self::Middle => "Middle button",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragToolField {
    Drag,
    ShiftDrag,
    CtrlDrag,
    CtrlShiftDrag,
    TabDrag,
}

impl DragToolField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Drag => "Drag",
            Self::ShiftDrag => "Shift+Drag",
            Self::CtrlDrag => "Ctrl+Drag",
            Self::CtrlShiftDrag => "Ctrl+Shift+Drag",
            Self::TabDrag => "Tab+Drag",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOption {
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
            Self::Triangle,
            Self::Parallelogram,
            Self::Rhombus,
            Self::RegularPolygon,
            Self::FreeformPolygon,
            Self::Arrow,
            Self::Blur,
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
            Self::Triangle => "Triangle",
            Self::Parallelogram => "Parallelogram",
            Self::Rhombus => "Rhombus",
            Self::RegularPolygon => "Regular polygon",
            Self::FreeformPolygon => "Freeform polygon",
            Self::Arrow => "Arrow",
            Self::Blur => "Blur",
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
            Self::Triangle => Tool::Triangle,
            Self::Parallelogram => Tool::Parallelogram,
            Self::Rhombus => Tool::Rhombus,
            Self::RegularPolygon => Tool::RegularPolygon,
            Self::FreeformPolygon => Tool::FreeformPolygon,
            Self::Arrow => Tool::Arrow,
            Self::Blur => Tool::Blur,
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
            Tool::Triangle => Self::Triangle,
            Tool::Parallelogram => Self::Parallelogram,
            Tool::Rhombus => Self::Rhombus,
            Tool::RegularPolygon => Self::RegularPolygon,
            Tool::FreeformPolygon => Self::FreeformPolygon,
            Tool::Arrow => Self::Arrow,
            Tool::Blur => Self::Blur,
            Tool::Marker => Self::Marker,
            Tool::StepMarker => Self::StepMarker,
            Tool::Highlight => Self::Highlight,
            Tool::Eraser => Self::Eraser,
        }
    }

    pub fn from_drag_bindable_tool(tool: DragBindableTool) -> Self {
        Self::from_tool(tool.to_tool())
    }
}

impl std::fmt::Display for ToolOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragToolOption {
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
    StepMarker,
    Highlight,
    Eraser,
}

impl DragToolOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::Default,
            Self::Select,
            Self::Pen,
            Self::Line,
            Self::Rect,
            Self::Ellipse,
            Self::Triangle,
            Self::Parallelogram,
            Self::Rhombus,
            Self::RegularPolygon,
            Self::Arrow,
            Self::Blur,
            Self::Marker,
            Self::StepMarker,
            Self::Highlight,
            Self::Eraser,
        ]
    }

    pub fn list_for_button(button: DragMouseButton) -> Vec<Self> {
        let mut options = Self::list();
        if button == DragMouseButton::Left {
            options.retain(|option| *option != Self::Default);
        }
        options
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Button default",
            Self::Select => "Select",
            Self::Pen => "Pen",
            Self::Line => "Line",
            Self::Rect => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Triangle => "Triangle",
            Self::Parallelogram => "Parallelogram",
            Self::Rhombus => "Rhombus",
            Self::RegularPolygon => "Regular polygon",
            Self::Arrow => "Arrow",
            Self::Blur => "Blur",
            Self::Marker => "Marker",
            Self::StepMarker => "Step",
            Self::Highlight => "Highlight",
            Self::Eraser => "Eraser",
        }
    }

    pub fn to_drag_tool(self) -> DragTool {
        match self {
            Self::Default => DragTool::Default,
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
            Self::StepMarker => DragTool::StepMarker,
            Self::Highlight => DragTool::Highlight,
            Self::Eraser => DragTool::Eraser,
        }
    }

    pub fn from_drag_tool(tool: DragTool) -> Self {
        match tool {
            DragTool::Default => Self::Default,
            DragTool::Select => Self::Select,
            DragTool::Pen => Self::Pen,
            DragTool::Line => Self::Line,
            DragTool::Rect => Self::Rect,
            DragTool::Ellipse => Self::Ellipse,
            DragTool::Triangle => Self::Triangle,
            DragTool::Parallelogram => Self::Parallelogram,
            DragTool::Rhombus => Self::Rhombus,
            DragTool::RegularPolygon => Self::RegularPolygon,
            DragTool::Arrow => Self::Arrow,
            DragTool::Blur => Self::Blur,
            DragTool::Marker => Self::Marker,
            DragTool::StepMarker => Self::StepMarker,
            DragTool::Highlight => Self::Highlight,
            DragTool::Eraser => Self::Eraser,
        }
    }

    pub fn to_tool_option(self) -> Option<ToolOption> {
        match self {
            Self::Default => None,
            Self::Select => Some(ToolOption::Select),
            Self::Pen => Some(ToolOption::Pen),
            Self::Line => Some(ToolOption::Line),
            Self::Rect => Some(ToolOption::Rect),
            Self::Ellipse => Some(ToolOption::Ellipse),
            Self::Triangle => Some(ToolOption::Triangle),
            Self::Parallelogram => Some(ToolOption::Parallelogram),
            Self::Rhombus => Some(ToolOption::Rhombus),
            Self::RegularPolygon => Some(ToolOption::RegularPolygon),
            Self::Arrow => Some(ToolOption::Arrow),
            Self::Blur => Some(ToolOption::Blur),
            Self::Marker => Some(ToolOption::Marker),
            Self::StepMarker => Some(ToolOption::StepMarker),
            Self::Highlight => Some(ToolOption::Highlight),
            Self::Eraser => Some(ToolOption::Eraser),
        }
    }
}

impl std::fmt::Display for DragToolOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragColorOption {
    Current,
    Red,
    Green,
    Blue,
    Yellow,
    Orange,
    Pink,
    White,
    Black,
    Custom,
}

impl DragColorOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::Current,
            Self::Red,
            Self::Green,
            Self::Blue,
            Self::Yellow,
            Self::Orange,
            Self::Pink,
            Self::White,
            Self::Black,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Current => "Current color",
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Yellow => "Yellow",
            Self::Orange => "Orange",
            Self::Pink => "Pink",
            Self::White => "White",
            Self::Black => "Black",
            Self::Custom => "Custom RGB",
        }
    }

    pub fn from_color(color: Option<&ColorSpec>) -> Self {
        match color {
            None => Self::Current,
            Some(ColorSpec::Rgb(_)) => Self::Custom,
            Some(ColorSpec::Name(name)) => match name.trim().to_lowercase().as_str() {
                "red" => Self::Red,
                "green" => Self::Green,
                "blue" => Self::Blue,
                "yellow" => Self::Yellow,
                "orange" => Self::Orange,
                "pink" => Self::Pink,
                "white" => Self::White,
                "black" => Self::Black,
                _ => Self::Custom,
            },
        }
    }

    pub fn to_color_spec(self, existing: Option<ColorSpec>) -> Option<ColorSpec> {
        let name = match self {
            Self::Current => return None,
            Self::Custom => return existing,
            Self::Red => "red",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Pink => "pink",
            Self::White => "white",
            Self::Black => "black",
        };
        Some(ColorSpec::Name(name.to_string()))
    }
}

impl std::fmt::Display for DragColorOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
