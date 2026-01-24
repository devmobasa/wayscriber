//! Hit-testing utilities for drawn shapes.

mod geometry;
mod shapes;

#[cfg(test)]
mod tests;

use crate::draw::shape::{arrow_label_layout, step_marker_outline_thickness, step_marker_radius};
use crate::draw::{DrawnShape, Shape};
use crate::util::Rect;

/// Computes a tolerance-aware bounding rectangle for the shape.
pub fn compute_hit_bounds(shape: &DrawnShape, tolerance: f64) -> Option<Rect> {
    let base = shape.shape.bounding_box()?;
    if matches!(shape.shape, Shape::EraserStroke { .. }) {
        return None;
    }
    let inflate = tolerance.ceil() as i32;
    if inflate == 0 {
        return Some(base);
    }
    base.inflated(inflate)
}

/// Returns `true` if the point intersects the provided shape within tolerance.
pub fn hit_test(shape: &DrawnShape, point: (i32, i32), tolerance: f64) -> bool {
    match &shape.shape {
        Shape::Freehand { points, thick, .. } => {
            shapes::freehand_hit(points, point, *thick, tolerance)
        }
        Shape::FreehandPressure { points, .. } => {
            // Check hit against variable width segments
            if points.is_empty() {
                return false;
            }
            // Simplified hit test: check each segment against max thickness or per-segment thickness
            // For precise hit testing, we'd check each segment with its specific width.
            // Using max thickness is a safe over-approximation for now, or we can iterate.

            // Let's iterate segments for better precision
            for i in 0..points.len() - 1 {
                let (x1, y1, t1) = points[i];
                let (x2, y2, t2) = points[i + 1];
                let max_segment_thick = t1.max(t2) as f64;
                if shapes::segment_hit(x1, y1, x2, y2, max_segment_thick, point, tolerance) {
                    return true;
                }
            }
            false
        }
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            thick,
            ..
        } => shapes::segment_hit(*x1, *y1, *x2, *y2, *thick, point, tolerance),
        Shape::Rect {
            x, y, w, h, thick, ..
        } => shapes::rect_outline_hit(*x, *y, *w, *h, *thick, point, tolerance),
        Shape::Ellipse {
            cx,
            cy,
            rx,
            ry,
            thick,
            ..
        } => shapes::ellipse_outline_hit(*cx, *cy, *rx, *ry, *thick, point, tolerance),
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
            ..
        } => {
            let (tip_x, tip_y, tail_x, tail_y) = if *head_at_end {
                (*x2, *y2, *x1, *y1)
            } else {
                (*x1, *y1, *x2, *y2)
            };

            let mut hit = shapes::segment_hit(*x1, *y1, *x2, *y2, *thick, point, tolerance)
                || shapes::arrowhead_hit(
                    tip_x,
                    tip_y,
                    tail_x,
                    tail_y,
                    *arrow_length,
                    *arrow_angle,
                    point,
                    tolerance,
                );
            if !hit && let Some(label) = label {
                let label_text = label.value.to_string();
                if let Some(layout) = arrow_label_layout(
                    tip_x,
                    tip_y,
                    tail_x,
                    tail_y,
                    *thick,
                    &label_text,
                    label.size,
                    &label.font_descriptor,
                ) {
                    let inflate = tolerance.ceil() as i32;
                    let bounds = layout.bounds.inflated(inflate).unwrap_or(layout.bounds);
                    hit = bounds.contains(point.0, point.1);
                }
            }
            hit
        }
        Shape::Text { .. } | Shape::StickyNote { .. } => {
            if let Some(bounds) = shape.shape.bounding_box() {
                let inflate = tolerance.ceil() as i32;
                bounds
                    .inflated(inflate)
                    .unwrap_or(bounds)
                    .contains(point.0, point.1)
            } else {
                false
            }
        }
        Shape::MarkerStroke { points, thick, .. } => {
            let effective_thick = (*thick * 1.35).max(*thick + 1.0);
            shapes::freehand_hit(points, point, effective_thick, tolerance)
        }
        Shape::StepMarker { x, y, label, .. } => {
            let radius = step_marker_radius(label.value, label.size, &label.font_descriptor);
            let outline = step_marker_outline_thickness(label.size);
            shapes::circle_hit(*x, *y, radius + outline / 2.0, point, tolerance)
        }
        Shape::EraserStroke { .. } => false,
    }
}
