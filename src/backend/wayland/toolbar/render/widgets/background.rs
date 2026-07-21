use super::constants::{
    COLOR_CARD_BACKGROUND, COLOR_PANEL_BACKGROUND, COLOR_PANEL_BORDER, RADIUS_CARD, RADIUS_PANEL,
    set_color,
};
use super::draw_round_rect;
use crate::ui::theme::Rgba;
/// Two-layer popover drop shadow approximating the GTK popover's
/// `0 2px 12px rgba(0, 0, 0, 0.35)` box-shadow: a wide faint halo plus a
/// tight core. Specific to this fake-blur trick — not the shared shadow token.
const COLOR_POPOVER_SHADOW_HALO: Rgba = (0.0, 0.0, 0.0, 0.18);
const COLOR_POPOVER_SHADOW_CORE: Rgba = (0.0, 0.0, 0.0, 0.35);

pub(in crate::backend::wayland::toolbar::render) fn draw_panel_background(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    set_color(ctx, COLOR_PANEL_BACKGROUND);
    draw_round_rect(ctx, x, y, width, height, RADIUS_PANEL);
    let _ = ctx.fill();

    // 1px hairline border, matching the GTK panel's
    // `border: 1px solid rgba(255, 255, 255, 0.10)`.
    set_color(ctx, COLOR_PANEL_BORDER);
    ctx.set_line_width(1.0);
    draw_round_rect(
        ctx,
        x + 0.5,
        y + 0.5,
        width - 1.0,
        height - 1.0,
        RADIUS_PANEL - 0.5,
    );
    let _ = ctx.stroke();
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

/// Anchored popover panel: opaque background with a border and a caret
/// pointing at the anchor. `caret_x` comes from the popover placement math
/// (`view::popover`); `caret_up` is true when the popover opened below its
/// anchor so the caret points up at it.
pub(in crate::backend::wayland::toolbar::render) fn draw_popover_panel(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    caret_x: f64,
    caret_up: bool,
) {
    // Soft drop shadow so the panel reads as floating over the bar. Two
    // layers approximate the GTK popover's `0 2px 12px rgba(0, 0, 0, 0.35)`
    // box-shadow: a wide faint halo plus a tight core at the same offset.
    set_color(ctx, COLOR_POPOVER_SHADOW_HALO);
    draw_round_rect(ctx, x - 3.0, y - 1.0, w + 6.0, h + 6.0, RADIUS_PANEL + 3.0);
    let _ = ctx.fill();
    set_color(ctx, COLOR_POPOVER_SHADOW_CORE);
    draw_round_rect(ctx, x, y + 2.0, w, h, RADIUS_PANEL);
    let _ = ctx.fill();

    set_color(ctx, COLOR_PANEL_BACKGROUND);
    draw_round_rect(ctx, x, y, w, h, RADIUS_PANEL);
    let _ = ctx.fill();

    let caret_half = 6.0;
    let caret_h = 6.0;
    let caret_x = caret_x.clamp(x + caret_half + 2.0, x + w - caret_half - 2.0);
    if caret_up {
        ctx.move_to(caret_x - caret_half, y);
        ctx.line_to(caret_x, y - caret_h);
        ctx.line_to(caret_x + caret_half, y);
    } else {
        ctx.move_to(caret_x - caret_half, y + h);
        ctx.line_to(caret_x, y + h + caret_h);
        ctx.line_to(caret_x + caret_half, y + h);
    }
    ctx.close_path();
    let _ = ctx.fill();

    // Hairline border at the same alpha as the GTK popover contents border.
    set_color(ctx, COLOR_PANEL_BORDER);
    ctx.set_line_width(1.0);
    draw_round_rect(ctx, x + 0.5, y + 0.5, w - 1.0, h - 1.0, RADIUS_PANEL);
    let _ = ctx.stroke();
}
