//! Release-time smoothing for freehand paths.
//!
//! A pointer path carries the shake of the hand that drew it. Smoothing removes
//! that shake without changing what the stroke is.
//!
//! ## Why this runs on release rather than during the drag
//!
//! Smoothing a point needs the points on both sides of it, so a live smoother
//! cannot draw the newest sample until the next one arrives. The line then trails
//! the cursor by a sample or two. On a projector that lag is visible to the room,
//! which is the opposite of what an annotation tool is for.
//!
//! Running on release instead keeps the live stroke exactly on the pointer, and
//! pays for the smoothing once, on a path that is already complete.
//!
//! ## The filter
//!
//! One pass is the binomial kernel `[1/4, 1/2, 1/4]` over interior points, with
//! both endpoints pinned. `level` is how many passes run, so the levels form a
//! progressively wider filter rather than a set of unrelated behaviors, and
//! level 0 is the identity.
//!
//! Endpoints are pinned because a stroke has to start and stop where the user
//! started and stopped. A filter that moved them would pull an underline off the
//! word it began under.

/// Highest smoothing level. Past roughly this many passes a stroke stops
/// following its own corners and reads as a different line from the one drawn.
pub const MAX_PEN_SMOOTHING: u8 = 6;

/// Fewest points a path needs before smoothing can do anything. With two points
/// there is no interior to smooth, and both are pinned.
const MIN_SMOOTHABLE_POINTS: usize = 3;

/// The centre weight of one pass. The neighbours share the remainder equally.
const CENTER_WEIGHT: f64 = 0.5;

/// Clamp a configured or stepped level into range.
pub fn clamp_pen_smoothing(level: u8) -> u8 {
    level.min(MAX_PEN_SMOOTHING)
}

/// Smooth an `(x, y)` path. Level 0 returns the path unchanged.
pub fn smooth_path(points: &[(i32, i32)], level: u8) -> Vec<(i32, i32)> {
    let Some(smoothed) = smooth_points(points, level, |&(x, y)| (f64::from(x), f64::from(y)))
    else {
        return points.to_vec();
    };
    smoothed
        .into_iter()
        .map(|(x, y)| (round_to_i32(x), round_to_i32(y)))
        .collect()
}

/// Smooth the positions of a pressure path, leaving every thickness alone.
///
/// The path of a pressure stroke shakes exactly like any other, so it is
/// smoothed like any other. The *pressure values* are not: they came from the
/// tablet rather than from the hand's aim, and averaging them would erase real
/// detail while fixing nothing. Each smoothed position keeps the thickness that
/// was sampled with it.
pub fn smooth_pressure_path(points: &[(i32, i32, f32)], level: u8) -> Vec<(i32, i32, f32)> {
    let Some(smoothed) = smooth_points(points, level, |&(x, y, _)| (f64::from(x), f64::from(y)))
    else {
        return points.to_vec();
    };
    points
        .iter()
        .zip(smoothed)
        .map(|(&(_, _, thickness), (x, y))| (round_to_i32(x), round_to_i32(y), thickness))
        .collect()
}

/// The shared filter. `None` means the caller should keep its input as it is.
fn smooth_points<T>(
    points: &[T],
    level: u8,
    position: impl Fn(&T) -> (f64, f64),
) -> Option<Vec<(f64, f64)>> {
    let level = clamp_pen_smoothing(level);
    if level == 0 || points.len() < MIN_SMOOTHABLE_POINTS {
        return None;
    }

    // One f64 buffer for the whole run: rounding to integers between passes
    // would quantize away exactly the sub-pixel corrections being applied.
    let mut current: Vec<(f64, f64)> = points.iter().map(&position).collect();
    if current
        .iter()
        .any(|(x, y)| !x.is_finite() || !y.is_finite())
    {
        return None;
    }
    let mut next = current.clone();
    let neighbor_weight = (1.0 - CENTER_WEIGHT) / 2.0;

    for _ in 0..level {
        for index in 1..current.len() - 1 {
            let previous = current[index - 1];
            let point = current[index];
            let following = current[index + 1];
            next[index] = (
                previous.0 * neighbor_weight
                    + point.0 * CENTER_WEIGHT
                    + following.0 * neighbor_weight,
                previous.1 * neighbor_weight
                    + point.1 * CENTER_WEIGHT
                    + following.1 * neighbor_weight,
            );
        }
        std::mem::swap(&mut current, &mut next);
    }
    Some(current)
}

fn round_to_i32(value: f64) -> i32 {
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straight run with one sample knocked sideways, as a shaky hand makes.
    fn spike() -> Vec<(i32, i32)> {
        vec![(0, 0), (10, 0), (20, 12), (30, 0), (40, 0)]
    }

    #[test]
    fn level_zero_returns_the_exact_path_that_was_drawn() {
        let points = spike();

        assert_eq!(smooth_path(&points, 0), points);
    }

    #[test]
    fn smoothing_pulls_a_spike_back_toward_its_neighbours() {
        let points = spike();

        let smoothed = smooth_path(&points, 3);

        assert_eq!(smoothed.len(), points.len());
        assert!(
            smoothed[2].1 < points[2].1,
            "the spike at index 2 must come down, not stay at 12"
        );
        assert!(smoothed[2].1 > 0, "and not be flattened away entirely");
    }

    #[test]
    fn a_higher_level_smooths_more_than_a_lower_one() {
        let points = spike();

        let light = smooth_path(&points, 1)[2].1;
        let heavy = smooth_path(&points, 6)[2].1;

        assert!(
            heavy < light,
            "level 6 must pull the spike further than level 1"
        );
    }

    #[test]
    fn both_endpoints_never_move() {
        let points = spike();

        for level in 0..=MAX_PEN_SMOOTHING {
            let smoothed = smooth_path(&points, level);
            assert_eq!(
                smoothed.first(),
                points.first(),
                "level {level} moved the start of the stroke"
            );
            assert_eq!(
                smoothed.last(),
                points.last(),
                "level {level} moved the end of the stroke"
            );
        }
    }

    #[test]
    fn a_straight_line_survives_every_level_unchanged() {
        let points = vec![(0, 0), (10, 0), (20, 0), (30, 0)];

        for level in 0..=MAX_PEN_SMOOTHING {
            assert_eq!(smooth_path(&points, level), points, "level {level}");
        }
    }

    #[test]
    fn paths_too_short_to_have_an_interior_are_returned_as_they_are() {
        for points in [vec![], vec![(5, 5)], vec![(5, 5), (9, 9)]] {
            assert_eq!(smooth_path(&points, 6), points);
        }
    }

    #[test]
    fn the_level_is_clamped_rather_than_trusted() {
        let points = spike();

        assert_eq!(clamp_pen_smoothing(200), MAX_PEN_SMOOTHING);
        assert_eq!(
            smooth_path(&points, 200),
            smooth_path(&points, MAX_PEN_SMOOTHING)
        );
    }

    #[test]
    fn a_pressure_path_keeps_every_thickness_while_its_positions_move() {
        let points = vec![
            (0, 0, 1.0f32),
            (10, 0, 4.0),
            (20, 12, 9.0),
            (30, 0, 4.0),
            (40, 0, 1.0),
        ];

        let smoothed = smooth_pressure_path(&points, 3);

        assert_eq!(
            smoothed.iter().map(|&(_, _, t)| t).collect::<Vec<_>>(),
            points.iter().map(|&(_, _, t)| t).collect::<Vec<_>>(),
            "tablet pressure is real detail and is not the hand's shake"
        );
        assert!(smoothed[2].1 < points[2].1);
        assert_eq!(smoothed.first().map(|&(x, y, _)| (x, y)), Some((0, 0)));
    }

    #[test]
    fn smoothing_never_changes_how_many_points_a_stroke_has() {
        let points = spike();

        for level in 0..=MAX_PEN_SMOOTHING {
            assert_eq!(smooth_path(&points, level).len(), points.len());
        }
    }
}
