//! Recognize lines and closed shapes from a pen path. Ambiguous ink stays ink.

use std::cell::RefCell;

use crate::domain::{BoardGrid, BoardGridKind};
use crate::draw::{Color, Shape};

mod grid;
mod outline;
mod rough_rectangle;
#[cfg(test)]
mod tests;
mod triangle;

/// Remembers the last recognition of the stroke being drawn, so the preview,
/// its damage, and the shape readout share one recognition per pointer move
/// instead of each running it over the whole stroke.
#[derive(Debug, Clone, Default)]
pub(crate) struct LiveShapeMemo {
    last: RefCell<Option<(MemoKey, Option<Shape>)>>,
    #[cfg(test)]
    runs: std::cell::Cell<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MemoKey {
    len: usize,
    first: (i32, i32),
    last: (i32, i32),
    color: Color,
    thick: f64,
    fill: bool,
    grid: BoardGrid,
    sensitivity: u8,
}

impl LiveShapeMemo {
    /// Recognize `points`, reusing the last answer while neither the stroke
    /// nor the settings have changed. A stroke only grows at its end while
    /// it is drawn, and the memo is reset when the next one starts, so the
    /// point count and both ends identify it.
    pub(crate) fn recognize(
        &self,
        points: &[(i32, i32)],
        color: Color,
        thick: f64,
        fill: bool,
        grid: BoardGrid,
        sensitivity: u8,
    ) -> Option<Shape> {
        let key = MemoKey {
            len: points.len(),
            first: *points.first()?,
            last: *points.last()?,
            color,
            thick,
            fill,
            grid,
            sensitivity,
        };
        if let Some((cached, shape)) = &*self.last.borrow()
            && *cached == key
        {
            return shape.clone();
        }

        #[cfg(test)]
        self.runs.set(self.runs.get() + 1);
        let shape = recognize(points, color, thick, fill, grid, sensitivity);
        *self.last.borrow_mut() = Some((key, shape.clone()));
        shape
    }

    /// How many recognitions actually ran.
    #[cfg(test)]
    pub(crate) fn runs(&self) -> usize {
        self.runs.get()
    }
}

pub(super) fn recognize(
    points: &[(i32, i32)],
    color: Color,
    thick: f64,
    fill: bool,
    grid: BoardGrid,
    sensitivity: u8,
) -> Option<Shape> {
    let first = *points.first()?;
    let last = *points.last()?;
    let bounds = Bounds::for_points(points);
    let chord = distance(first, last);
    let length: f64 = points
        .windows(2)
        .map(|pair| distance(pair[0], pair[1]))
        .sum();
    let sensitivity = sensitivity.min(crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY);

    if points.len() >= 8
        && bounds.width >= 24.0
        && bounds.height >= 24.0
        && chord <= bounds.diameter() * (0.12 + 0.05 * f64::from(sensitivity))
        && let Some(mut shape) = recognize_closed(points, bounds, length, color, thick, sensitivity)
    {
        // Closed shapes follow the Fill toggle, like the dedicated shape tools.
        if let Shape::Ellipse { fill: filled, .. }
        | Shape::Rect { fill: filled, .. }
        | Shape::Polygon { fill: filled, .. } = &mut shape
        {
            *filled = fill;
        }
        return Some(grid::snap_closed_shape(shape, grid));
    }

    if chord < 16.0 || resampled_length(points) > chord * (1.08 + 0.04 * f64::from(sensitivity)) {
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
    if deviation > (chord * (0.04 + 0.02 * f64::from(sensitivity))).max(4.0) {
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

#[derive(Clone, Copy)]
struct Bounds {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    width: f64,
    height: f64,
}

impl Bounds {
    fn for_points(points: &[(i32, i32)]) -> Self {
        let (min_x, max_x, min_y, max_y) = points.iter().fold(
            (i32::MAX, i32::MIN, i32::MAX, i32::MIN),
            |(min_x, max_x, min_y, max_y), &(x, y)| {
                (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
            },
        );
        Self {
            min_x,
            max_x,
            min_y,
            max_y,
            width: f64::from(max_x) - f64::from(min_x),
            height: f64::from(max_y) - f64::from(min_y),
        }
    }

    fn diameter(self) -> f64 {
        self.width.max(self.height)
    }

    fn center(self) -> (f64, f64) {
        (
            (f64::from(self.min_x) + f64::from(self.max_x)) / 2.0,
            (f64::from(self.min_y) + f64::from(self.max_y)) / 2.0,
        )
    }
}

struct ClosedFit {
    shape: Shape,
    error: f64,
}

fn recognize_closed(
    points: &[(i32, i32)],
    bounds: Bounds,
    length: f64,
    color: Color,
    thick: f64,
    sensitivity: u8,
) -> Option<Shape> {
    // Winding is measured around the box center, which can fall on a
    // triangle's edge, so the triangle fit checks its own edges instead.
    let (ellipse, rectangle) = if winds_once(points, bounds) {
        (
            fit_ellipse(points, bounds, length, color, thick, sensitivity),
            fit_rectangle(points, bounds, length, color, thick, sensitivity),
        )
    } else {
        (None, None)
    };
    // A rectangle the box fit rejects may still be one whose sides lean.
    let rectangle = rectangle.or_else(|| {
        rough_rectangle::fit_rough_rectangle(points, bounds, color, thick, sensitivity)
    });
    let triangle = triangle::fit_triangle(points, bounds, color, thick, sensitivity);

    // Ties keep the earlier candidate, so an ellipse wins an exact tie.
    [ellipse, rectangle, triangle]
        .into_iter()
        .flatten()
        .min_by(|a, b| a.error.total_cmp(&b.error))
        .map(|fit| fit.shape)
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
    let margin = grid_snap_margin(spacing);
    ((first - coordinate).abs() <= margin && (last - coordinate).abs() <= margin)
        .then_some(coordinate.round() as i32)
}

/// How far ink may sit from board paper and still snap to it.
fn grid_snap_margin(spacing: f64) -> f64 {
    (spacing * 0.15).clamp(4.0, 8.0)
}

fn winds_once(points: &[(i32, i32)], bounds: Bounds) -> bool {
    let (cx, cy) = bounds.center();
    let rx = bounds.width / 2.0;
    let ry = bounds.height / 2.0;
    let mut winding = 0.0_f64;
    let mut travel = 0.0_f64;
    let mut previous_angle: Option<f64> = None;

    for &(x, y) in points {
        let angle = ((f64::from(y) - cy) / ry).atan2((f64::from(x) - cx) / rx);
        if let Some(previous) = previous_angle {
            let delta = (angle - previous + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            if delta.abs() > std::f64::consts::FRAC_PI_2 + 0.1 {
                return false;
            }
            winding += delta;
            travel += delta.abs();
        }
        previous_angle = Some(angle);
    }

    (std::f64::consts::TAU * 0.8..=std::f64::consts::TAU * 1.2).contains(&winding.abs())
        && travel <= winding.abs() * 1.5
}

fn fit_ellipse(
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
    for &(x, y) in points {
        let radius = ((f64::from(x) - cx) / rx).hypot((f64::from(y) - cy) / ry);
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

fn fit_rectangle(
    points: &[(i32, i32)],
    bounds: Bounds,
    length: f64,
    color: Color,
    thick: f64,
    sensitivity: u8,
) -> Option<ClosedFit> {
    let level = f64::from(sensitivity);
    let short_side = bounds.width.min(bounds.height);
    let perimeter = 2.0 * (bounds.width + bounds.height);
    if bounds.diameter() / short_side > 5.0
        || !(perimeter * (0.8 - 0.04 * level)..=perimeter * (1.25 + 0.08 * level)).contains(&length)
    {
        return None;
    }

    let mut error = 0.0_f64;
    let mut worst = 0.0_f64;
    for &(x, y) in points {
        let x = f64::from(x);
        let y = f64::from(y);
        let distance = (x - f64::from(bounds.min_x))
            .abs()
            .min((x - f64::from(bounds.max_x)).abs())
            .min((y - f64::from(bounds.min_y)).abs())
            .min((y - f64::from(bounds.max_y)).abs())
            / short_side;
        error += distance;
        worst = worst.max(distance);
    }
    let error = error / points.len() as f64;
    if error > 0.035 + 0.02 * level || worst > 0.13 + 0.035 * level {
        return None;
    }

    for corner in [
        (bounds.min_x, bounds.min_y),
        (bounds.max_x, bounds.min_y),
        (bounds.max_x, bounds.max_y),
        (bounds.min_x, bounds.max_y),
    ] {
        let nearest = points
            .iter()
            .map(|&point| distance(point, corner))
            .fold(f64::INFINITY, f64::min);
        if nearest > short_side * (0.12 + 0.04 * level) {
            return None;
        }
    }

    Some(ClosedFit {
        shape: Shape::Rect {
            x: bounds.min_x,
            y: bounds.min_y,
            w: bounds.width.round() as i32,
            h: bounds.height.round() as i32,
            fill: false,
            color,
            thick,
        },
        error,
    })
}

fn distance(a: (i32, i32), b: (i32, i32)) -> f64 {
    (f64::from(a.0) - f64::from(b.0)).hypot(f64::from(a.1) - f64::from(b.1))
}

fn resampled_length(points: &[(i32, i32)]) -> f64 {
    let Some(&first) = points.first() else {
        return 0.0;
    };
    let mut anchor = first;
    let mut length = 0.0;

    // Small pixel steps exaggerate diagonal length after integer rounding.
    for &point in points.iter().skip(1) {
        let segment = distance(anchor, point);
        if segment >= 4.0 {
            length += segment;
            anchor = point;
        }
    }

    length + distance(anchor, *points.last().unwrap())
}
