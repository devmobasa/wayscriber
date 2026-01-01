use crate::draw::Color;

/// Background replay context for eraser strokes.
pub struct EraserReplayContext<'a> {
    /// Optional pattern representing the current background (e.g., frozen image) in device space.
    pub pattern: Option<&'a cairo::Pattern>,
    /// Solid background color (board modes) when no pattern is available.
    pub bg_color: Option<Color>,
}
