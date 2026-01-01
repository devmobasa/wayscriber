use crate::util::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPropertyKind {
    Color,
    Thickness,
    Fill,
    FontSize,
    ArrowHead,
    ArrowLength,
    ArrowAngle,
    TextBackground,
}

#[derive(Debug, Clone)]
pub struct SelectionPropertyEntry {
    pub label: String,
    pub value: String,
    pub kind: SelectionPropertyKind,
    pub disabled: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PropertiesPanelLayout {
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub title_baseline_y: f64,
    pub info_start_y: f64,
    pub entry_start_y: f64,
    pub entry_row_height: f64,
    pub padding_x: f64,
    pub label_x: f64,
    pub value_x: f64,
}

#[derive(Debug, Clone)]
pub struct ShapePropertiesPanel {
    pub title: String,
    pub anchor: (f64, f64),
    pub anchor_rect: Option<Rect>,
    pub lines: Vec<String>,
    pub entries: Vec<SelectionPropertyEntry>,
    pub hover_index: Option<usize>,
    pub keyboard_focus: Option<usize>,
    pub multiple_selection: bool,
}
