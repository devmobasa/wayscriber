//! Recognize a confident line or circle from a pen path. Ambiguous ink stays ink.

use crate::domain::{BoardGrid, BoardGridKind};
use crate::draw::{Color, Shape};

pub(super) fn recognize(
    points: &[(i32, i32)],
    color: Color,
    thick: f64,
    grid: BoardGrid,
) -> Option<Shape> {
    let first = *points.first()?;
    let last = *points.last()?;
    let (min_x, max_x, min_y, max_y) = points.iter().fold(
        (i32::MAX, i32::MIN, i32::MAX, i32::MIN),
        |(min_x, max_x, min_y, max_y), &(x, y)| {
            (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
        },
    );
    let width = f64::from(max_x) - f64::from(min_x);
    let height = f64::from(max_y) - f64::from(min_y);
    let diameter = width.max(height);
    let chord = distance(first, last);
    let length: f64 = points
        .windows(2)
        .map(|pair| distance(pair[0], pair[1]))
        .sum();

    if diameter >= 24.0
        && (width - height).abs() <= diameter * 0.25
        && chord <= diameter * 0.2
        && let Some(circle) = recognize_circle(points, min_x, max_x, min_y, max_y, length)
    {
        let (cx, cy, radius) = circle;
        return Some(Shape::Ellipse {
            cx,
            cy,
            rx: radius,
            ry: radius,
            fill: false,
            color,
            thick,
        });
    }

    if chord < 16.0 || length > chord * 1.16 {
        return None;
    }

    let deviation = points
        .iter()
        .map(|&(x, y)| {
            (((f64::from(x) - f64::from(first.0)) * (f64::from(last.1) - f64::from(first.1))
                - (f64::from(y) - f64::from(first.1)) * (f64::from(last.0) - f64::from(first.0)))
                / chord)
                .abs()
        })
        .fold(0.0_f64, f64::max);
    if deviation > (chord * 0.08).max(4.0) {
        return None;
    }

    let (start, end) = align_line(first, last, chord, grid);

    Some(Shape::Line {
        x1: start.0,
        y1: start.1,
        x2: end.0,
        y2: end.1,
        color,
        thick,
    })
}

fn align_line(
    first: (i32, i32),
    last: (i32, i32),
    chord: f64,
    grid: BoardGrid,
) -> ((i32, i32), (i32, i32)) {
    let dx = f64::from(last.0) - f64::from(first.0);
    let dy = f64::from(last.1) - f64::from(first.1);
    let spacing = f64::from(grid.spacing());

    if dy.abs() <= chord * 0.12 {
        let y = if grid.kind == BoardGridKind::Cartesian {
            snap_to_grid(f64::from(first.1), f64::from(last.1), spacing).unwrap_or(first.1)
        } else {
            first.1
        };
        return ((first.0, y), (last.0, y));
    }

    if dx.abs() <= chord * 0.12 {
        let vertical_spacing = match grid.kind {
            BoardGridKind::Cartesian => Some(spacing),
            BoardGridKind::Isometric => Some(3.0_f64.sqrt() * spacing / 2.0),
            _ => None,
        };
        let x = vertical_spacing
            .and_then(|s| snap_to_grid(f64::from(first.0), f64::from(last.0), s))
            .unwrap_or(first.0);
        return ((x, first.1), (x, last.1));
    }

    if grid.kind == BoardGridKind::Isometric {
        for slope in [-1.0 / 3.0_f64.sqrt(), 1.0 / 3.0_f64.sqrt()] {
            if (dy - slope * dx).abs() / (1.0 + slope * slope).sqrt() > chord * 0.12 {
                continue;
            }
            let first_intercept = f64::from(first.1) - slope * f64::from(first.0);
            let last_intercept = f64::from(last.1) - slope * f64::from(last.0);
            if let Some(intercept) = snap_to_grid(first_intercept, last_intercept, spacing) {
                let y1 = (slope * f64::from(first.0) + f64::from(intercept)).round() as i32;
                let y2 = (slope * f64::from(last.0) + f64::from(intercept)).round() as i32;
                return ((first.0, y1), (last.0, y2));
            }
        }
    }

    (first, last)
}

fn snap_to_grid(first: f64, last: f64, spacing: f64) -> Option<i32> {
    let coordinate = ((first + last) / (2.0 * spacing)).round() * spacing;
    let margin = (spacing * 0.15).clamp(4.0, 8.0);
    ((first - coordinate).abs() <= margin && (last - coordinate).abs() <= margin)
        .then_some(coordinate.round() as i32)
}

fn recognize_circle(
    points: &[(i32, i32)],
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    length: f64,
) -> Option<(i32, i32, i32)> {
    if points.len() < 8 {
        return None;
    }

    let cx = (f64::from(min_x) + f64::from(max_x)) / 2.0;
    let cy = (f64::from(min_y) + f64::from(max_y)) / 2.0;
    let radius =
        ((f64::from(max_x) - f64::from(min_x)) + (f64::from(max_y) - f64::from(min_y))) / 4.0;
    let circumference = std::f64::consts::TAU * radius;
    if !(circumference * 0.75..=circumference * 1.35).contains(&length) {
        return None;
    }

    let mut winding = 0.0_f64;
    let mut reverse = 0.0_f64;
    let mut previous_angle: Option<f64> = None;
    for &(x, y) in points {
        let dx = f64::from(x) - cx;
        let dy = f64::from(y) - cy;
        let sample_radius = dx.hypot(dy);
        if (sample_radius - radius).abs() > (radius * 0.25).max(5.0) {
            return None;
        }

        let angle = dy.atan2(dx);
        if let Some(previous) = previous_angle {
            let delta = (angle - previous + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            if delta.abs() > std::f64::consts::FRAC_PI_3 {
                return None;
            }
            winding += delta;
            reverse += delta.abs();
        }
        previous_angle = Some(angle);
    }
    if winding.abs() < std::f64::consts::TAU * 0.8
        || winding.abs() > std::f64::consts::TAU * 1.2
        || reverse > winding.abs() * 1.35
    {
        return None;
    }

    Some((cx.round() as i32, cy.round() as i32, radius.round() as i32))
}

fn distance(a: (i32, i32), b: (i32, i32)) -> f64 {
    (f64::from(a.0) - f64::from(b.0)).hypot(f64::from(a.1) - f64::from(b.1))
}
