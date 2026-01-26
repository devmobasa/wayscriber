use crate::draw::Color;

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
    let dx = (x2 - x1) as f64;
    let dy = (y2 - y1) as f64;
    let line_length = (dx * dx + dy * dy).sqrt();

    if line_length < 1.0 {
        return;
    }

    // Unit vector along the arrow direction (from tail to tip)
    let ux = dx / line_length;
    let uy = dy / line_length;

    // Perpendicular unit vector
    let px = -uy;
    let py = ux;

    // Determine which end gets the arrowhead
    let (tip_x, tip_y, tail_x, tail_y) = if head_at_end {
        (x2 as f64, y2 as f64, x1 as f64, y1 as f64)
    } else {
        (x1 as f64, y1 as f64, x2 as f64, y2 as f64)
    };

    // Direction from tip toward tail
    let arrow_dx = tail_x - tip_x;
    let arrow_dy = tail_y - tip_y;
    let arrow_dist = (arrow_dx * arrow_dx + arrow_dy * arrow_dy).sqrt();
    let (arrow_ux, arrow_uy) = if arrow_dist > 0.0 {
        (arrow_dx / arrow_dist, arrow_dy / arrow_dist)
    } else {
        (0.0, 0.0)
    };

    // Scale arrowhead: ensure it's at least 2x the line thickness for visibility
    let scaled_length = arrow_length.max(thick * 2.5);
    // Cap at 40% of line length to avoid oversized heads on short arrows
    let effective_length = scaled_length.min(line_length * 0.4);

    // Calculate arrowhead base width: must cover the line thickness, plus extra for the angle
    let angle_rad = arrow_angle.to_radians();
    let half_base_from_angle = effective_length * angle_rad.tan();
    let half_base = half_base_from_angle.max(thick * 0.6);

    // Arrowhead points
    let base_x = tip_x + arrow_ux * effective_length;
    let base_y = tip_y + arrow_uy * effective_length;

    let left_x = base_x + px * half_base;
    let left_y = base_y + py * half_base;
    let right_x = base_x - px * half_base;
    let right_y = base_y - py * half_base;

    // Draw the shaft line, stopping at the arrowhead base to avoid overlap
    ctx.save().ok();
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_cap(cairo::LineCap::Butt);

    if head_at_end {
        ctx.move_to(x1 as f64, y1 as f64);
        ctx.line_to(base_x, base_y);
    } else {
        ctx.move_to(base_x, base_y);
        ctx.line_to(x2 as f64, y2 as f64);
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
