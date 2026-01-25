use super::draw_round_rect;
use crate::draw::Color;

/// Convert RGB to HSV color space.
/// Returns (hue, saturation, value) all in 0.0-1.0 range.
pub fn rgb_to_hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let value = max;
    let saturation = if max == 0.0 { 0.0 } else { delta / max };

    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };

    (hue, saturation, value)
}

pub(in crate::backend::wayland::toolbar::render) fn draw_swatch(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    color: Color,
    active: bool,
) {
    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
    draw_round_rect(ctx, x, y, size, size, 4.0);
    let _ = ctx.fill();

    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance < 0.3 {
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.8);
        ctx.set_line_width(1.5);
        draw_round_rect(ctx, x, y, size, size, 4.0);
        let _ = ctx.stroke();
    }

    if active {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        ctx.set_line_width(2.0);
        draw_round_rect(ctx, x - 2.0, y - 2.0, size + 4.0, size + 4.0, 5.0);
        let _ = ctx.stroke();
    }
}

pub(in crate::backend::wayland::toolbar::render) fn draw_color_picker(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    let hue_grad = cairo::LinearGradient::new(x, y, x + w, y);
    hue_grad.add_color_stop_rgba(0.0, 1.0, 0.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.17, 1.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.33, 0.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.5, 0.0, 1.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.66, 0.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.83, 1.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(1.0, 1.0, 0.0, 0.0, 1.0);

    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&hue_grad);
    let _ = ctx.fill();

    let val_grad = cairo::LinearGradient::new(x, y, x, y + h);
    val_grad.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.0);
    val_grad.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.65);
    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&val_grad);
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
