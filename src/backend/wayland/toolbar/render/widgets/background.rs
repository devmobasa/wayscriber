use super::draw_round_rect;

pub(in crate::backend::wayland::toolbar::render) fn draw_panel_background(ctx: &cairo::Context, width: f64, height: f64) {
    ctx.set_source_rgba(0.05, 0.05, 0.08, 0.92);
    draw_round_rect(ctx, 0.0, 0.0, width, height, 14.0);
    let _ = ctx.fill();
}

pub(in crate::backend::wayland::toolbar::render) fn draw_group_card(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    ctx.set_source_rgba(0.12, 0.12, 0.18, 0.35);
    draw_round_rect(ctx, x, y, w, h, 8.0);
    let _ = ctx.fill();
}
