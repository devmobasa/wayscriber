//! Snap recognized closed shapes to the board paper under them: rectangle
//! and ellipse edges to Cartesian lines, triangle corners to Cartesian lines
//! or isometric lattice points. Lines snap in `align_line`.

use crate::domain::{BoardGrid, BoardGridKind};
use crate::draw::Shape;

use super::grid_snap_margin;

pub(super) fn snap_closed_shape(shape: Shape, grid: BoardGrid) -> Shape {
    let spacing = f64::from(grid.spacing());
    match (grid.kind, shape) {
        (
            BoardGridKind::Cartesian,
            Shape::Rect {
                x,
                y,
                w,
                h,
                fill,
                color,
                thick,
            },
        ) => {
            let (x, w) = snap_span(x, w, spacing);
            let (y, h) = snap_span(y, h, spacing);
            Shape::Rect {
                x,
                y,
                w,
                h,
                fill,
                color,
                thick,
            }
        }
        (
            BoardGridKind::Cartesian,
            Shape::Ellipse {
                cx,
                cy,
                rx,
                ry,
                fill,
                color,
                thick,
            },
        ) => {
            let (left, width) = snap_span(cx - rx, 2 * rx, spacing);
            let (top, height) = snap_span(cy - ry, 2 * ry, spacing);
            Shape::Ellipse {
                cx: left + width / 2,
                cy: top + height / 2,
                rx: width / 2,
                ry: height / 2,
                fill,
                color,
                thick,
            }
        }
        (
            paper,
            Shape::Polygon {
                kind,
                points,
                fill,
                color,
                thick,
            },
        ) if points.len() == 3 => {
            let snap = |point: (i32, i32)| match paper {
                BoardGridKind::Cartesian => (
                    snap_coordinate(point.0, spacing),
                    snap_coordinate(point.1, spacing),
                ),
                BoardGridKind::Isometric | BoardGridKind::IsometricDots => {
                    snap_to_lattice(point, spacing)
                }
                BoardGridKind::None => point,
            };
            let snapped: Vec<_> = points.iter().map(|&point| snap(point)).collect();
            // Corners pulled onto one lattice point would flatten the triangle.
            let points = if doubled_area(&snapped) * 2 >= doubled_area(&points) {
                snapped
            } else {
                points
            };
            Shape::Polygon {
                kind,
                points,
                fill,
                color,
                thick,
            }
        }
        (_, shape) => shape,
    }
}

/// Snap both ends of one axis of a box, unless that would collapse it.
fn snap_span(start: i32, length: i32, spacing: f64) -> (i32, i32) {
    let low = snap_coordinate(start, spacing);
    let high = snap_coordinate(start + length, spacing);
    if high > low {
        (low, high - low)
    } else {
        (start, length)
    }
}

fn snap_coordinate(value: i32, spacing: f64) -> i32 {
    let value = f64::from(value);
    let line = (value / spacing).round() * spacing;
    if (value - line).abs() <= grid_snap_margin(spacing) {
        line.round() as i32
    } else {
        value as i32
    }
}

/// The nearest isometric lattice point, if it is close. Columns sit
/// `√3/2 · spacing` apart and odd columns are shifted down half a spacing,
/// matching the painted lines and dots.
fn snap_to_lattice(point: (i32, i32), spacing: f64) -> (i32, i32) {
    let (x, y) = (f64::from(point.0), f64::from(point.1));
    let column = 3.0_f64.sqrt() * spacing / 2.0;
    let nearest_column = (x / column).round() as i64;

    let (lattice, distance) = (nearest_column - 1..=nearest_column + 1)
        .map(|index| {
            let shift = index.rem_euclid(2) as f64 * spacing / 2.0;
            let lattice = (
                index as f64 * column,
                ((y - shift) / spacing).round() * spacing + shift,
            );
            (lattice, (lattice.0 - x).hypot(lattice.1 - y))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("three candidate columns");
    if distance <= grid_snap_margin(spacing) {
        (lattice.0.round() as i32, lattice.1.round() as i32)
    } else {
        point
    }
}

fn doubled_area(points: &[(i32, i32)]) -> i64 {
    let [a, b, c] = [points[0], points[1], points[2]].map(|(x, y)| (i64::from(x), i64::from(y)));
    ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs()
}
