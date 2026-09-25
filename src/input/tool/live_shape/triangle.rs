//! Fit a triangle to a closed pen path: find its three corners, then fit a
//! straight edge between each pair so rounded or overshooting corners still
//! land where the edges meet.

use crate::draw::{Color, PolygonKind, Shape};

use super::outline::{self, Point, SidedFit, cross, distance, farthest, sub};
use super::{Bounds, ClosedFit};

/// Lowest height over the longest side; flatter strokes read as a line drawn
/// out and back.
const MIN_HEIGHT_RATIO: f64 = 0.15;
/// Edges this close to horizontal or vertical are levelled, like lines.
const AXIS_TOLERANCE: f64 = 0.12;

pub(super) fn fit_triangle(
    points: &[(i32, i32)],
    bounds: Bounds,
    color: Color,
    thick: f64,
    sensitivity: u8,
) -> Option<ClosedFit> {
    let level = f64::from(sensitivity);
    let samples = outline::resample_closed(points)?;
    let length = outline::closed_length(points)?;
    let corners = find_corners(&samples);
    let fit = SidedFit::new(samples, corners.to_vec())?;
    let [a, b, c] = fit.vertices[..] else {
        return None;
    };

    let sides = fit.side_lengths();
    let perimeter: f64 = sides.iter().sum();
    let longest = sides.iter().copied().fold(0.0_f64, f64::max);
    let doubled_area = cross(sub(b, a), sub(c, a)).abs();
    if doubled_area < MIN_HEIGHT_RATIO * longest * longest
        || !(perimeter * (0.85 - 0.03 * level)..=perimeter * (1.25 + 0.08 * level))
            .contains(&length)
        || !fit.corners_drawn(0.06 + 0.02 * level)
    {
        return None;
    }

    let (error, worst) = fit.deviation(bounds.width.min(bounds.height));
    if error > 0.035 + 0.02 * level || worst > 0.13 + 0.035 * level {
        return None;
    }

    Some(ClosedFit {
        shape: Shape::Polygon {
            kind: PolygonKind::Triangle,
            points: level_edges([a, b, c])
                .map(|(x, y)| (x.round() as i32, y.round() as i32))
                .to_vec(),
            fill: false,
            color,
            thick,
        },
        error,
    })
}

/// Sample indices of the largest triangle inscribed in the stroke.
fn find_corners(samples: &[Point]) -> [usize; 3] {
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
    [a, b, c]
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
