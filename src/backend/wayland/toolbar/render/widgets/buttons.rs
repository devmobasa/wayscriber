use super::constants::{
    ALPHA_DEFAULT, ALPHA_HOVER, COLOR_BUTTON_ACTIVE, COLOR_BUTTON_DEFAULT, COLOR_BUTTON_HOVER,
    COLOR_CLOSE_DEFAULT, COLOR_CLOSE_HOVER, COLOR_PIN_ACTIVE, COLOR_PIN_DEFAULT, COLOR_PIN_HOVER,
    COLOR_TEXT_PRIMARY, LINE_WIDTH_THICK, RADIUS_LG, RADIUS_STD, SPACING_XS, set_color,
};
use super::draw_round_rect;
use std::f64::consts::PI;

pub(in crate::backend::wayland::toolbar::render) fn draw_drag_handle(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    hover: bool,
) {
    draw_round_rect(ctx, x, y, w, h, RADIUS_STD);
    let alpha = if hover { ALPHA_HOVER } else { ALPHA_DEFAULT };
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha * 0.5);
    let _ = ctx.fill();

    ctx.set_line_width(1.1);
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha);
    let bar_w = w * 0.55;
    let bar_h = SPACING_XS;
    let bar_x = x + (w - bar_w) / 2.0;
    let mut bar_y = y + (h - 3.0 * bar_h) / 2.0;
    for _ in 0..3 {
        draw_round_rect(ctx, bar_x, bar_y, bar_w, bar_h, 1.0);
        let _ = ctx.fill();
        bar_y += bar_h + SPACING_XS;
    }
}

pub(in crate::backend::wayland::toolbar::render) fn draw_close_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) {
    let r = size / 2.0;
    let cx = x + r;
    let cy = y + r;

    set_color(
        ctx,
        if hover {
            COLOR_CLOSE_HOVER
        } else {
            COLOR_CLOSE_DEFAULT
        },
    );
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.fill();

    set_color(ctx, COLOR_TEXT_PRIMARY);
    ctx.set_line_width(LINE_WIDTH_THICK);
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
    let color = if pinned {
        COLOR_PIN_ACTIVE
    } else if hover {
        COLOR_PIN_HOVER
    } else {
        COLOR_PIN_DEFAULT
    };
    set_color(ctx, color);
    draw_round_rect(ctx, x, y, size, size, RADIUS_STD);
    let _ = ctx.fill();

    set_color(ctx, COLOR_TEXT_PRIMARY);
    let cx = x + size / 2.0;
    let cy = y + size / 2.0;
    let pin_r = size * 0.2;

    ctx.arc(cx, cy - pin_r * 0.5, pin_r, 0.0, PI * 2.0);
    let _ = ctx.fill();

    ctx.set_line_width(LINE_WIDTH_THICK);
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
    let color = if active {
        COLOR_BUTTON_ACTIVE
    } else if hover {
        COLOR_BUTTON_HOVER
    } else {
        COLOR_BUTTON_DEFAULT
    };
    set_color(ctx, color);
    draw_round_rect(ctx, x, y, w, h, RADIUS_LG);
    let _ = ctx.fill();
}
