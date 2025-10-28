//! Shape definitions for screen annotations.

use super::color::Color;
use super::font::FontDescriptor;
use serde::{Deserialize, Serialize};

/// Represents a drawable shape or annotation on screen.
///
/// Each variant represents a different drawing tool/primitive with its specific parameters.
/// All shapes store their own color and size information for independent rendering.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Shape {
    /// Freehand drawing - polyline connecting mouse drag points
    Freehand {
        /// Sequence of (x, y) coordinates traced by the mouse
        points: Vec<(i32, i32)>,
        /// Stroke color
        color: Color,
        /// Line thickness in pixels
        thick: f64,
    },
    /// Straight line between two points (drawn with Shift modifier)
    Line {
        /// Starting X coordinate
        x1: i32,
        /// Starting Y coordinate
        y1: i32,
        /// Ending X coordinate
        x2: i32,
        /// Ending Y coordinate
        y2: i32,
        /// Line color
        color: Color,
        /// Line thickness in pixels
        thick: f64,
    },
    /// Rectangle outline (drawn with Ctrl modifier)
    Rect {
        /// Top-left X coordinate
        x: i32,
        /// Top-left Y coordinate
        y: i32,
        /// Width in pixels
        w: i32,
        /// Height in pixels
        h: i32,
        /// Border color
        color: Color,
        /// Border thickness in pixels
        thick: f64,
    },
    /// Ellipse/circle outline (drawn with Tab modifier)
    Ellipse {
        /// Center X coordinate
        cx: i32,
        /// Center Y coordinate
        cy: i32,
        /// Horizontal radius
        rx: i32,
        /// Vertical radius
        ry: i32,
        /// Border color
        color: Color,
        /// Border thickness in pixels
        thick: f64,
    },
    /// Arrow with directional head (drawn with Ctrl+Shift modifiers)
    Arrow {
        /// Starting X coordinate (arrowhead location)
        x1: i32,
        /// Starting Y coordinate (arrowhead location)
        y1: i32,
        /// Ending X coordinate (arrow tail)
        x2: i32,
        /// Ending Y coordinate (arrow tail)
        y2: i32,
        /// Arrow color
        color: Color,
        /// Line thickness in pixels
        thick: f64,
        /// Arrowhead length in pixels
        arrow_length: f64,
        /// Arrowhead angle in degrees
        arrow_angle: f64,
    },
    /// Text annotation (activated with 'T' key)
    Text {
        /// Baseline X coordinate
        x: i32,
        /// Baseline Y coordinate
        y: i32,
        /// Text content to display
        text: String,
        /// Text color
        color: Color,
        /// Font size in points
        size: f64,
        /// Font descriptor (family, weight, style)
        font_descriptor: FontDescriptor,
        /// Whether to draw background box behind text
        background_enabled: bool,
    },
}
