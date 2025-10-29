//! Shape definitions for screen annotations.

use super::color::Color;
use super::font::FontDescriptor;
use crate::util::{self, Rect};
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

impl Shape {
    /// Returns the axis-aligned bounding box for this shape, expanded to cover stroke width.
    ///
    /// The returned rectangle is suitable for dirty region tracking and damage hints.
    /// Returns `None` only when the shape has no drawable area (e.g., degenerate data).
    pub fn bounding_box(&self) -> Option<Rect> {
        match self {
            Shape::Freehand { points, thick, .. } => bounding_box_for_points(points, *thick),
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
                ..
            } => bounding_box_for_arrow(*x1, *y1, *x2, *y2, *thick, *arrow_length, *arrow_angle),
            Shape::Text {
                x,
                y,
                text,
                size,
                font_descriptor,
                background_enabled,
                ..
            } => bounding_box_for_text(*x, *y, text, *size, font_descriptor, *background_enabled),
        }
    }
}

fn stroke_padding(thick: f64) -> i32 {
    let padding = (thick / 2.0).ceil() as i32;
    padding.max(1)
}

pub(crate) fn bounding_box_for_points(points: &[(i32, i32)], thick: f64) -> Option<Rect> {
    if points.is_empty() {
        return None;
    }
    let mut min_x = points[0].0;
    let mut max_x = points[0].0;
    let mut min_y = points[0].1;
    let mut max_y = points[0].1;

    for &(x, y) in &points[1..] {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    let padding = stroke_padding(thick);
    min_x -= padding;
    max_x += padding;
    min_y -= padding;
    max_y += padding;

    ensure_positive_rect(min_x, min_y, max_x, max_y)
}

pub(crate) fn bounding_box_for_line(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    thick: f64,
) -> Option<Rect> {
    let padding = stroke_padding(thick);

    let min_x = x1.min(x2) - padding;
    let max_x = x1.max(x2) + padding;
    let min_y = y1.min(y2) - padding;
    let max_y = y1.max(y2) + padding;

    ensure_positive_rect(min_x, min_y, max_x, max_y)
}

pub(crate) fn bounding_box_for_rect(x: i32, y: i32, w: i32, h: i32, thick: f64) -> Option<Rect> {
    let padding = stroke_padding(thick);

    let x2 = x + w;
    let y2 = y + h;

    let min_x = x.min(x2) - padding;
    let max_x = x.max(x2) + padding;
    let min_y = y.min(y2) - padding;
    let max_y = y.max(y2) + padding;

    ensure_positive_rect(min_x, min_y, max_x, max_y)
}

pub(crate) fn bounding_box_for_ellipse(
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    thick: f64,
) -> Option<Rect> {
    let padding = stroke_padding(thick);
    let min_x = (cx - rx) - padding;
    let max_x = (cx + rx) + padding;
    let min_y = (cy - ry) - padding;
    let max_y = (cy + ry) + padding;

    ensure_positive_rect(min_x, min_y, max_x, max_y)
}

pub(crate) fn bounding_box_for_arrow(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
) -> Option<Rect> {
    let arrow_points = util::calculate_arrowhead_custom(x1, y1, x2, y2, arrow_length, arrow_angle);

    let mut min_x = x1.min(x2) as f64;
    let mut max_x = x1.max(x2) as f64;
    let mut min_y = y1.min(y2) as f64;
    let mut max_y = y1.max(y2) as f64;

    for &(px, py) in &arrow_points {
        min_x = min_x.min(px);
        max_x = max_x.max(px);
        min_y = min_y.min(py);
        max_y = max_y.max(py);
    }

    let padding = stroke_padding(thick) as f64;

    ensure_positive_rect_f64(
        min_x - padding,
        min_y - padding,
        max_x + padding,
        max_y + padding,
    )
}

pub(crate) fn bounding_box_for_text(
    x: i32,
    y: i32,
    text: &str,
    size: f64,
    font_descriptor: &FontDescriptor,
    background_enabled: bool,
) -> Option<Rect> {
    if text.is_empty() {
        return None;
    }

    // Use a tiny image surface for measurement; the layout is all we need.
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).ok()?;
    let ctx = cairo::Context::new(&surface).ok()?;

    ctx.set_antialias(cairo::Antialias::Best);

    let layout = pangocairo::functions::create_layout(&ctx);

    let font_desc_str = font_descriptor.to_pango_string(size);
    let font_desc = pango::FontDescription::from_string(&font_desc_str);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);

    let (ink_rect, _logical_rect) = layout.extents();

    // Convert Pango units to floats
    let scale = pango::SCALE as f64;
    let ink_x = ink_rect.x() as f64 / scale;
    let ink_y = ink_rect.y() as f64 / scale;
    let ink_width = ink_rect.width() as f64 / scale;
    let ink_height = ink_rect.height() as f64 / scale;
    let baseline = layout.baseline() as f64 / scale;

    let base_x = x as f64;
    let base_y = y as f64 - baseline;

    // Text fill bounds (before outline expansion)
    let mut min_x = base_x + ink_x;
    let mut max_x = min_x + ink_width;
    let mut min_y = base_y + ink_y;
    let mut max_y = min_y + ink_height;

    // Stroke outline expands half the stroke width around text
    let stroke_padding = (size * 0.06) / 2.0;
    min_x -= stroke_padding;
    max_x += stroke_padding;
    min_y -= stroke_padding;
    max_y += stroke_padding;

    // Drop shadow extends the bounds; include union
    let shadow_offset = size * 0.04;
    min_x = min_x.min(base_x + ink_x + shadow_offset - stroke_padding);
    min_y = min_y.min(base_y + ink_y + shadow_offset - stroke_padding);
    max_x = max_x.max(base_x + ink_x + ink_width + shadow_offset + stroke_padding);
    max_y = max_y.max(base_y + ink_y + ink_height + shadow_offset + stroke_padding);

    if background_enabled && ink_width > 0.0 && ink_height > 0.0 {
        let padding = size * 0.15;
        let bg_min_x = base_x + ink_x - padding;
        let bg_min_y = base_y + ink_y - padding;
        let bg_max_x = base_x + ink_x + ink_width + padding;
        let bg_max_y = base_y + ink_y + ink_height + padding;

        min_x = min_x.min(bg_min_x);
        min_y = min_y.min(bg_min_y);
        max_x = max_x.max(bg_max_x);
        max_y = max_y.max(bg_max_y);
    }

    ensure_positive_rect_f64(min_x, min_y, max_x, max_y)
}

fn ensure_positive_rect(min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Option<Rect> {
    let (min_x, max_x) = if min_x == max_x {
        (min_x, max_x + 1)
    } else {
        (min_x, max_x)
    };
    let (min_y, max_y) = if min_y == max_y {
        (min_y, max_y + 1)
    } else {
        (min_y, max_y)
    };
    Rect::from_min_max(min_x, min_y, max_x, max_y)
}

fn ensure_positive_rect_f64(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Option<Rect> {
    let min_x = min_x.floor() as i32;
    let min_y = min_y.floor() as i32;
    let max_x = max_x.ceil() as i32;
    let max_y = max_y.ceil() as i32;
    ensure_positive_rect(min_x, min_y, max_x, max_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{FontDescriptor, color::WHITE};
    use crate::util;

    #[test]
    fn freehand_bounding_box_expands_with_thickness() {
        let shape = Shape::Freehand {
            points: vec![(10, 20), (30, 40)],
            color: WHITE,
            thick: 6.0,
        };

        let rect = shape.bounding_box().expect("freehand should have bounds");
        assert_eq!(rect.x, 7);
        assert_eq!(rect.y, 17);
        assert_eq!(rect.width, 26);
        assert_eq!(rect.height, 26);
    }

    #[test]
    fn line_bounding_box_covers_stroke() {
        let shape = Shape::Line {
            x1: 50,
            y1: 40,
            x2: 70,
            y2: 90,
            color: WHITE,
            thick: 4.0,
        };

        let rect = shape.bounding_box().expect("line should have bounds");
        assert_eq!(rect.x, 48);
        assert_eq!(rect.y, 38);
        assert_eq!(rect.width, 24);
        assert_eq!(rect.height, 54);
    }

    #[test]
    fn arrow_bounding_box_includes_head() {
        let shape = Shape::Arrow {
            x1: 100,
            y1: 100,
            x2: 50,
            y2: 120,
            color: WHITE,
            thick: 3.0,
            arrow_length: 20.0,
            arrow_angle: 30.0,
        };

        let rect = shape.bounding_box().expect("arrow should have bounds");
        let x_min = rect.x;
        let x_max = rect.x + rect.width;
        let y_min = rect.y;
        let y_max = rect.y + rect.height;

        assert!(x_min <= 50 && x_max >= 100);
        assert!(y_min <= 100 && y_max >= 120);

        let arrow_points = util::calculate_arrowhead_custom(100, 100, 50, 120, 20.0, 30.0);
        for &(px, py) in &arrow_points {
            assert!(px >= x_min as f64 && px <= x_max as f64);
            assert!(py >= y_min as f64 && py <= y_max as f64);
        }
    }

    #[test]
    fn ellipse_bounding_box_handles_radii_and_stroke() {
        let shape = Shape::Ellipse {
            cx: 200,
            cy: 150,
            rx: 40,
            ry: 20,
            color: WHITE,
            thick: 2.0,
        };

        let rect = shape.bounding_box().expect("ellipse should have bounds");
        assert_eq!(rect.x, 159);
        assert_eq!(rect.y, 129);
        assert_eq!(rect.width, 82);
        assert_eq!(rect.height, 42);
    }

    #[test]
    fn text_bounding_box_is_non_zero() {
        let shape = Shape::Text {
            x: 10,
            y: 20,
            text: "Hello".to_string(),
            color: WHITE,
            size: 24.0,
            font_descriptor: FontDescriptor::default(),
            background_enabled: true,
        };

        let rect = shape.bounding_box().expect("text should have bounds");
        assert!(rect.width > 0);
        assert!(rect.height > 0);
        assert!(rect.x <= 10);
        assert!(rect.y <= 20);
    }
}
