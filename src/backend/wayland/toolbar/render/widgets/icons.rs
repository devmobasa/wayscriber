use super::constants::{COLOR_ICON_DEFAULT, COLOR_ICON_HOVER, COLOR_ICON_HOVER_BG, set_color};
use super::draw_round_rect;

pub(in crate::backend::wayland::toolbar::render) fn set_icon_color(
    ctx: &cairo::Context,
    hover: bool,
) {
    set_color(
        ctx,
        if hover {
            COLOR_ICON_HOVER
        } else {
            COLOR_ICON_DEFAULT
        },
    );
}

/// Draws a subtle hover background behind an icon.
/// Call this before drawing the icon itself.
#[allow(dead_code)]
pub(in crate::backend::wayland::toolbar::render) fn draw_icon_hover_bg(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) {
    if hover {
        set_color(ctx, COLOR_ICON_HOVER_BG);
        draw_round_rect(ctx, x - 2.0, y - 2.0, size + 4.0, size + 4.0, 4.0);
        let _ = ctx.fill();
    }
}
