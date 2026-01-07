use super::constants::{
    COLOR_CARD_BACKGROUND, COLOR_PANEL_BACKGROUND, RADIUS_CARD, RADIUS_PANEL, set_color,
};
use super::draw_round_rect;

pub(in crate::backend::wayland::toolbar::render) fn draw_panel_background(
    ctx: &cairo::Context,
    width: f64,
    height: f64,
) {
    set_color(ctx, COLOR_PANEL_BACKGROUND);
    draw_round_rect(ctx, 0.0, 0.0, width, height, RADIUS_PANEL);
    let _ = ctx.fill();
}

pub(in crate::backend::wayland::toolbar::render) fn draw_group_card(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    set_color(ctx, COLOR_CARD_BACKGROUND);
    draw_round_rect(ctx, x, y, w, h, RADIUS_CARD);
    let _ = ctx.fill();
}
