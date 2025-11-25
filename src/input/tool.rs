//! Drawing tool selection.

/// Drawing tool selection.
///
/// The active tool determines what shape is created when the user drags the mouse.
/// Tools are selected by holding modifier keys (Shift, Ctrl, Tab) while dragging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Select/cursor tool - interact with UI without drawing
    Select,
    /// Freehand drawing - follows mouse path (default, no modifiers)
    Pen,
    /// Straight line - between start and end points (Shift)
    Line,
    /// Rectangle outline - from corner to corner (Ctrl)
    Rect,
    /// Ellipse/circle outline - from center outward (Tab)
    Ellipse,
    /// Arrow with directional head (Ctrl+Shift)
    Arrow,
    /// Semi-transparent marker stroke for highlighting text
    Marker,
    /// Highlight-only tool (no drawing, emits click highlight)
    Highlight,
    // Note: Text mode uses DrawingState::TextInput instead of Tool::Text
}
