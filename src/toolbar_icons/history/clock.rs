use cairo::Context;
use std::f64::consts::PI;

pub(super) fn draw_clock(ctx: &Context, cx: f64, cy: f64, r: f64) {
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    ctx.move_to(cx, cy);
    ctx.line_to(cx, cy - r * 0.55);
    let _ = ctx.stroke();

    ctx.move_to(cx, cy);
    ctx.line_to(cx + r * 0.55, cy);
    let _ = ctx.stroke();
}
