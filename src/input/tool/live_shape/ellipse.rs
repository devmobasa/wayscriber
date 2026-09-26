//! Oval fitting and shared polar geometry for angular coverage.

use super::{Bounds, ClosedFit};
use crate::draw::{Color, Shape};

pub(super) struct AngularTravel {
    pub(super) winding: f64,
    pub(super) travel: f64,
    pub(super) first_lap_end: Option<usize>,
}

pub(super) fn angular_travel(points: &[(i32, i32)], bounds: Bounds) -> Option<AngularTravel> {
    let mut winding = 0.0_f64;
    let mut travel = 0.0_f64;
    let mut first_lap_end = None;
    let mut previous_angle: Option<f64> = None;

    for (index, &point) in points.iter().enumerate() {
        let (x, y) = normalized_point(point, bounds);
        let angle = y.atan2(x);
        if let Some(previous) = previous_angle {
            let delta = angle_delta(previous, angle);
            if delta.abs() > std::f64::consts::FRAC_PI_2 + 0.1 {
                return None;
            }
            winding += delta;
            travel += delta.abs();
            // Allow only floating-point summation error at the lap boundary.
            if first_lap_end.is_none() && winding.abs() >= std::f64::consts::TAU - 1e-9 {
                first_lap_end = Some(index);
            }
        }
        previous_angle = Some(angle);
    }

    Some(AngularTravel {
        winding,
        travel,
        first_lap_end,
    })
}

pub(super) fn fit_ellipse(
    points: &[(i32, i32)],
    bounds: Bounds,
    length: f64,
    color: Color,
    thick: f64,
    sensitivity: u8,
) -> Option<ClosedFit> {
    let level = f64::from(sensitivity);
    let rx = bounds.width / 2.0;
    let ry = bounds.height / 2.0;
    if rx.max(ry) / rx.min(ry) > 2.5 || has_polygon_corners(points, bounds) {
        return None;
    }

    let circumference =
        std::f64::consts::PI * (3.0 * (rx + ry) - ((3.0 * rx + ry) * (rx + 3.0 * ry)).sqrt());
    if !(circumference * (0.74 - 0.03 * level)..=circumference * (1.3 + 0.05 * level))
        .contains(&length)
    {
        return None;
    }

    let (cx, cy) = bounds.center();
    let mut error = 0.0_f64;
    let mut worst = 0.0_f64;
    for &point in points {
        let (x, y) = normalized_point(point, bounds);
        let radius = x.hypot(y);
        let deviation = (radius - 1.0).abs();
        error += deviation;
        worst = worst.max(deviation);
    }
    let error = error / points.len() as f64;
    if error > 0.07 + 0.025 * level || worst > 0.2 + 0.05 * level {
        return None;
    }

    Some(ClosedFit {
        shape: Shape::Ellipse {
            cx: cx.round() as i32,
            cy: cy.round() as i32,
            rx: rx.round() as i32,
            ry: ry.round() as i32,
            fill: false,
            color,
            thick,
        },
        error,
    })
}

fn has_polygon_corners(points: &[(i32, i32)], bounds: Bounds) -> bool {
    // One kink can be hand jitter; three clear turns describe a polygon.
    let min_segment = (bounds.width.min(bounds.height) * 0.08).max(3.0);
    let mut corners = 0;
    for triplet in points.windows(3) {
        let first = (
            f64::from(triplet[1].0) - f64::from(triplet[0].0),
            f64::from(triplet[1].1) - f64::from(triplet[0].1),
        );
        let second = (
            f64::from(triplet[2].0) - f64::from(triplet[1].0),
            f64::from(triplet[2].1) - f64::from(triplet[1].1),
        );
        let first_length = first.0.hypot(first.1);
        let second_length = second.0.hypot(second.1);
        if first_length.min(second_length) < min_segment {
            continue;
        }
        if first.0 * second.0 + first.1 * second.1 < 0.25 * first_length * second_length {
            corners += 1;
            if corners >= 3 {
                return true;
            }
        }
    }
    false
}

pub(super) fn polar(point: (i32, i32), bounds: Bounds) -> (f64, f64) {
    let (x, y) = normalized_point(point, bounds);

    (y.atan2(x), x.hypot(y))
}

fn normalized_point(point: (i32, i32), bounds: Bounds) -> (f64, f64) {
    let (cx, cy) = bounds.center();

    (
        (f64::from(point.0) - cx) / (bounds.width / 2.0),
        (f64::from(point.1) - cy) / (bounds.height / 2.0),
    )
}

pub(super) fn angle_delta(previous: f64, angle: f64) -> f64 {
    (angle - previous + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI
}
