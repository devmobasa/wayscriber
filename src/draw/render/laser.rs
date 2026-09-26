//! Glowing laser-pointer ink.
//!
//! Laser ink is never a [`crate::draw::Shape`]: it is presenter feedback the
//! input layer keeps for a moment and then drops. This module only knows how
//! to paint one stroke and how far that paint reaches, so the live stroke and
//! the finished, fading ink look identical and damage the same pixels.

use crate::draw::Color;
use crate::draw::shape::bounding_box_for_points;
use crate::util::Rect;

/// Outer halo width as a multiple of the core width. It sets the stroke's
/// reach, so every damage rect is computed from it.
const OUTER_GLOW_SCALE: f64 = 3.2;
const INNER_GLOW_SCALE: f64 = 1.9;
const HOT_CORE_SCALE: f64 = 0.4;

const OUTER_GLOW_ALPHA: f64 = 0.16;
const INNER_GLOW_ALPHA: f64 = 0.32;
const HOT_CORE_ALPHA: f64 = 0.85;

/// How far the hot centre line is lifted toward white, so the core reads as
/// light rather than paint on any background.
const HOT_CORE_WHITE_MIX: f64 = 0.7;

/// Points per damage rect. A long diagonal sweep then damages its own path
/// instead of the box around it, which would be most of the screen.
const DAMAGE_CHUNK_POINTS: usize = 24;

/// Color and core width of laser ink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LaserStyle {
    pub(crate) color: Color,
    pub(crate) width: f64,
}

impl LaserStyle {
    /// Full painted width, halo included.
    pub(crate) fn glow_width(self) -> f64 {
        self.width.max(1.0) * OUTER_GLOW_SCALE
    }

    /// Bounds of everything [`render_laser_stroke`] paints for `points`.
    pub(crate) fn bounds(self, points: &[(i32, i32)]) -> Option<Rect> {
        bounding_box_for_points(points, self.glow_width())
    }

    /// Damage covering the painted stroke in path-following chunks.
    ///
    /// Neighbouring chunks share their joining point, so the round join
    /// between them is covered by both.
    pub(crate) fn damage_regions(self, points: &[(i32, i32)]) -> Vec<Rect> {
        let width = self.glow_width();
        let step = DAMAGE_CHUNK_POINTS - 1;
        let mut regions = Vec::with_capacity(points.len() / step + 1);
        let mut start = 0;
        while start < points.len() {
            let end = (start + DAMAGE_CHUNK_POINTS).min(points.len());
            if let Some(rect) = bounding_box_for_points(&points[start..end], width) {
                regions.push(rect);
            }
            if end == points.len() {
                break;
            }
            start += step;
        }
        regions
    }
}

/// Paints one laser stroke: a soft halo, the colored core, and a hot,
/// near-white centre line, all scaled by `opacity`.
///
/// A single point paints a round dot, so a tap still shows where it landed.
pub(crate) fn render_laser_stroke(
    ctx: &cairo::Context,
    points: &[(i32, i32)],
    style: LaserStyle,
    opacity: f64,
) {
    let opacity = opacity.clamp(0.0, 1.0);
    if points.is_empty() || opacity <= 0.0 {
        return;
    }

    let width = style.width.max(1.0);
    let color = style.color;
    let hot = Color {
        r: color.r + (1.0 - color.r) * HOT_CORE_WHITE_MIX,
        g: color.g + (1.0 - color.g) * HOT_CORE_WHITE_MIX,
        b: color.b + (1.0 - color.b) * HOT_CORE_WHITE_MIX,
        a: color.a,
    };
    let passes = [
        (color, width * OUTER_GLOW_SCALE, OUTER_GLOW_ALPHA),
        (color, width * INNER_GLOW_SCALE, INNER_GLOW_ALPHA),
        (color, width, 1.0),
        (hot, width * HOT_CORE_SCALE, HOT_CORE_ALPHA),
    ];

    let _ = ctx.save();
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    for (pass_color, pass_width, pass_alpha) in passes {
        ctx.set_source_rgba(
            pass_color.r,
            pass_color.g,
            pass_color.b,
            pass_color.a * pass_alpha * opacity,
        );
        ctx.set_line_width(pass_width);
        trace_path(ctx, points);
        let _ = ctx.stroke();
    }
    let _ = ctx.restore();
}

fn trace_path(ctx: &cairo::Context, points: &[(i32, i32)]) {
    let (x0, y0) = points[0];
    ctx.new_path();
    ctx.move_to(f64::from(x0), f64::from(y0));
    if points.len() == 1 {
        // A zero-length segment with round caps is how Cairo paints a dot.
        ctx.line_to(f64::from(x0), f64::from(y0));
        return;
    }
    for &(x, y) in &points[1..] {
        ctx.line_to(f64::from(x), f64::from(y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Format, ImageSurface};

    fn style() -> LaserStyle {
        LaserStyle {
            color: Color {
                r: 1.0,
                g: 0.1,
                b: 0.1,
                a: 1.0,
            },
            width: 6.0,
        }
    }

    fn painted_alpha(opacity: f64, points: &[(i32, i32)], at: (i32, i32)) -> u8 {
        let mut surface = ImageSurface::create(Format::ARgb32, 64, 64).expect("surface");
        {
            let ctx = cairo::Context::new(&surface).expect("context");
            render_laser_stroke(&ctx, points, style(), opacity);
        }
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("surface data");
        data[at.1 as usize * stride + at.0 as usize * 4 + 3]
    }

    #[test]
    fn a_stroke_paints_its_core_and_a_glow_beyond_it() {
        let points = [(10, 32), (54, 32)];

        assert_eq!(painted_alpha(1.0, &points, (32, 32)), 255);
        let glow = painted_alpha(1.0, &points, (32, 32 + 7));
        assert!(glow > 0 && glow < 255, "halo alpha {glow}");
    }

    #[test]
    fn opacity_scales_the_paint_and_zero_paints_nothing() {
        let points = [(10, 32), (54, 32)];

        let half = painted_alpha(0.5, &points, (32, 32));
        assert!(half > 64 && half < 200, "half-faded core alpha {half}");
        assert_eq!(painted_alpha(0.0, &points, (32, 32)), 0);
    }

    #[test]
    fn a_tap_paints_a_dot() {
        assert!(painted_alpha(1.0, &[(32, 32)], (32, 32)) > 0);
    }

    #[test]
    fn bounds_reach_the_outer_glow() {
        let bounds = style().bounds(&[(100, 100)]).expect("bounds");

        let reach = (style().glow_width() / 2.0).ceil() as i32;
        assert!(bounds.x <= 100 - reach && bounds.y <= 100 - reach);
        assert!(bounds.x + bounds.width >= 100 + reach);
    }

    #[test]
    fn damage_follows_a_long_path_in_chunks_that_cover_every_point() {
        let points: Vec<_> = (0..100).map(|i| (i * 10, i * 10)).collect();

        let regions = style().damage_regions(&points);
        let whole = style().bounds(&points).expect("bounds");

        assert!(regions.len() > 1);
        assert!(regions.iter().all(|rect| rect.width < whole.width));
        for &(x, y) in &points {
            assert!(regions.iter().any(|rect| rect.contains(x, y)), "({x}, {y})");
        }
    }

    #[test]
    fn damage_for_no_points_is_empty() {
        assert!(style().damage_regions(&[]).is_empty());
    }
}
