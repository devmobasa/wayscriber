//! Fit a triangle to a closed pen path: find its three corners, then fit a
//! straight edge between each pair so rounded or overshooting corners still
//! land where the edges meet.

use crate::draw::{Color, PolygonKind, Shape};

use super::{Bounds, ClosedFit};

/// Evenly spaced samples taken around the closed stroke.
const SAMPLES: usize = 96;
/// Share of each side, at both ends, left out of its line fit so the curve
/// of a hand-drawn corner does not tilt the edge.
const CORNER_TRIM: f64 = 0.2;
/// Samples each side needs before its edge can be fitted.
const MIN_SIDE_SAMPLES: usize = 6;
/// Adjacent edges closer to parallel than this (sine of the angle) have no
/// usable corner.
const MIN_CORNER_SINE: f64 = 0.05;
/// Lowest height over the longest side; flatter strokes read as a line drawn
/// out and back.
const MIN_HEIGHT_RATIO: f64 = 0.15;
/// Edges this close to horizontal or vertical are levelled, like lines.
const AXIS_TOLERANCE: f64 = 0.12;

type Point = (f64, f64);

#[derive(Clone, Copy)]
struct Line {
    origin: Point,
    direction: Point,
}

pub(super) fn fit_triangle(
    points: &[(i32, i32)],
    bounds: Bounds,
    color: Color,
    thick: f64,
    sensitivity: u8,
) -> Option<ClosedFit> {
    let level = f64::from(sensitivity);
    let samples = resample_closed(points)?;
    let length = super::resampled_length(points) + super::distance(*points.last()?, points[0]);
    let corners = find_corners(&samples)?;
    let vertices = fit_vertices(&samples, corners)?;

    let sides = [0, 1, 2].map(|side| distance(vertices[side], vertices[(side + 1) % 3]));
    let perimeter: f64 = sides.iter().sum();
    let longest = sides.iter().copied().fold(0.0_f64, f64::max);
    let doubled_area = cross(sub(vertices[1], vertices[0]), sub(vertices[2], vertices[0])).abs();
    if doubled_area < MIN_HEIGHT_RATIO * longest * longest
        || !(perimeter * (0.85 - 0.03 * level)..=perimeter * (1.25 + 0.08 * level))
            .contains(&length)
    {
        return None;
    }

    // A drawn corner passes close to where its edges meet. Corners rounded
    // off far inside, relative to the shorter edge beside them, describe an
    // ellipse instead.
    let corner_reach = 0.06 + 0.02 * level;
    if (0..3).any(|corner| {
        let shorter_edge = sides[corner].min(sides[(corner + 2) % 3]);
        distance_to_outline(&samples, vertices[corner]) > shorter_edge * corner_reach
    }) {
        return None;
    }

    let (error, worst) = edge_deviation(&samples, corners, vertices, bounds);
    if error > 0.035 + 0.02 * level || worst > 0.13 + 0.035 * level {
        return None;
    }

    Some(ClosedFit {
        shape: Shape::Polygon {
            kind: PolygonKind::Triangle,
            points: level_edges(vertices)
                .map(|(x, y)| (x.round() as i32, y.round() as i32))
                .to_vec(),
            fill: false,
            color,
            thick,
        },
        error,
    })
}

/// Evenly spaced samples around the stroke, closed back to its start.
fn resample_closed(points: &[(i32, i32)]) -> Option<Vec<Point>> {
    let outline: Vec<Point> = points
        .iter()
        .map(|&(x, y)| (f64::from(x), f64::from(y)))
        .collect();
    let segment = |index: usize| (outline[index], outline[(index + 1) % outline.len()]);
    let total: f64 = (0..outline.len())
        .map(|index| {
            let (start, end) = segment(index);
            distance(start, end)
        })
        .sum();
    if outline.len() < 3 || total <= 0.0 {
        return None;
    }

    let step = total / SAMPLES as f64;
    let mut samples = Vec::with_capacity(SAMPLES);
    let mut walked = 0.0;
    for index in 0..outline.len() {
        let (start, end) = segment(index);
        let length = distance(start, end);
        // Repeated pointer positions add nothing to walk along.
        if length == 0.0 {
            continue;
        }
        while samples.len() < SAMPLES && step * samples.len() as f64 <= walked + length {
            let t = (step * samples.len() as f64 - walked) / length;
            samples.push((
                start.0 + (end.0 - start.0) * t,
                start.1 + (end.1 - start.1) * t,
            ));
        }
        walked += length;
    }

    (samples.len() == SAMPLES).then_some(samples)
}

/// Sample indices of the largest triangle inscribed in the stroke, in
/// drawing order.
fn find_corners(samples: &[Point]) -> Option<[usize; 3]> {
    let count = samples.len() as f64;
    let centroid = samples.iter().fold((0.0, 0.0), |sum, &point| {
        (sum.0 + point.0 / count, sum.1 + point.1 / count)
    });
    let farthest_from = |target: Point| farthest(samples, |point| distance(point, target));
    let farthest_from_line = |a: usize, b: usize| {
        farthest(samples, |point| {
            cross(sub(samples[b], samples[a]), sub(point, samples[a])).abs()
        })
    };

    // Each pass moves one corner to the sample that widens the triangle most.
    let mut a = farthest_from(centroid);
    let mut b = farthest_from(samples[a]);
    let mut c = farthest_from_line(a, b);
    for _ in 0..2 {
        a = farthest_from_line(b, c);
        b = farthest_from_line(c, a);
        c = farthest_from_line(a, b);
    }

    let mut corners = [a, b, c];
    corners.sort_unstable();
    let gaps = [
        corners[1] - corners[0],
        corners[2] - corners[1],
        samples.len() - corners[2] + corners[0],
    ];
    gaps.iter()
        .all(|&gap| gap >= MIN_SIDE_SAMPLES)
        .then_some(corners)
}

fn farthest(samples: &[Point], measure: impl Fn(Point) -> f64) -> usize {
    samples
        .iter()
        .map(|&point| measure(point))
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(0, |(index, _)| index)
}

/// Corners where the fitted edges meet. Corner `k` joins the side ending
/// there with the side starting there.
fn fit_vertices(samples: &[Point], corners: [usize; 3]) -> Option<[Point; 3]> {
    let [first, second, third] = [0, 1, 2].map(|side| {
        let (start, end) = side_span(samples, corners, side);
        let trim = ((end - start) as f64 * CORNER_TRIM) as usize;
        fit_line((start + trim..=end - trim).map(|index| samples[index % samples.len()]))
    });
    let edges = [first?, second?, third?];

    Some([
        intersect(edges[2], edges[0])?,
        intersect(edges[0], edges[1])?,
        intersect(edges[1], edges[2])?,
    ])
}

/// First and last sample index of one side; the last side wraps past the
/// stroke's start.
fn side_span(samples: &[Point], corners: [usize; 3], side: usize) -> (usize, usize) {
    let start = corners[side];
    let end = if side == 2 {
        corners[0] + samples.len()
    } else {
        corners[side + 1]
    };
    (start, end)
}

/// Total least-squares line through the points.
fn fit_line(points: impl Iterator<Item = Point> + Clone) -> Option<Line> {
    let count = points.clone().count();
    if count < 2 {
        return None;
    }

    let count = count as f64;
    let origin = points.clone().fold((0.0, 0.0), |sum, point| {
        (sum.0 + point.0 / count, sum.1 + point.1 / count)
    });
    let (xx, yy, xy) = points.fold((0.0, 0.0, 0.0), |(xx, yy, xy), point| {
        let dx = point.0 - origin.0;
        let dy = point.1 - origin.1;
        (xx + dx * dx, yy + dy * dy, xy + dx * dy)
    });
    let angle = 0.5 * (2.0 * xy).atan2(xx - yy);

    Some(Line {
        origin,
        direction: (angle.cos(), angle.sin()),
    })
}

fn intersect(first: Line, second: Line) -> Option<Point> {
    let denominator = cross(first.direction, second.direction);
    if denominator.abs() < MIN_CORNER_SINE {
        return None;
    }

    let t = cross(sub(second.origin, first.origin), second.direction) / denominator;
    Some((
        first.origin.0 + first.direction.0 * t,
        first.origin.1 + first.direction.1 * t,
    ))
}

/// Mean and worst distance from each sample to the fitted edge of its own
/// side, relative to the stroke's short extent.
fn edge_deviation(
    samples: &[Point],
    corners: [usize; 3],
    vertices: [Point; 3],
    bounds: Bounds,
) -> (f64, f64) {
    let scale = bounds.width.min(bounds.height);
    let mut total = 0.0;
    let mut worst = 0.0_f64;
    for side in 0..3 {
        let (start, end) = side_span(samples, corners, side);
        let (a, b) = (vertices[side], vertices[(side + 1) % 3]);
        for index in start..end {
            let deviation = distance_to_segment(samples[index % samples.len()], a, b) / scale;
            total += deviation;
            worst = worst.max(deviation);
        }
    }
    (total / samples.len() as f64, worst)
}

fn distance_to_outline(samples: &[Point], target: Point) -> f64 {
    (0..samples.len())
        .map(|index| {
            distance_to_segment(target, samples[index], samples[(index + 1) % samples.len()])
        })
        .fold(f64::INFINITY, f64::min)
}

/// Level the side nearest horizontal and the side nearest vertical when
/// they are within tolerance. One moves only y and the other only x, so
/// neither undoes the other.
fn level_edges(mut vertices: [Point; 3]) -> [Point; 3] {
    let flattest = |across: fn(Point) -> f64| {
        (0..3)
            .map(|side| {
                let delta = sub(vertices[(side + 1) % 3], vertices[side]);
                (side, across(delta).abs() / delta.0.hypot(delta.1))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|&(_, tilt)| tilt <= AXIS_TOLERANCE)
            .map(|(side, _)| (side, (side + 1) % 3))
    };
    let horizontal = flattest(|delta| delta.1);
    let vertical = flattest(|delta| delta.0);

    if let Some((a, b)) = horizontal {
        let y = (vertices[a].1 + vertices[b].1) / 2.0;
        vertices[a].1 = y;
        vertices[b].1 = y;
    }
    if let Some((a, b)) = vertical {
        let x = (vertices[a].0 + vertices[b].0) / 2.0;
        vertices[a].0 = x;
        vertices[b].0 = x;
    }
    vertices
}

fn distance_to_segment(point: Point, a: Point, b: Point) -> f64 {
    let edge = sub(b, a);
    let length_squared = edge.0 * edge.0 + edge.1 * edge.1;
    let t = if length_squared > 0.0 {
        (((point.0 - a.0) * edge.0 + (point.1 - a.1) * edge.1) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    distance(point, (a.0 + edge.0 * t, a.1 + edge.1 * t))
}

fn distance(a: Point, b: Point) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn sub(a: Point, b: Point) -> Point {
    (a.0 - b.0, a.1 - b.1)
}

fn cross(a: Point, b: Point) -> f64 {
    a.0 * b.1 - a.1 * b.0
}
