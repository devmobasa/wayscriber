//! Recognize rectangles drawn quickly by hand, whose sides lean or bow too
//! far from the stroke's bounding box for the box fit: find the four corners,
//! fit a line through each side, and square the sides up.

use crate::draw::{Color, Shape};

use super::outline::{self, SidedFit, farthest};
use super::{Bounds, ClosedFit};

pub(super) fn fit_rough_rectangle(
    points: &[(i32, i32)],
    bounds: Bounds,
    color: Color,
    thick: f64,
    sensitivity: u8,
) -> Option<ClosedFit> {
    let level = f64::from(sensitivity);
    let samples = outline::resample_closed(points)?;
    let length = outline::closed_length(points)?;
    // The sample farthest toward each corner of the box, measured diagonally,
    // is the drawn corner even when the sides beside it lean.
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(dx, dy)| farthest(&samples, |point| dx * point.0 + dy * point.1));
    let fit = SidedFit::new(samples, corners.to_vec())?;
    if fit.lines.len() != 4 {
        return None;
    }

    // Sides must alternate between nearly horizontal and nearly vertical,
    // each leaning at most this far from its axis.
    let max_lean = (12.0 + 4.0 * level).to_radians().tan();
    let horizontal: Vec<bool> = fit
        .lines
        .iter()
        .map(|line| line.direction.1.abs() <= line.direction.0.abs())
        .collect();
    if (0..4).any(|side| horizontal[side] == horizontal[(side + 1) % 4])
        || fit.lines.iter().any(|line| {
            let (along, across) = (line.direction.0.abs(), line.direction.1.abs());
            along.min(across) > along.max(across) * max_lean
        })
    {
        return None;
    }

    let perimeter: f64 = fit.side_lengths().iter().sum();
    if !(perimeter * (0.85 - 0.03 * level)..=perimeter * (1.25 + 0.08 * level)).contains(&length)
        || !fit.corners_drawn(0.06 + 0.02 * level)
    {
        return None;
    }

    let (error, worst) = fit.deviation(bounds.width.min(bounds.height));
    if error > 0.035 + 0.02 * level || worst > 0.13 + 0.035 * level {
        return None;
    }

    // Each side becomes the axis line through its average position.
    let (mut xs, mut ys) = (Vec::with_capacity(2), Vec::with_capacity(2));
    for (line, &is_horizontal) in fit.lines.iter().zip(&horizontal) {
        if is_horizontal {
            ys.push(line.origin.1);
        } else {
            xs.push(line.origin.0);
        }
    }
    let (left, right) = (xs[0].min(xs[1]), xs[0].max(xs[1]));
    let (top, bottom) = (ys[0].min(ys[1]), ys[0].max(ys[1]));

    Some(ClosedFit {
        shape: Shape::Rect {
            x: left.round() as i32,
            y: top.round() as i32,
            w: (right - left).round() as i32,
            h: (bottom - top).round() as i32,
            fill: false,
            color,
            thick,
        },
        error,
    })
}
