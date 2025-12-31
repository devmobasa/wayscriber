use super::draw_round_rect;
use std::f64::consts::PI;

pub(in crate::backend::wayland::toolbar::render) fn draw_drag_handle(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, hover: bool) {
    draw_round_rect(ctx, x, y, w, h, 4.0);
    let alpha = if hover { 0.9 } else { 0.6 };
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha * 0.5);
    let _ = ctx.fill();

    ctx.set_line_width(1.1);
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha);
    let bar_w = w * 0.55;
    let bar_h = 2.0;
    let bar_x = x + (w - bar_w) / 2.0;
    let mut bar_y = y + (h - 3.0 * bar_h) / 2.0;
    for _ in 0..3 {
        draw_round_rect(ctx, bar_x, bar_y, bar_w, bar_h, 1.0);
        let _ = ctx.fill();
        bar_y += bar_h + 2.0;
    }
}

pub(in crate::backend::wayland::toolbar::render) fn draw_close_button(ctx: &cairo::Context, x: f64, y: f64, size: f64, hover: bool) {
    let r = size / 2.0;
    let cx = x + r;
    let cy = y + r;

    if hover {
        ctx.set_source_rgba(0.8, 0.3, 0.3, 0.9);
    } else {
        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.7);
    }
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.set_line_width(2.0);
    let inset = size * 0.3;
    ctx.move_to(x + inset, y + inset);
    ctx.line_to(x + size - inset, y + size - inset);
    let _ = ctx.stroke();
    ctx.move_to(x + size - inset, y + inset);
    ctx.line_to(x + inset, y + size - inset);
    let _ = ctx.stroke();
}

pub(in crate::backend::wayland::toolbar::render) fn draw_pin_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    pinned: bool,
    hover: bool,
) {
    let (r, g, b, a) = if pinned {
        (0.25, 0.6, 0.35, 0.95)
    } else if hover {
        (0.35, 0.35, 0.45, 0.85)
    } else {
        (0.3, 0.3, 0.35, 0.7)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, size, size, 4.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    let cx = x + size / 2.0;
    let cy = y + size / 2.0;
    let pin_r = size * 0.2;

    ctx.arc(cx, cy - pin_r * 0.5, pin_r, 0.0, PI * 2.0);
    let _ = ctx.fill();

    ctx.set_line_width(2.0);
    ctx.move_to(cx, cy + pin_r * 0.5);
    ctx.line_to(cx, cy + pin_r * 2.0);
    let _ = ctx.stroke();
}

pub(in crate::backend::wayland::toolbar::render) fn draw_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    active: bool,
    hover: bool,
) {
    let (r, g, b, a) = if active {
        (0.25, 0.5, 0.95, 0.95)
    } else if hover {
        (0.35, 0.35, 0.45, 0.85)
    } else {
        (0.2, 0.22, 0.26, 0.75)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 6.0);
    let _ = ctx.fill();
}
