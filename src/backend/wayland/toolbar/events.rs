use crate::draw::Color;

/// Kinds of hit regions and their drag semantics.
#[derive(Clone, Debug, PartialEq)]
pub enum HitKind {
    Click,
    DragSetThickness { min: f64, max: f64 },
    DragSetMarkerOpacity { min: f64, max: f64 },
    DragSetFontSize,
    PickColor { x: f64, y: f64, w: f64, h: f64 },
    DragUndoDelay,
    DragRedoDelay,
    DragCustomUndoDelay,
    DragCustomRedoDelay,
    DragMoveTop,
    DragMoveSide,
}

/// Cursor hint for toolbar regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarCursorHint {
    /// Default arrow cursor.
    Default,
    /// Pointer/hand cursor for clickable buttons.
    Pointer,
    /// Grab cursor for sliders and drag handles.
    Grab,
    /// Crosshair for color pickers.
    Crosshair,
}

impl HitKind {
    /// Get the appropriate cursor hint for this hit kind.
    pub fn cursor_hint(&self) -> ToolbarCursorHint {
        match self {
            HitKind::Click => ToolbarCursorHint::Pointer,
            HitKind::DragSetThickness { .. }
            | HitKind::DragSetMarkerOpacity { .. }
            | HitKind::DragSetFontSize
            | HitKind::DragUndoDelay
            | HitKind::DragRedoDelay
            | HitKind::DragCustomUndoDelay
            | HitKind::DragCustomRedoDelay
            | HitKind::DragMoveTop
            | HitKind::DragMoveSide => ToolbarCursorHint::Grab,
            HitKind::PickColor { .. } => ToolbarCursorHint::Crosshair,
        }
    }
}

/// Convert normalized drag position [0,1] to a delay in seconds.
pub fn delay_secs_from_t(t: f64) -> f64 {
    const MIN_DELAY_S: f64 = 0.05;
    const MAX_DELAY_S: f64 = 5.0;
    MIN_DELAY_S + t.clamp(0.0, 1.0) * (MAX_DELAY_S - MIN_DELAY_S)
}

/// Convert a delay in ms to normalized [0,1] position for sliders.
pub fn delay_t_from_ms(delay_ms: u64) -> f64 {
    const MIN_DELAY_S: f64 = 0.05;
    const MAX_DELAY_S: f64 = 5.0;
    let delay_s = (delay_ms as f64 / 1000.0).clamp(MIN_DELAY_S, MAX_DELAY_S);
    (delay_s - MIN_DELAY_S) / (MAX_DELAY_S - MIN_DELAY_S)
}

/// Convert HSV to RGB for color picker math.
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Color {
    let h = (h - h.floor()).clamp(0.0, 1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color { r, g, b, a: 1.0 }
}
