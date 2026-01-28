use std::f64::consts::{FRAC_PI_2, PI};

use super::constants::{COLOR_DIVIDER, set_color};

pub(in crate::backend::wayland::toolbar::render) fn point_in_rect(
    px: f64,
    py: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

pub(in crate::backend::wayland::toolbar::render) fn draw_round_rect(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    radius: f64,
) {
    let r = radius.min(w / 2.0).min(h / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    ctx.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    ctx.arc(x + r, y + r, r, PI, PI * 1.5);
    ctx.close_path();
}

pub(in crate::backend::wayland::toolbar::render) fn draw_divider_vertical(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    h: f64,
) {
    set_color(ctx, COLOR_DIVIDER);
    ctx.set_line_width(1.0);
    ctx.move_to(x + 0.5, y);
    ctx.line_to(x + 0.5, y + h);
    let _ = ctx.stroke();
}
