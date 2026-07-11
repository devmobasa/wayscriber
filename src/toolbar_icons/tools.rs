use cairo::Context;

/// Draw a cursor/select icon (arrow pointer)
pub fn draw_icon_select(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_select(ctx, x, y, size);
}

/// Draw a pen/freehand icon (nib with a short stroke)
pub fn draw_icon_pen(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_pen(ctx, x, y, size);
}

/// Draw a line tool icon
pub fn draw_icon_line(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_line(ctx, x, y, size);
}

/// Draw a rectangle tool icon
pub fn draw_icon_rect(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_rect(ctx, x, y, size);
}

/// Draw a circle/ellipse tool icon
pub fn draw_icon_circle(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_circle(ctx, x, y, size);
}

/// Draw a triangle tool icon.
pub fn draw_icon_triangle(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_polygon_icon(ctx, x, y, size, &[0.5, 0.12, 0.86, 0.84, 0.14, 0.84]);
}

/// Draw a parallelogram tool icon.
pub fn draw_icon_parallelogram(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_polygon_icon(
        ctx,
        x,
        y,
        size,
        &[0.32, 0.16, 0.88, 0.16, 0.68, 0.84, 0.12, 0.84],
    );
}

/// Draw a rhombus tool icon.
pub fn draw_icon_rhombus(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_polygon_icon(ctx, x, y, size, &[0.5, 0.08, 0.9, 0.5, 0.5, 0.92, 0.1, 0.5]);
}

/// Draw a regular polygon picker/tool icon.
pub fn draw_icon_polygon(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_regular_polygon_icon(ctx, x, y, size, 5);
}

/// Draw a freeform polygon tool icon.
pub fn draw_icon_freeform_polygon(ctx: &Context, x: f64, y: f64, size: f64) {
    let points = &[0.18, 0.64, 0.34, 0.18, 0.72, 0.28, 0.84, 0.76, 0.46, 0.88];
    draw_polygon_icon(ctx, x, y, size, points);
    for pair in points.chunks_exact(2) {
        ctx.arc(
            x + pair[0] * size,
            y + pair[1] * size,
            (size * 0.055).max(1.2),
            0.0,
            std::f64::consts::TAU,
        );
        let _ = ctx.fill();
    }
}

/// Draw an arrow tool icon
pub fn draw_icon_arrow(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_arrow(ctx, x, y, size);
}

/// Draw a blur tool icon
pub fn draw_icon_blur(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_blur(ctx, x, y, size);
}

/// Draw an eraser tool icon
#[allow(dead_code)]
pub fn draw_icon_eraser(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_eraser(ctx, x, y, size);
}

/// Draw a text tool icon (letter T)
pub fn draw_icon_text(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_text(ctx, x, y, size);
}

/// Draw a sticky note icon (square with folded corner)
pub fn draw_icon_note(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_note(ctx, x, y, size);
}

/// Draw a highlighter tool icon (cursor with click ripple effect)
#[allow(dead_code)]
pub fn draw_icon_highlight(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_highlight(ctx, x, y, size);
}

/// Draw a marker/highlighter icon
pub fn draw_icon_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_marker(ctx, x, y, size);
}

/// Draw a step marker icon (numbered list)
pub fn draw_icon_step_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_step_marker(ctx, x, y, size);
}

fn draw_polygon_icon(ctx: &Context, x: f64, y: f64, size: f64, normalized_points: &[f64]) {
    let mut pairs = normalized_points.chunks_exact(2);
    let Some(first) = pairs.next() else {
        return;
    };
    // Inherit the caller's source color (hover/disabled states) like every
    // other icon; only line style is scoped here.
    let _ = ctx.save();
    ctx.set_line_width((size * 0.11).clamp(1.5, 2.4));
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.move_to(x + first[0] * size, y + first[1] * size);
    for pair in pairs {
        ctx.line_to(x + pair[0] * size, y + pair[1] * size);
    }
    ctx.close_path();
    let _ = ctx.stroke();
    let _ = ctx.restore();
}

fn draw_regular_polygon_icon(ctx: &Context, x: f64, y: f64, size: f64, sides: u8) {
    let center = (x + size * 0.5, y + size * 0.5);
    let radius = size * 0.38;
    let mut points = Vec::with_capacity(sides as usize * 2);
    for index in 0..sides {
        let angle = -std::f64::consts::FRAC_PI_2
            + std::f64::consts::TAU * f64::from(index) / f64::from(sides);
        points.push((center.0 + angle.cos() * radius - x) / size);
        points.push((center.1 + angle.sin() * radius - y) / size);
    }
    draw_polygon_icon(ctx, x, y, size, &points);
}
