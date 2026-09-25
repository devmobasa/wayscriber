//! Shared geometry for straight-sided Shape Pen fits: resample a closed pen
//! path evenly, then fit a straight line through each side between known
//! corners, so rounded or overshooting corners still land where the sides
//! meet.

pub(super) type Point = (f64, f64);

/// Evenly spaced samples taken around the closed stroke.
const SAMPLES: usize = 96;
/// Share of each side, at both ends, left out of its line fit so the curve
/// of a hand-drawn corner does not tilt the side.
const CORNER_TRIM: f64 = 0.2;
/// Samples each side needs before its line can be fitted.
const MIN_SIDE_SAMPLES: usize = 6;
/// Adjacent sides closer to parallel than this (sine of the angle) have no
/// usable corner.
const MIN_CORNER_SINE: f64 = 0.05;

#[derive(Clone, Copy)]
pub(super) struct Line {
    pub(super) origin: Point,
    pub(super) direction: Point,
}

/// A closed stroke fitted with one straight line per side.
pub(super) struct SidedFit {
    samples: Vec<Point>,
    /// Sample indices where sides meet, in drawing order.
    corners: Vec<usize>,
    /// Side `k` runs from corner `k` to corner `k + 1`.
    pub(super) lines: Vec<Line>,
    /// Vertex `k` is where side `k - 1` meets side `k`.
    pub(super) vertices: Vec<Point>,
}

impl SidedFit {
    pub(super) fn new(samples: Vec<Point>, mut corners: Vec<usize>) -> Option<Self> {
        corners.sort_unstable();
        corners.dedup();
        let sides = corners.len();
        if sides < 3
            || (0..sides).any(|side| {
                let (start, end) = side_span(samples.len(), &corners, side);
                end - start < MIN_SIDE_SAMPLES
            })
        {
            return None;
        }

        let lines = (0..sides)
            .map(|side| {
                let (start, end) = side_span(samples.len(), &corners, side);
                let trim = ((end - start) as f64 * CORNER_TRIM) as usize;
                fit_line((start + trim..=end - trim).map(|index| samples[index % samples.len()]))
            })
            .collect::<Option<Vec<_>>>()?;
        let vertices = (0..sides)
            .map(|corner| intersect(lines[(corner + sides - 1) % sides], lines[corner]))
            .collect::<Option<Vec<_>>>()?;

        Some(Self {
            samples,
            corners,
            lines,
            vertices,
        })
    }

    /// Length of side `k`, between vertex `k` and vertex `k + 1`.
    pub(super) fn side_lengths(&self) -> Vec<f64> {
        let count = self.vertices.len();
        (0..count)
            .map(|side| distance(self.vertices[side], self.vertices[(side + 1) % count]))
            .collect()
    }

    /// Whether the stroke passes close to every fitted vertex, within `reach`
    /// of the shorter side beside it. Corners rounded off far inside describe
    /// an ellipse instead.
    pub(super) fn corners_drawn(&self, reach: f64) -> bool {
        let sides = self.side_lengths();
        let count = sides.len();
        (0..count).all(|corner| {
            let shorter_side = sides[corner].min(sides[(corner + count - 1) % count]);
            distance_to_outline(&self.samples, self.vertices[corner]) <= shorter_side * reach
        })
    }

    /// Mean and worst distance from each sample to the fitted segment of its
    /// own side, relative to `scale`.
    pub(super) fn deviation(&self, scale: f64) -> (f64, f64) {
        let count = self.vertices.len();
        let mut total = 0.0;
        let mut worst = 0.0_f64;
        for side in 0..count {
            let (start, end) = side_span(self.samples.len(), &self.corners, side);
            let (a, b) = (self.vertices[side], self.vertices[(side + 1) % count]);
            for index in start..end {
                let sample = self.samples[index % self.samples.len()];
                let deviation = distance_to_segment(sample, a, b) / scale;
                total += deviation;
                worst = worst.max(deviation);
            }
        }
        (total / self.samples.len() as f64, worst)
    }
}

/// Evenly spaced samples around the stroke, closed back to its start.
pub(super) fn resample_closed(points: &[(i32, i32)]) -> Option<Vec<Point>> {
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

/// The stroke's length including the gap back to its start, free of the
/// integer pixel staircases that inflate dense diagonal input.
pub(super) fn closed_length(points: &[(i32, i32)]) -> Option<f64> {
    Some(super::resampled_length(points) + super::distance(*points.last()?, *points.first()?))
}

/// Index of the sample that scores highest.
pub(super) fn farthest(samples: &[Point], measure: impl Fn(Point) -> f64) -> usize {
    samples
        .iter()
        .map(|&point| measure(point))
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(0, |(index, _)| index)
}

/// First and last sample index of one side; the last side wraps past the
/// stroke's start.
fn side_span(sample_count: usize, corners: &[usize], side: usize) -> (usize, usize) {
    let start = corners[side];
    let end = match corners.get(side + 1) {
        Some(&next) => next,
        None => corners[0] + sample_count,
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

fn distance_to_outline(samples: &[Point], target: Point) -> f64 {
    (0..samples.len())
        .map(|index| {
            distance_to_segment(target, samples[index], samples[(index + 1) % samples.len()])
        })
        .fold(f64::INFINITY, f64::min)
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

pub(super) fn distance(a: Point, b: Point) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

pub(super) fn sub(a: Point, b: Point) -> Point {
    (a.0 - b.0, a.1 - b.1)
}

pub(super) fn cross(a: Point, b: Point) -> f64 {
    a.0 * b.1 - a.1 * b.0
}
