//! Hit-testing utilities for drawn shapes.

mod geometry;
mod shapes;

#[cfg(test)]
mod tests;

use crate::draw::shape::{arrow_label_layout, step_marker_outline_thickness, step_marker_radius};
use crate::draw::{DrawnShape, Shape};
use crate::util::Rect;

const MAX_HIT_TEST_TOLERANCE: f64 = i32::MAX as f64;

/// A finite, non-negative tolerance that is safe to convert for integer inflation.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub(crate) struct HitTestTolerance(f64);

impl HitTestTolerance {
    pub(crate) const ONE_PIXEL: Self = Self(1.0);

    pub(crate) fn new(value: f64) -> Option<Self> {
        (value.is_finite() && (0.0..=MAX_HIT_TEST_TOLERANCE).contains(&value))
            .then_some(Self(value))
    }

    pub(crate) fn value(self) -> f64 {
        self.0
    }

    fn ceil_i32(self) -> i32 {
        self.0.ceil() as i32
    }

    pub(crate) fn at_least(self, minimum: Self) -> Self {
        Self(self.0.max(minimum.0))
    }
}

/// Computes a tolerance-aware bounding rectangle for the shape.
pub fn compute_hit_bounds(shape: &DrawnShape, tolerance: f64) -> Option<Rect> {
    compute_hit_bounds_with_tolerance(shape, HitTestTolerance::new(tolerance)?)
}

pub(crate) fn compute_hit_bounds_with_tolerance(
    shape: &DrawnShape,
    tolerance: HitTestTolerance,
) -> Option<Rect> {
    let base = shape.bounding_box()?;
    if matches!(shape.shape, Shape::EraserStroke { .. }) {
        return None;
    }
    let inflate = tolerance.ceil_i32();
    if inflate == 0 {
        return Some(base);
    }
    base.inflated(inflate)
}

/// Returns `true` if the point intersects the provided shape within tolerance.
pub fn hit_test(shape: &DrawnShape, point: (i32, i32), tolerance: f64) -> bool {
    let Some(tolerance) = HitTestTolerance::new(tolerance) else {
        return false;
    };
    hit_test_with_tolerance(shape, point, tolerance)
}

pub(crate) fn hit_test_with_tolerance(
    shape: &DrawnShape,
    point: (i32, i32),
    tolerance: HitTestTolerance,
) -> bool {
    let tolerance = tolerance.value();
    match &shape.shape {
        Shape::Freehand { points, thick, .. } => {
            shapes::freehand_hit(points, point, *thick, tolerance)
        }
        Shape::FreehandPressure { points, .. } => {
            shapes::freehand_pressure_hit(points, point, tolerance)
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
        Shape::Spotlight { cx, cy, rx, ry, .. } => {
            // No stroke to aim at, so the whole opening is the target.
            shapes::ellipse_fill_hit(*cx, *cy, *rx, *ry, point)
        }
        Shape::Polygon { points, thick, .. } => {
            shapes::polygon_outline_hit(points, *thick, point, tolerance)
        }
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
                    *thick,
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
        Shape::BlurRect { .. } => {
            let inflate = tolerance.ceil() as i32;
            if let Some(bounds) = shape.bounding_box() {
                bounds
                    .inflated(inflate)
                    .unwrap_or(bounds)
                    .contains(point.0, point.1)
            } else {
                false
            }
        }
        Shape::Text { .. } | Shape::StickyNote { .. } | Shape::Image { .. } => {
            if let Some(bounds) = shape.bounding_box() {
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

/// Returns `true` when a point should target a shape for selection or menus.
///
/// Stroke erasing intentionally keeps using `hit_test`, while direct point
/// targeting includes filled interiors for closed fill-capable shapes.
pub fn hit_test_for_point_targeting(shape: &DrawnShape, point: (i32, i32), tolerance: f64) -> bool {
    let Some(tolerance) = HitTestTolerance::new(tolerance) else {
        return false;
    };
    hit_test_for_point_targeting_with_tolerance(shape, point, tolerance)
}

pub(crate) fn hit_test_for_point_targeting_with_tolerance(
    shape: &DrawnShape,
    point: (i32, i32),
    tolerance: HitTestTolerance,
) -> bool {
    if hit_test_with_tolerance(shape, point, tolerance) {
        return true;
    }

    match &shape.shape {
        Shape::Rect {
            x,
            y,
            w,
            h,
            fill: true,
            ..
        } => shapes::rect_fill_hit(*x, *y, *w, *h, point),
        Shape::Ellipse {
            cx,
            cy,
            rx,
            ry,
            fill: true,
            ..
        } => shapes::ellipse_fill_hit(*cx, *cy, *rx, *ry, point),
        Shape::Polygon {
            points, fill: true, ..
        } => shapes::polygon_fill_hit(points, point),
        _ => false,
    }
}
