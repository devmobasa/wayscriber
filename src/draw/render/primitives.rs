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

/// Render an arrow (line with arrowhead pointing towards the tip)
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

    let Some(geometry) = util::calculate_arrowhead_triangle_custom(
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
    let (tip_x, tip_y) = geometry.tip;
    let (base_x, base_y) = geometry.base;
    let (left_x, left_y) = geometry.left;
    let (right_x, right_y) = geometry.right;
    let (tail_x, tail_y) = (tail_x as f64, tail_y as f64);

    // Draw the shaft line, stopping at the arrowhead base to avoid overlap
    ctx.save().ok();
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_cap(cairo::LineCap::Butt);

    if head_at_end {
        ctx.move_to(tail_x, tail_y);
        ctx.line_to(base_x, base_y);
    } else {
        ctx.move_to(base_x, base_y);
        ctx.line_to(tail_x, tail_y);
    }
    let _ = ctx.stroke();

    // Draw filled arrowhead triangle
    ctx.move_to(tip_x, tip_y);
    ctx.line_to(left_x, left_y);
    ctx.line_to(right_x, right_y);
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
