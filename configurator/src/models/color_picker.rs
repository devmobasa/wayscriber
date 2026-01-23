#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorPickerId {
    BoardBackground(usize),
    BoardPen(usize),
    StatusBarBg,
    StatusBarText,
    HighlightFill,
    HighlightOutline,
    HelpBg,
    HelpBorder,
    HelpText,
}

#[derive(Debug, Clone, Copy)]
pub struct ColorPickerValue {
    pub rgb: [f64; 3],
    pub alpha: Option<f64>,
}
