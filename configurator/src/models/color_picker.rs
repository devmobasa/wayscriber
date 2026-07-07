#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorPickerId {
    DrawingColor,
    QuickColor(usize),
    BoardBackground(usize),
    BoardPen(usize),
    RenderProfileMappingFrom(usize, usize),
    RenderProfileMappingTo(usize, usize),
    StatusBarBg,
    StatusBarText,
    HighlightFill,
    HighlightOutline,
    HelpBg,
    HelpBorder,
    HelpText,
    ExportPdfLabelText,
    ExportPdfLabelBackground,
}

#[derive(Debug, Clone, Copy)]
pub struct ColorPickerValue {
    pub rgb: [f64; 3],
    pub alpha: Option<f64>,
}
