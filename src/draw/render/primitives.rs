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

/// Render an arrow (line with arrowhead pointing towards start)
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
    // Draw the main line
    render_line(ctx, x1, y1, x2, y2, color, thick);

    // Determine where the arrowhead should sit
    let (tip_x, tip_y, tail_x, tail_y) = if head_at_end {
        (x2, y2, x1, y1)
    } else {
        (x1, y1, x2, y2)
    };

    // Draw arrowhead at the tip, pointing toward the tail
    // Returns [left_point, right_point]
    let arrow_points =
        util::calculate_arrowhead_custom(tip_x, tip_y, tail_x, tail_y, arrow_length, arrow_angle);

    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Draw left line of arrowhead (from start to left point)
    ctx.move_to(tip_x as f64, tip_y as f64);
    ctx.line_to(arrow_points[0].0, arrow_points[0].1);
    let _ = ctx.stroke();

    // Draw right line of arrowhead (from start to right point)
    ctx.move_to(tip_x as f64, tip_y as f64);
    ctx.line_to(arrow_points[1].0, arrow_points[1].1);
    let _ = ctx.stroke();
}
