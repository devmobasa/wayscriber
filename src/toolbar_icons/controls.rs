use cairo::Context;
use std::f64::consts::PI;

/// Draw a minus icon
pub fn draw_icon_minus(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.15).max(2.0);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    ctx.move_to(x + s * 0.25, y + s * 0.5);
    ctx.line_to(x + s * 0.75, y + s * 0.5);
    let _ = ctx.stroke();
}

/// Draw a plus icon
pub fn draw_icon_plus(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.15).max(2.0);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    ctx.move_to(x + s * 0.25, y + s * 0.5);
    ctx.line_to(x + s * 0.75, y + s * 0.5);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.5, y + s * 0.25);
    ctx.line_to(x + s * 0.5, y + s * 0.75);
    let _ = ctx.stroke();
}

/// Draw a "more" (three dots) icon
pub fn draw_icon_more(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let r = (s * 0.09).max(1.5);
    let cy = y + s * 0.5;
    let start_x = x + s * 0.25;
    let gap = s * 0.25;

    for i in 0..3 {
        let cx = start_x + gap * i as f64;
        ctx.arc(cx, cy, r, 0.0, PI * 2.0);
        let _ = ctx.fill();
    }
}
