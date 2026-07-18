use super::constants::{COLOR_ACCENT, set_color};
use super::draw_round_rect;
use crate::draw::Color;

/// Rounded-square quick-color swatch: the fill sits one pixel inside the
/// hit rect, a subtle inner hairline keeps every fill defined against the
/// panel (boosted for dark colors), and the active state draws a 2px accent
/// ring with a ~2px gap around the fill.
pub(in crate::backend::wayland::toolbar::render) fn draw_swatch(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    color: Color,
    active: bool,
) {
    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
    draw_round_rect(ctx, x + 1.0, y + 1.0, size - 2.0, size - 2.0, 5.0);
    let _ = ctx.fill();

    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance < 0.3 {
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.8);
    } else {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.16);
    }
    ctx.set_line_width(1.0);
    draw_round_rect(ctx, x + 1.5, y + 1.5, size - 3.0, size - 3.0, 4.5);
    let _ = ctx.stroke();

    if active {
        set_color(ctx, COLOR_ACCENT);
        ctx.set_line_width(2.0);
        draw_round_rect(ctx, x - 2.0, y - 2.0, size + 4.0, size + 4.0, 7.0);
        let _ = ctx.stroke();
    }
}

fn set_hue_gradient(ctx: &cairo::Context, x: f64, y: f64, w: f64) {
    let hue_grad = cairo::LinearGradient::new(x, y, x + w, y);
    hue_grad.add_color_stop_rgba(0.0, 1.0, 0.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.17, 1.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.33, 0.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.5, 0.0, 1.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.66, 0.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.83, 1.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(1.0, 1.0, 0.0, 0.0, 1.0);
    let _ = ctx.set_source(&hue_grad);
}

/// Draw the 2-D saturation/value area for a fixed hue: white→hue across x,
/// transparent→black down y.
pub(in crate::backend::wayland::toolbar::render) fn draw_sat_val_area(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    hue: f64,
) {
    let hue_color = crate::draw::color::hsv_to_rgb(hue, 1.0, 1.0);

    let sat_grad = cairo::LinearGradient::new(x, y, x + w, y);
    sat_grad.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 1.0);
    sat_grad.add_color_stop_rgba(1.0, hue_color.r, hue_color.g, hue_color.b, 1.0);
    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&sat_grad);
    let _ = ctx.fill();

    let val_grad = cairo::LinearGradient::new(x, y, x, y + h);
    val_grad.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
    val_grad.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 1.0);
    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&val_grad);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.4);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

/// Draw the horizontal hue bar.
pub(in crate::backend::wayland::toolbar::render) fn draw_hue_bar(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    ctx.rectangle(x, y, w, h);
    set_hue_gradient(ctx, x, y, w);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.4);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

/// Draw a color indicator dot on the gradient picker.
pub(in crate::backend::wayland::toolbar::render) fn draw_color_indicator(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    color: Color,
) {
    let radius = 5.0;

    // Draw outer white ring
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.arc(x, y, radius + 1.5, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Draw inner color circle
    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
    ctx.arc(x, y, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Draw dark outline for visibility
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.3);
    ctx.set_line_width(1.0);
    ctx.arc(x, y, radius + 1.5, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.stroke();
}
