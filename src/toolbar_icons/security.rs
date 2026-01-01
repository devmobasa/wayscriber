use cairo::Context;
use std::f64::consts::PI;

/// Draw a closed lock icon.
pub fn draw_icon_lock(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.08).max(1.4);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let body_w = s * 0.5;
    let body_h = s * 0.38;
    let body_x = x + (s - body_w) / 2.0;
    let body_y = y + s * 0.45;

    ctx.rectangle(body_x, body_y, body_w, body_h);
    let _ = ctx.stroke();

    let shackle_r = body_w * 0.3;
    let shackle_cx = x + s * 0.5;
    let shackle_cy = body_y;
    ctx.arc(shackle_cx, shackle_cy, shackle_r, PI, PI * 2.0);
    let _ = ctx.stroke();
}

/// Draw an open lock icon.
pub fn draw_icon_unlock(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.08).max(1.4);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let body_w = s * 0.5;
    let body_h = s * 0.38;
    let body_x = x + (s - body_w) / 2.0;
    let body_y = y + s * 0.45;

    ctx.rectangle(body_x, body_y, body_w, body_h);
    let _ = ctx.stroke();

    let shackle_r = body_w * 0.3;
    let shackle_cx = x + s * 0.42;
    let shackle_cy = body_y;
    ctx.arc(shackle_cx, shackle_cy, shackle_r, PI * 0.9, PI * 1.9);
    let _ = ctx.stroke();
}
