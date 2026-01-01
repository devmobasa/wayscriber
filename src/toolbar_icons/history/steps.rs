use cairo::Context;
use std::f64::consts::PI;

pub(super) fn draw_step_undo(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Curved arrow (upper portion)
    ctx.arc_negative(x + s * 0.5, y + s * 0.35, s * 0.2, PI * 0.15, PI * 1.05);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.28, y + s * 0.42);
    ctx.line_to(x + s * 0.36, y + s * 0.28);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.28, y + s * 0.42);
    ctx.line_to(x + s * 0.44, y + s * 0.40);
    let _ = ctx.stroke();

    draw_step_indicator(ctx, x, y, s);
}

pub(super) fn draw_step_redo(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Curved arrow (upper portion)
    ctx.arc(x + s * 0.5, y + s * 0.35, s * 0.2, PI * 0.85, -PI * 0.05);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.72, y + s * 0.42);
    ctx.line_to(x + s * 0.64, y + s * 0.28);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.72, y + s * 0.42);
    ctx.line_to(x + s * 0.56, y + s * 0.40);
    let _ = ctx.stroke();

    draw_step_indicator(ctx, x, y, s);
}

fn draw_step_indicator(ctx: &Context, x: f64, y: f64, size: f64) {
    // Small "N" indicator at bottom center
    ctx.set_line_width((size * 0.06).max(1.0));
    let ny = y + size * 0.72;
    let nx = x + size * 0.5;
    // Draw "N" shape
    ctx.move_to(nx - size * 0.1, ny + size * 0.08);
    ctx.line_to(nx - size * 0.1, ny - size * 0.08);
    ctx.line_to(nx + size * 0.1, ny + size * 0.08);
    ctx.line_to(nx + size * 0.1, ny - size * 0.08);
    let _ = ctx.stroke();
}
