//! Hit-testing utilities for drawn shapes.

use crate::draw::{DrawnShape, Shape};
use crate::util::{self, Rect};

const EPS: f64 = 1e-6;

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
        Shape::Freehand { points, thick, .. } => freehand_hit(points, point, *thick, tolerance),
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            thick,
            ..
        } => segment_hit(*x1, *y1, *x2, *y2, *thick, point, tolerance),
        Shape::Rect {
            x, y, w, h, thick, ..
        } => rect_outline_hit(*x, *y, *w, *h, *thick, point, tolerance),
        Shape::Ellipse {
            cx,
            cy,
            rx,
            ry,
            thick,
            ..
        } => ellipse_outline_hit(*cx, *cy, *rx, *ry, *thick, point, tolerance),
        Shape::Arrow {
            x1,
            y1,
            x2,
            y2,
            thick,
            arrow_length,
            arrow_angle,
            ..
        } => {
            segment_hit(*x1, *y1, *x2, *y2, *thick, point, tolerance)
                || arrowhead_hit(
                    *x1,
                    *y1,
                    *x2,
                    *y2,
                    *arrow_length,
                    *arrow_angle,
                    point,
                    tolerance,
                )
        }
        Shape::Text { .. } => {
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
            freehand_hit(points, point, effective_thick, tolerance)
        }
        Shape::EraserStroke { .. } => false,
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::draw::{BLACK, DrawnShape, EraserBrush, EraserKind, Shape};

    #[test]
    fn compute_hit_bounds_inflates_bounds_for_tolerance() {
        let drawn = DrawnShape {
            id: 1,
            shape: Shape::Rect {
                x: 10,
                y: 20,
                w: 30,
                h: 40,
                fill: false,
                color: BLACK,
                thick: 2.0,
            },
            created_at: 0,
            locked: false,
        };

        let base = drawn
            .shape
            .bounding_box()
            .expect("rect should have base bounds");
        let expanded =
            compute_hit_bounds(&drawn, 4.2).expect("compute_hit_bounds should expand rectangle");

        assert!(expanded.x <= base.x);
        assert!(expanded.y <= base.y);
        assert!(expanded.width >= base.width);
        assert!(expanded.height >= base.height);
    }

    #[test]
    fn compute_hit_bounds_ignores_eraser_strokes() {
        let eraser = DrawnShape {
            id: 2,
            shape: Shape::EraserStroke {
                points: vec![(0, 0), (10, 10)],
                brush: EraserBrush {
                    size: 8.0,
                    kind: EraserKind::Circle,
                },
            },
            created_at: 0,
            locked: false,
        };

        assert!(
            compute_hit_bounds(&eraser, 5.0).is_none(),
            "eraser strokes should not participate in hit bounds"
        );
    }

    #[test]
    fn rect_hit_handles_degenerate_dimensions() {
        let rect = DrawnShape {
            id: 1,
            shape: Shape::Rect {
                x: 10,
                y: 10,
                w: 0,
                h: 20,
                fill: false,
                color: BLACK,
                thick: 2.0,
            },
            created_at: 0,
            locked: false,
        };

        assert!(hit_test(&rect, (10, 10), 3.0));
        assert!(!hit_test(&rect, (5, 5), 2.0));
    }

    #[test]
    fn ellipse_hit_handles_zero_radius() {
        let ellipse = DrawnShape {
            id: 2,
            shape: Shape::Ellipse {
                cx: 50,
                cy: 80,
                rx: 0,
                ry: 0,
                fill: false,
                color: BLACK,
                thick: 2.0,
            },
            created_at: 0,
            locked: false,
        };

        assert!(hit_test(&ellipse, (50, 80), 2.0));
        assert!(!hit_test(&ellipse, (60, 90), 1.0));
    }

    #[test]
    fn arrowhead_hit_detects_point_near_tip_and_rejects_distant_point() {
        // Arrow pointing upwards from tail at (0, -20) to tip at (0, 0).
        let tip = (0, 0);
        let tail = (0, -20);

        assert!(
            arrowhead_hit(tip.0, tip.1, tail.0, tail.1, 10.0, 30.0, tip, 0.5),
            "tip point should be inside arrowhead"
        );

        assert!(
            !arrowhead_hit(tip.0, tip.1, tail.0, tail.1, 10.0, 30.0, (50, 50), 0.5),
            "faraway point should not be inside arrowhead even with tolerance"
        );
    }

    #[test]
    fn distance_point_to_segment_matches_point_distance_for_zero_length_segment() {
        let start = (10, 10);
        let point = (13, 14);

        let seg_dist = distance_point_to_segment(point, start, start);
        let direct = distance_point_to_point(start, point);

        assert!(
            (seg_dist - direct).abs() < 1e-6,
            "distance to zero-length segment should equal point distance"
        );
    }
}

fn freehand_hit(points: &[(i32, i32)], point: (i32, i32), thickness: f64, tolerance: f64) -> bool {
    if points.is_empty() {
        return false;
    }
    let padded = tolerance.max(thickness / 2.0);
    let mut prev = points[0];
    if distance_point_to_point(prev, point) <= padded {
        return true;
    }
    for &next in &points[1..] {
        if distance_point_to_segment(point, prev, next) <= padded {
            return true;
        }
        prev = next;
    }
    false
}

fn segment_hit(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    thickness: f64,
    point: (i32, i32),
    tolerance: f64,
) -> bool {
    let padded = tolerance.max(thickness / 2.0);
    distance_point_to_segment(point, (x1, y1), (x2, y2)) <= padded
}

fn rect_outline_hit(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    thickness: f64,
    point: (i32, i32),
    tolerance: f64,
) -> bool {
    let tolerance = tolerance.max(thickness / 2.0);
    let (px, py) = (point.0 as f64, point.1 as f64);
    let left = x as f64;
    let right = (x + w) as f64;
    let top = y as f64;
    let bottom = (y + h) as f64;

    if w <= 0 || h <= 0 {
        let dx = (px - left).abs();
        let dy = (py - top).abs();
        return dx <= tolerance && dy <= tolerance;
    }

    let outer_hit = px >= left - tolerance
        && px <= right + tolerance
        && py >= top - tolerance
        && py <= bottom + tolerance;
    if !outer_hit {
        return false;
    }

    let vertical_hit = px >= left
        && px <= right
        && ((py - top).abs() <= tolerance || (py - bottom).abs() <= tolerance);
    let horizontal_hit = py >= top
        && py <= bottom
        && ((px - left).abs() <= tolerance || (px - right).abs() <= tolerance);

    vertical_hit || horizontal_hit
}

fn ellipse_outline_hit(
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    thickness: f64,
    point: (i32, i32),
    tolerance: f64,
) -> bool {
    let (px, py) = (point.0 as f64, point.1 as f64);
    let (cx, cy) = (cx as f64, cy as f64);
    let rx = rx.max(0) as f64;
    let ry = ry.max(0) as f64;
    if rx.abs() < EPS && ry.abs() < EPS {
        return distance_point_to_point((cx as i32, cy as i32), point)
            <= tolerance.max(thickness / 2.0);
    }

    let inflate = tolerance.max(thickness / 2.0);
    let rx_outer = (rx + inflate).max(1.0);
    let ry_outer = (ry + inflate).max(1.0);
    let rx_inner = (rx - inflate).max(0.0);
    let ry_inner = (ry - inflate).max(0.0);

    let dx = px - cx;
    let dy = py - cy;

    let outer = (dx * dx) / (rx_outer * rx_outer) + (dy * dy) / (ry_outer * ry_outer) <= 1.0 + EPS;

    let inner = if rx_inner < EPS || ry_inner < EPS {
        false
    } else {
        (dx * dx) / (rx_inner * rx_inner) + (dy * dy) / (ry_inner * ry_inner) <= 1.0 - EPS
    };

    outer && !inner
}

#[allow(clippy::too_many_arguments)]
fn arrowhead_hit(
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    arrow_length: f64,
    arrow_angle: f64,
    point: (i32, i32),
    tolerance: f64,
) -> bool {
    let [(left_x, left_y), (right_x, right_y)] =
        util::calculate_arrowhead_custom(tip_x, tip_y, tail_x, tail_y, arrow_length, arrow_angle);
    let tip = (tip_x as f64, tip_y as f64);
    let p = (point.0 as f64, point.1 as f64);
    if point_in_triangle(p, tip, (left_x, left_y), (right_x, right_y)) {
        return true;
    }
    // Permit small tolerance around the triangle edges.
    let padded = tolerance.max(1.0);
    let edges = [
        ((tip.0, tip.1), (left_x, left_y)),
        ((tip.0, tip.1), (right_x, right_y)),
        ((left_x, left_y), (right_x, right_y)),
    ];
    edges.iter().any(|&(a, b)| {
        distance_point_to_segment(p_as_i32(p), to_i32_pair(a), to_i32_pair(b)) <= padded
    })
}

fn distance_point_to_segment(point: (i32, i32), start: (i32, i32), end: (i32, i32)) -> f64 {
    let (px, py) = (point.0 as f64, point.1 as f64);
    let (x1, y1) = (start.0 as f64, start.1 as f64);
    let (x2, y2) = (end.0 as f64, end.1 as f64);
    let vx = x2 - x1;
    let vy = y2 - y1;
    let len_sq = vx * vx + vy * vy;
    if len_sq.abs() < EPS {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = ((px - x1) * vx + (py - y1) * vy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj_x = x1 + t * vx;
    let proj_y = y1 + t * vy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

fn distance_point_to_point(a: (i32, i32), b: (i32, i32)) -> f64 {
    let dx = (a.0 - b.0) as f64;
    let dy = (a.1 - b.1) as f64;
    (dx * dx + dy * dy).sqrt()
}

fn point_in_triangle(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let (px, py) = p;
    let (ax, ay) = a;
    let (bx, by) = b;
    let (cx, cy) = c;
    let v0 = (cx - ax, cy - ay);
    let v1 = (bx - ax, by - ay);
    let v2 = (px - ax, py - ay);

    let dot00 = v0.0 * v0.0 + v0.1 * v0.1;
    let dot01 = v0.0 * v1.0 + v0.1 * v1.1;
    let dot02 = v0.0 * v2.0 + v0.1 * v2.1;
    let dot11 = v1.0 * v1.0 + v1.1 * v1.1;
    let dot12 = v1.0 * v2.0 + v1.1 * v2.1;

    let denom = dot00 * dot11 - dot01 * dot01;
    if denom.abs() < EPS {
        return false;
    }
    let inv_denom = 1.0 / denom;
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    u >= -EPS && v >= -EPS && (u + v) <= 1.0 + EPS
}

fn to_i32_pair(p: (f64, f64)) -> (i32, i32) {
    (p.0.round() as i32, p.1.round() as i32)
}

fn p_as_i32(p: (f64, f64)) -> (i32, i32) {
    (p.0.round() as i32, p.1.round() as i32)
}
