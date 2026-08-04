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

impl ColorPickerId {
    /// Whether this picker edits a four-component color rather than RGB.
    pub(crate) fn uses_alpha(self) -> bool {
        matches!(
            self,
            Self::StatusBarBg
                | Self::StatusBarText
                | Self::HighlightFill
                | Self::HighlightOutline
                | Self::HelpBg
                | Self::HelpBorder
                | Self::HelpText
                | Self::ExportPdfLabelText
                | Self::ExportPdfLabelBackground
        )
    }
}
