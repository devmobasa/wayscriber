use super::bounds::{
    bounding_box_for_arrow, bounding_box_for_blur, bounding_box_for_ellipse,
    bounding_box_for_eraser, bounding_box_for_line, bounding_box_for_points,
    bounding_box_for_pressure_points, bounding_box_for_rect, ensure_positive_rect_i64,
};
use super::polygon::{PolygonKind, bounding_box_for_polygon};
use super::step_marker::step_marker_bounds;
use super::text::{bounding_box_for_sticky_note, bounding_box_for_text};
use crate::draw::color::Color;
use crate::draw::font::FontDescriptor;
use crate::util::Rect;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;

/// Encoded image payload stored directly on an image shape.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddedImage {
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    #[serde(with = "base64_bytes")]
    pub bytes: Arc<[u8]>,
}

/// Brush options for eraser strokes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EraserBrush {
    /// Brush diameter in pixels (logical coordinates)
    pub size: f64,
    /// Brush shape
    pub kind: EraserKind,
}

/// Shape of the eraser brush.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum EraserKind {
    Circle,
    Rect,
}

/// How a blur rectangle obscures whatever sits beneath it.
///
/// The variants form a ladder of increasing information loss. `Gaussian` softens
/// detail and is the historical behavior; `Pixelate` averages fixed blocks;
/// `Secure` and `BlackOut` retain no detail from the source region at all, which
/// is what makes them suitable for redacting exported captures.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum BlurStyle {
    /// Multi-pass downsample and resample. Softens detail without erasing it.
    #[default]
    Gaussian,
    /// Coarse mosaic of averaged blocks.
    Pixelate,
    /// Whole region collapsed to a single averaged color. No detail survives.
    Secure,
    /// Opaque black fill. Ignores the backdrop entirely.
    BlackOut,
}

impl BlurStyle {
    /// Every style, in the order the toolbar and cycling action step through them.
    pub const ALL: [Self; 4] = [Self::Gaussian, Self::Pixelate, Self::Secure, Self::BlackOut];

    /// Short human-readable name for toolbars, menus, and toasts.
    pub fn label(self) -> &'static str {
        match self {
            Self::Gaussian => "Blur",
            Self::Pixelate => "Pixelate",
            Self::Secure => "Secure",
            Self::BlackOut => "Black out",
        }
    }

    /// Next style in [`Self::ALL`] order, wrapping at the end.
    pub fn next(self) -> Self {
        match self {
            Self::Gaussian => Self::Pixelate,
            Self::Pixelate => Self::Secure,
            Self::Secure => Self::BlackOut,
            Self::BlackOut => Self::Gaussian,
        }
    }

    /// Whether the style needs the captured backdrop to render.
    pub fn needs_backdrop(self) -> bool {
        !matches!(self, Self::BlackOut)
    }
}

/// How an arrow's shaft and head are shaped.
///
/// The variants are drawing styles, not different shapes: every one of them
/// still stores the same two endpoints and the same head sizing, so switching
/// styles never loses geometry. `Standard` is what arrows looked like before
/// styles existed, which is what makes it the serde default.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ArrowStyle {
    /// Tapered shaft fused into one head. The historical arrow.
    #[default]
    Standard,
    /// Dart head: the rear edge is notched forward into a concave V.
    Pointy,
    /// Shaft follows a quadratic Bezier arc so it can route around whatever
    /// sits between the pointer and its target.
    Curved,
    /// Parallel-sided shaft with a head at both ends.
    Double,
}

impl ArrowStyle {
    /// Every style, in the order the toolbar and cycling action step through them.
    pub const ALL: [Self; 4] = [Self::Standard, Self::Pointy, Self::Curved, Self::Double];

    /// Short human-readable name for toolbars, menus, and toasts.
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Pointy => "Pointy",
            Self::Curved => "Curved",
            Self::Double => "Double",
        }
    }

    /// Next style in [`Self::ALL`] order, wrapping at the end.
    pub fn next(self) -> Self {
        match self {
            Self::Standard => Self::Pointy,
            Self::Pointy => Self::Curved,
            Self::Curved => Self::Double,
            Self::Double => Self::Standard,
        }
    }

    /// Previous style in [`Self::ALL`] order, wrapping at the start.
    ///
    /// The properties panel steps entries in both directions, so a four-way
    /// cycle needs a way back that is not three presses forward.
    pub fn previous(self) -> Self {
        match self {
            Self::Standard => Self::Double,
            Self::Pointy => Self::Standard,
            Self::Curved => Self::Pointy,
            Self::Double => Self::Curved,
        }
    }

    /// Whether this style bends its shaft, and so needs the `bend` field and
    /// the curve sampler rather than the straight-line fast paths.
    pub fn is_curved(self) -> bool {
        matches!(self, Self::Curved)
    }

    /// The bend this style actually draws.
    ///
    /// Every arrow carries a `bend` so a trip round the style cycle does not
    /// lose the arc the user shaped, which means readers have to ask the style
    /// whether that stored value is on screen before they position anything
    /// against it.
    pub fn effective_bend(self, bend: f64) -> f64 {
        if self.is_curved() { bend } else { 0.0 }
    }
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
    /// Generic closed polygon, including named generated polygons and freeform polygons.
    Polygon {
        /// Polygon metadata used by UI labels and future editing.
        kind: PolygonKind,
        /// Concrete persisted vertices. These are the source of truth for rendering.
        points: Vec<(i32, i32)>,
        /// Whether to fill the polygon.
        fill: bool,
        /// Border/fill color.
        color: Color,
        /// Border thickness in pixels.
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
        /// How the shaft and head are drawn. Absent in sessions written before
        /// styles existed, which deserialize as the historical straight arrow.
        #[serde(default)]
        style: ArrowStyle,
        /// Signed bend as a fraction of the tail-to-tip chord length. `0.0` is
        /// straight; positive bulges toward the left of the tail-to-tip
        /// direction (screen coordinates, y down), negative toward the right.
        ///
        /// Only [`ArrowStyle::Curved`] draws it, but every arrow carries it so
        /// switching a curved arrow to another style and back keeps the arc the
        /// user shaped.
        #[serde(default)]
        bend: f64,
        /// Optional label rendered near the arrow.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<ArrowLabel>,
    },
    /// Rectangular blur region over the captured background.
    BlurRect {
        /// Top-left X coordinate
        x: i32,
        /// Top-left Y coordinate
        y: i32,
        /// Width in pixels
        w: i32,
        /// Height in pixels
        h: i32,
        /// Blur strength, reusing the tool size slider semantics
        strength: f64,
        /// How the region is obscured. Absent in sessions written before styles
        /// existed, which deserialize as the historical Gaussian blur.
        #[serde(default)]
        style: BlurStyle,
    },
    /// Region that stays bright while the spotlight pass dims everything else.
    ///
    /// Draws nothing on its own; it is consumed by the spotlight compositing
    /// pass, which needs every region at once to build a single dim layer.
    Spotlight {
        /// Center X coordinate
        cx: i32,
        /// Center Y coordinate
        cy: i32,
        /// Horizontal radius in pixels
        rx: i32,
        /// Vertical radius in pixels
        ry: i32,
        /// Per-shape loupe factor. 1.0 preserves the historical bright opening.
        #[serde(
            default = "crate::draw::default_spotlight_magnification",
            deserialize_with = "crate::draw::deserialize_spotlight_magnification"
        )]
        magnification: f64,
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
    /// Embedded raster image pasted from the clipboard.
    Image {
        /// Top-left X coordinate
        x: i32,
        /// Top-left Y coordinate
        y: i32,
        /// Display width in logical canvas pixels
        w: i32,
        /// Display height in logical canvas pixels
        h: i32,
        /// Original encoded image payload and natural dimensions
        data: EmbeddedImage,
    },
}

impl Shape {
    /// Returns the axis-aligned bounding box for this shape, expanded to cover stroke width.
    ///
    /// The returned rectangle is suitable for dirty region tracking and damage hints.
    /// Returns `None` when the shape has no drawable area or its full bounds cannot be
    /// represented safely by [`Rect`].
    pub fn bounding_box(&self) -> Option<Rect> {
        match self {
            Shape::Freehand { points, thick, .. } => bounding_box_for_points(points, *thick),
            Shape::FreehandPressure { points, .. } => bounding_box_for_pressure_points(points),
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
            Shape::Spotlight { cx, cy, rx, ry, .. } => {
                bounding_box_for_ellipse(*cx, *cy, *rx, *ry, 0.0)
            }
            Shape::Polygon { points, thick, .. } => bounding_box_for_polygon(points, *thick),
            Shape::Arrow {
                x1,
                y1,
                x2,
                y2,
                thick,
                arrow_length,
                arrow_angle,
                head_at_end,
                style,
                bend,
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
                *style,
                *bend,
                label.as_ref(),
            ),
            Shape::BlurRect { x, y, w, h, .. } => bounding_box_for_blur(*x, *y, *w, *h),
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
            Shape::Image { x, y, w, h, .. } => normalized_rect(*x, *y, *w, *h),
        }
    }

    /// Returns a human-readable label for the shape variant.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Shape::Freehand { .. } | Shape::FreehandPressure { .. } => "Freehand",
            Shape::Line { .. } => "Line",
            Shape::Rect { .. } => "Rectangle",
            Shape::Ellipse { .. } => "Ellipse",
            Shape::Spotlight { .. } => "Spotlight",
            Shape::Polygon { kind, .. } => kind.label(),
            Shape::Arrow { .. } => "Arrow",
            Shape::BlurRect { .. } => "Blur",
            Shape::Text { .. } => "Text",
            Shape::StickyNote { .. } => "Sticky Note",
            Shape::MarkerStroke { .. } => "Marker",
            Shape::StepMarker { .. } => "Step Marker",
            Shape::EraserStroke { .. } => "Eraser",
            Shape::Image { .. } => "Image",
        }
    }
}

const fn default_arrow_head_at_end() -> bool {
    true
}

fn normalized_rect(x: i32, y: i32, w: i32, h: i32) -> Option<Rect> {
    let x = i64::from(x);
    let y = i64::from(y);
    let x2 = x + i64::from(w);
    let y2 = y + i64::from(h);
    ensure_positive_rect_i64(x.min(x2), y.min(y2), x.max(x2), y.max(y2))
}

mod base64_bytes {
    use super::*;

    pub fn serialize<S>(bytes: &Arc<[u8]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        crate::base64::encode_standard(bytes).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<[u8]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        crate::base64::decode_standard(&encoded)
            .map(Arc::from)
            .map_err(serde::de::Error::custom)
    }
}
