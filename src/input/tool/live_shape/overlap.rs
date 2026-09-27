//! Compare an oval's overlapping tail with the first lap at the same angle.

use super::ellipse::{angle_delta, polar};
use super::{Bounds, ellipse};

const MAX_OVAL_TURNS: f64 = 1.5;
const OVERLAP_RADIUS_TOLERANCE: f64 = 0.08;
// Pixel rounding and an uneven fitted center can shift the measured winding.
const WINDING_MARGIN_TURNS: f64 = 0.02;

pub(super) fn follows_first_lap(points: &[(i32, i32)], lap_end: usize, bounds: Bounds) -> bool {
    let Some(coverage) = ellipse::angular_travel(points, bounds) else {
        return false;
    };
    if coverage.winding.abs() > std::f64::consts::TAU * (MAX_OVAL_TURNS + WINDING_MARGIN_TURNS)
        || coverage.travel > coverage.winding.abs() * 1.5
    {
        return false;
    }

    let direction = coverage.winding.signum();
    let (start_angle, start_radius) = polar(points[0], bounds);
    let mut previous_angle = start_angle;
    let mut winding = 0.0;
    let mut last_position = 0.0;
    let mut profile = vec![(0.0, start_radius)];
    for &point in &points[1..=lap_end] {
        let (angle, radius) = polar(point, bounds);
        winding += angle_delta(previous_angle, angle) * direction;
        if winding > last_position {
            profile.push((winding, radius));
            last_position = winding;
        }
        previous_angle = angle;
    }
    if last_position < std::f64::consts::TAU {
        profile.push((std::f64::consts::TAU, start_radius));
    }

    // Keep the overlap tolerance independent of the one-lap fit sensitivity.
    // Scale with the shorter radius so a large oval can be retraced by hand,
    // while a small spiral still exceeds the allowed radial drift.
    let margin = (bounds.width.min(bounds.height) / 2.0 * OVERLAP_RADIUS_TOLERANCE).max(2.0);
    for &point in &points[lap_end + 1..] {
        let (angle, radius) = polar(point, bounds);
        let progress = ((angle - start_angle) * direction).rem_euclid(std::f64::consts::TAU);
        let upper = profile.partition_point(|&(position, _)| position < progress);
        let expected = if upper == 0 {
            start_radius
        } else {
            let (a, ra) = profile[upper - 1];
            let (b, rb) = profile[upper];
            ra + (rb - ra) * (progress - a) / (b - a)
        };
        let ray_scale = (bounds.width / 2.0 * angle.cos()).hypot(bounds.height / 2.0 * angle.sin());
        if (radius - expected).abs() * ray_scale > margin {
            return false;
        }
    }

    true
}
