use crate::draw::Color;
use crate::util;

/// Render a straight line
pub(super) fn render_line(
    ctx: &cairo::Context,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color: Color,
    thick: f64,
) {
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_cap(cairo::LineCap::Round);

    ctx.move_to(x1 as f64, y1 as f64);
    ctx.line_to(x2 as f64, y2 as f64);
    let _ = ctx.stroke();
}

/// Render a rectangle (outline)
#[allow(clippy::too_many_arguments)]
pub(super) fn render_rect(
    ctx: &cairo::Context,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    fill: bool,
    color: Color,
    thick: f64,
) {
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_join(cairo::LineJoin::Miter);

    // Normalize rectangle to handle any legacy data with negative dimensions
    // (InputState already normalizes, but this ensures consistent rendering)
    let (norm_x, norm_w) = if w >= 0 {
        (x as f64, w as f64)
    } else {
        ((x + w) as f64, (-w) as f64)
    };
    let (norm_y, norm_h) = if h >= 0 {
        (y as f64, h as f64)
    } else {
        ((y + h) as f64, (-h) as f64)
    };

    ctx.rectangle(norm_x, norm_y, norm_w, norm_h);
    if fill {
        let _ = ctx.save();
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        let _ = ctx.fill_preserve();
        let _ = ctx.restore();
    }
    let _ = ctx.stroke();
}

/// Render an ellipse using Cairo's arc with scaling
#[allow(clippy::too_many_arguments)]
pub(super) fn render_ellipse(
    ctx: &cairo::Context,
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    fill: bool,
    color: Color,
    thick: f64,
) {
    if rx == 0 || ry == 0 {
        return;
    }

    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);

    ctx.save().ok();
    ctx.translate(cx as f64, cy as f64);
    ctx.scale(rx as f64, ry as f64);
    ctx.new_sub_path();
    ctx.arc(0.0, 0.0, 1.0, 0.0, 2.0 * std::f64::consts::PI);
    if fill {
        let _ = ctx.save();
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        let _ = ctx.fill_preserve();
        ctx.restore().ok();
    }
    ctx.restore().ok();

    let _ = ctx.stroke();
}

/// Render a closed polygon outline with optional fill.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_polygon(
    ctx: &cairo::Context,
    points: &[(i32, i32)],
    fill: bool,
    color: Color,
    thick: f64,
) {
    if !crate::draw::shape::has_minimum_distinct_points(points) {
        return;
    }

    let _ = ctx.save();
    ctx.new_path();
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.move_to(points[0].0 as f64, points[0].1 as f64);
    for &(x, y) in &points[1..] {
        ctx.line_to(x as f64, y as f64);
    }
    ctx.close_path();
    if fill {
        let _ = ctx.fill_preserve();
    }
    let _ = ctx.stroke();
    let _ = ctx.restore();
}

/// Render the in-progress freeform polygon preview without open-end round cap blobs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_polygon_preview(
    ctx: &cairo::Context,
    points: &[(i32, i32)],
    fill: bool,
    color: Color,
    thick: f64,
) {
    if points.len() < 2 {
        return;
    }

    let _ = ctx.save();
    ctx.new_path();
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_cap(cairo::LineCap::Butt);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.move_to(points[0].0 as f64, points[0].1 as f64);
    for &(x, y) in &points[1..] {
        ctx.line_to(x as f64, y as f64);
    }
    if crate::draw::shape::has_minimum_distinct_points(points) {
        ctx.close_path();
        if fill {
            let _ = ctx.fill_preserve();
        }
    }
    let _ = ctx.stroke();
    let _ = ctx.restore();
}

/// Render an arrow: a tapered shaft fused into an arrowhead pointing at the tip.
///
/// Shaft and head are one filled path, so there is no shoulder step where they
/// meet and a semi-transparent color paints at an even opacity throughout.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_arrow(
    ctx: &cairo::Context,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color: Color,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
    head_at_end: bool,
) {
    // Determine which end gets the arrowhead
    let (tip_x, tip_y, tail_x, tail_y) = if head_at_end {
        (x2, y2, x1, y1)
    } else {
        (x1, y1, x2, y2)
    };

    let Some(outline) = util::calculate_arrow_outline(
        tip_x,
        tip_y,
        tail_x,
        tail_y,
        thick,
        arrow_length,
        arrow_angle,
    ) else {
        return;
    };

    ctx.save().ok();
    ctx.new_path();
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);

    let [first, rest @ ..] = &outline.points;
    ctx.move_to(first.0, first.1);
    for &(x, y) in rest {
        ctx.line_to(x, y);
    }
    ctx.close_path();
    let _ = ctx.fill();
    ctx.restore().ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Context, ImageSurface};

    fn surface_with_context(width: i32, height: i32) -> (ImageSurface, Context) {
        let surface = ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
        let ctx = Context::new(&surface).unwrap();
        (surface, ctx)
    }

    fn alpha_at(surface: &mut ImageSurface, x: i32, y: i32) -> u8 {
        let stride = surface.stride() as usize;
        let offset = y as usize * stride + x as usize * 4 + 3;
        surface.data().unwrap()[offset]
    }

    #[test]
    fn polygon_preview_uses_butt_caps_for_open_edges() {
        let (mut surface, ctx) = surface_with_context(90, 70);
        let red = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };

        render_polygon_preview(&ctx, &[(20, 35), (70, 35)], false, red, 10.0);

        drop(ctx);
        assert_eq!(
            alpha_at(&mut surface, 17, 35),
            0,
            "open polygon preview edges should not leave round endpoint blobs"
        );
        assert!(
            alpha_at(&mut surface, 25, 35) > 0,
            "open polygon preview edge should still render"
        );
    }

    fn painted_column_height(surface: &mut ImageSurface, x: i32, height: i32) -> i32 {
        (0..height)
            .filter(|&y| alpha_at(surface, x, y) > 0)
            .count()
            .try_into()
            .unwrap_or(i32::MAX)
    }

    #[test]
    fn arrow_shaft_tapers_from_tail_to_arrowhead() {
        let (mut surface, ctx) = surface_with_context(460, 120);
        let red = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };

        // Horizontal arrow of length 400 at 10px thick: the head is 3x the
        // stroke (30px), so the head base sits at x = 390 and the bevelled
        // shoulder at x = 393. The samples below land on the tail, on the bare
        // shaft behind the head, and across the head itself.
        render_arrow(&ctx, 20, 60, 420, 60, red, 10.0, 20.0, 24.0, true);

        drop(ctx);
        let near_tail = painted_column_height(&mut surface, 25, 120);
        let near_shoulder = painted_column_height(&mut surface, 370, 120);
        let across_head = painted_column_height(&mut surface, 398, 120);

        assert!(near_tail > 0, "tail should still paint");
        assert!(
            near_tail < near_shoulder,
            "shaft should widen toward the head: tail {near_tail} vs shoulder {near_shoulder}"
        );
        assert!(
            across_head > near_shoulder,
            "arrowhead should be wider than the shaft: head {across_head} vs shoulder {near_shoulder}"
        );
    }

    #[test]
    fn arrow_is_one_connected_fill_with_no_gap_at_the_shoulder() {
        let (mut surface, ctx) = surface_with_context(220, 120);
        let red = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };

        render_arrow(&ctx, 20, 60, 180, 60, red, 20.0, 30.0, 30.0, true);

        drop(ctx);
        // Every column between tail and tip must carry paint on the centre line.
        for x in 21..=178 {
            assert!(
                alpha_at(&mut surface, x, 60) > 0,
                "arrow centre line has a gap at x = {x}"
            );
        }
    }

    #[test]
    fn arrow_does_not_connect_to_existing_current_path() {
        let (mut surface, ctx) = surface_with_context(220, 140);
        let red = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };

        ctx.move_to(10.0, 130.0);
        render_arrow(&ctx, 120, 40, 200, 40, red, 8.0, 20.0, 30.0, true);

        drop(ctx);
        assert_eq!(
            alpha_at(&mut surface, 60, 100),
            0,
            "arrow fill must not absorb a prior current point into its polygon"
        );
        assert!(
            alpha_at(&mut surface, 190, 40) > 0,
            "arrow should still render"
        );
    }

    #[test]
    fn ellipse_does_not_connect_to_existing_current_path() {
        let (mut surface, ctx) = surface_with_context(120, 120);
        let magenta = Color {
            r: 1.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        };

        ctx.move_to(10.0, 90.0);
        render_ellipse(&ctx, 80, 20, 20, 10, false, magenta, 6.0);

        drop(ctx);
        assert_eq!(
            alpha_at(&mut surface, 48, 60),
            0,
            "ellipse rendering must not stroke a line from a prior current point"
        );
        assert!(
            alpha_at(&mut surface, 100, 20) > 0,
            "ellipse stroke should still render"
        );
    }
}
