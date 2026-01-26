use super::bounds::{
    bounding_box_for_arrow, bounding_box_for_ellipse, bounding_box_for_eraser,
    bounding_box_for_line, bounding_box_for_points, bounding_box_for_rect,
};
use super::step_marker::step_marker_bounds;
use super::text::{bounding_box_for_sticky_note, bounding_box_for_text};
use crate::draw::color::Color;
use crate::draw::font::FontDescriptor;
use crate::util::Rect;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Brush options for eraser strokes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EraserBrush {
    /// Brush diameter in pixels (logical coordinates)
    pub size: f64,
    /// Brush shape
    pub kind: EraserKind,
}

/// Shape of the eraser brush.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub enum EraserKind {
    Circle,
    Rect,
}

/// Label metadata for numbered arrows.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArrowLabel {
    /// Numeric label value.
    pub value: u32,
    /// Font size in points.
    pub size: f64,
    /// Font descriptor (family, weight, style).
    pub font_descriptor: FontDescriptor,
}

/// Label metadata for numbered step markers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepMarkerLabel {
    /// Numeric label value.
    pub value: u32,
    /// Font size in points.
    pub size: f64,
    /// Font descriptor (family, weight, style).
    pub font_descriptor: FontDescriptor,
}

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
    /// Freehand drawing with variable thickness (pressure sensitivity)
    FreehandPressure {
        /// Sequence of (x, y, thickness) coordinates
        points: Vec<(i32, i32, f32)>,
        /// Stroke color
        color: Color,
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
        /// Whether to fill the rectangle
        fill: bool,
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
        /// Whether to fill the ellipse
        fill: bool,
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
        /// Whether the arrowhead sits at the end of the line
        #[serde(default = "default_arrow_head_at_end")]
        head_at_end: bool,
        /// Optional label rendered near the arrow.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<ArrowLabel>,
    },
    /// Numbered step marker bubble.
    StepMarker {
        /// Center X coordinate
        x: i32,
        /// Center Y coordinate
        y: i32,
        /// Fill color for the marker bubble
        color: Color,
        /// Label metadata (number + font)
        label: StepMarkerLabel,
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
        /// Optional wrap width in pixels (None = auto)
        #[serde(default)]
        wrap_width: Option<i32>,
    },
    /// Sticky note with filled background and drop shadow
    StickyNote {
        /// Baseline X coordinate
        x: i32,
        /// Baseline Y coordinate
        y: i32,
        /// Note text content
        text: String,
        /// Background fill color for the note
        background: Color,
        /// Font size in points
        size: f64,
        /// Font descriptor (family, weight, style)
        font_descriptor: FontDescriptor,
        /// Optional wrap width in pixels (None = auto)
        #[serde(default)]
        wrap_width: Option<i32>,
    },
    /// Highlighter-style stroke with translucent ink
    MarkerStroke {
        /// Sequence of (x, y) coordinates traced by the marker
        points: Vec<(i32, i32)>,
        /// Stroke color (alpha controls ink intensity)
        color: Color,
        /// Stroke thickness in pixels
        thick: f64,
    },
    /// Eraser stroke that punches holes in the canvas
    EraserStroke {
        /// Sequence of (x, y) coordinates traced by the eraser
        points: Vec<(i32, i32)>,
        /// Brush options (shape + diameter)
        brush: EraserBrush,
    },
}

impl Shape {
    /// Returns the axis-aligned bounding box for this shape, expanded to cover stroke width.
    ///
    /// The returned rectangle is suitable for dirty region tracking and damage hints.
    /// Returns `None` only when the shape has no drawable area (e.g., degenerate data).
    pub fn bounding_box(&self) -> Option<Rect> {
        match self {
            Shape::Freehand { points, thick, .. } => bounding_box_for_points(points, *thick),
            Shape::FreehandPressure { points, .. } => {
                if points.is_empty() {
                    return None;
                }
                let mut min_x = points[0].0;
                let mut min_y = points[0].1;
                let mut max_x = points[0].0;
                let mut max_y = points[0].1;
                let mut max_thick = 0.0f32;

                for &(x, y, t) in points {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                    max_thick = max_thick.max(t);
                }

                let pad = (max_thick as i32 / 2).max(1);
                Some(Rect {
                    x: min_x - pad,
                    y: min_y - pad,
                    width: (max_x - min_x + 2 * pad).max(1),
                    height: (max_y - min_y + 2 * pad).max(1),
                })
            }
            Shape::Line {
                x1,
                y1,
                x2,
                y2,
                thick,
                ..
            } => bounding_box_for_line(*x1, *y1, *x2, *y2, *thick),
            Shape::Rect {
                x, y, w, h, thick, ..
            } => bounding_box_for_rect(*x, *y, *w, *h, *thick),
            Shape::Ellipse {
                cx,
                cy,
                rx,
                ry,
                thick,
                ..
            } => bounding_box_for_ellipse(*cx, *cy, *rx, *ry, *thick),
            Shape::Arrow {
                x1,
                y1,
                x2,
                y2,
                thick,
                arrow_length,
                arrow_angle,
                head_at_end,
                label,
                color: _,
            } => bounding_box_for_arrow(
                *x1,
                *y1,
                *x2,
                *y2,
                *thick,
                *arrow_length,
                *arrow_angle,
                *head_at_end,
                label.as_ref(),
            ),
            Shape::Text {
                x,
                y,
                text,
                size,
                font_descriptor,
                background_enabled,
                wrap_width,
                ..
            } => bounding_box_for_text(
                *x,
                *y,
                text,
                *size,
                font_descriptor,
                *background_enabled,
                *wrap_width,
            ),
            Shape::StepMarker { x, y, label, .. } => {
                step_marker_bounds(*x, *y, label.value, label.size, &label.font_descriptor)
            }
            Shape::StickyNote {
                x,
                y,
                text,
                size,
                font_descriptor,
                wrap_width,
                ..
            } => bounding_box_for_sticky_note(*x, *y, text, *size, font_descriptor, *wrap_width),
            Shape::MarkerStroke { points, thick, .. } => {
                let inflated = (*thick * 1.35).max(*thick + 1.0);
                bounding_box_for_points(points, inflated)
            }
            Shape::EraserStroke { points, brush } => bounding_box_for_eraser(points, brush.size),
        }
    }

    /// Returns a human-readable label for the shape variant.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Shape::Freehand { .. } | Shape::FreehandPressure { .. } => "Freehand",
            Shape::Line { .. } => "Line",
            Shape::Rect { .. } => "Rectangle",
            Shape::Ellipse { .. } => "Ellipse",
            Shape::Arrow { .. } => "Arrow",
            Shape::Text { .. } => "Text",
            Shape::StickyNote { .. } => "Sticky Note",
            Shape::MarkerStroke { .. } => "Marker",
            Shape::StepMarker { .. } => "Step Marker",
            Shape::EraserStroke { .. } => "Eraser",
        }
    }
}

const fn default_arrow_head_at_end() -> bool {
    true
}
