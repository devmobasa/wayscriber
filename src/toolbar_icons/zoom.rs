use cairo::Context;
use std::f64::consts::PI;

/// Draw a zoom-in icon (magnifier with plus).
pub fn draw_icon_zoom_in(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let cx = x + s * 0.45;
    let cy = y + s * 0.45;
    let r = s * 0.26;

    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    ctx.move_to(cx + r * 0.6, cy + r * 0.6);
    ctx.line_to(x + s * 0.9, y + s * 0.9);
    let _ = ctx.stroke();

    ctx.move_to(cx - r * 0.5, cy);
    ctx.line_to(cx + r * 0.5, cy);
    let _ = ctx.stroke();
    ctx.move_to(cx, cy - r * 0.5);
    ctx.line_to(cx, cy + r * 0.5);
    let _ = ctx.stroke();
}

/// Draw a zoom-out icon (magnifier with minus).
pub fn draw_icon_zoom_out(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let cx = x + s * 0.45;
    let cy = y + s * 0.45;
    let r = s * 0.26;

    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    ctx.move_to(cx + r * 0.6, cy + r * 0.6);
    ctx.line_to(x + s * 0.9, y + s * 0.9);
    let _ = ctx.stroke();

    ctx.move_to(cx - r * 0.5, cy);
    ctx.line_to(cx + r * 0.5, cy);
    let _ = ctx.stroke();
}

/// Draw a zoom reset icon (magnifier with crosshair).
pub fn draw_icon_zoom_reset(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let cx = x + s * 0.45;
    let cy = y + s * 0.45;
    let r = s * 0.26;

    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    ctx.move_to(cx + r * 0.6, cy + r * 0.6);
    ctx.line_to(x + s * 0.9, y + s * 0.9);
    let _ = ctx.stroke();

    let dot_r = r * 0.15;
    ctx.arc(cx, cy, dot_r, 0.0, PI * 2.0);
    let _ = ctx.fill();
}
