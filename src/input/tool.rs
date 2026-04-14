//! Drawing tool selection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Drawing tool selection.
///
/// The active tool determines what shape is created when the user drags the mouse.
/// Drag modifier mappings are configurable via `[drawing]` drag-tool fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
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
    /// Privacy blur rectangle over the captured background
    Blur,
    /// Semi-transparent marker stroke for highlighting text
    Marker,
    /// Highlight-only tool (no drawing, emits click highlight)
    Highlight,
    /// Numbered step marker tool (places auto-incrementing bubbles)
    StepMarker,
    /// Eraser brush that removes content within its stroke
    Eraser,
    // Note: Text mode uses DrawingState::TextInput instead of Tool::Text
}

/// Eraser behavior mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub enum EraserMode {
    /// Brush-style eraser that clears pixels along its stroke.
    #[default]
    Brush,
    /// Stroke eraser that deletes any shape it touches.
    Stroke,
}
