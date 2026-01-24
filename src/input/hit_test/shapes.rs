use crate::util;

use super::geometry::{
    EPS, distance_point_to_point, distance_point_to_segment, p_as_i32, point_in_triangle,
    to_i32_pair,
};

pub(super) fn freehand_hit(
    points: &[(i32, i32)],
    point: (i32, i32),
    thickness: f64,
    tolerance: f64,
) -> bool {
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

pub(super) fn segment_hit(
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

pub(super) fn rect_outline_hit(
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

pub(super) fn ellipse_outline_hit(
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

pub(super) fn circle_hit(
    cx: i32,
    cy: i32,
    radius: f64,
    point: (i32, i32),
    tolerance: f64,
) -> bool {
    let dx = point.0 as f64 - cx as f64;
    let dy = point.1 as f64 - cy as f64;
    let r = radius + tolerance.max(0.5);
    (dx * dx + dy * dy) <= r * r
}

#[allow(clippy::too_many_arguments)]
pub(super) fn arrowhead_hit(
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
