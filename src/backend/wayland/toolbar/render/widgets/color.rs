use super::constants::{
    COLOR_ACCENT, COLOR_SWATCH_HAIRLINE, COLOR_SWATCH_HAIRLINE_DARK, set_color,
};
use super::draw_round_rect;
use crate::draw::Color;
use crate::ui::theme::{Rgba, SHADOW_RGBA};
/// Outline around the gradient picker areas (sat/val square, hue bar).
const COLOR_PICKER_OUTLINE: Rgba = (1.0, 1.0, 1.0, 0.4);
/// Outer white ring of the picker's position indicator dot.
const COLOR_INDICATOR_RING: Rgba = (1.0, 1.0, 1.0, 0.9);

/// Rounded-square quick-color swatch: the fill sits one pixel inside the
/// hit rect, a subtle inner hairline keeps every fill defined against the
/// panel (boosted for dark colors), and the active state draws a 2px accent
/// ring with a ~2px gap around the fill.
///
/// A translucent color is painted at its own alpha over the checkerboard, so
/// the swatch shows the transparency the canvas will draw with instead of
/// presenting it as an opaque color.
pub(in crate::backend::wayland::toolbar::render) fn draw_swatch(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    color: Color,
    active: bool,
) {
    let fill_path =
        |ctx: &cairo::Context| draw_round_rect(ctx, x + 1.0, y + 1.0, size - 2.0, size - 2.0, 5.0);
    crate::ui::checkerboard_behind(ctx, color.a, fill_path);
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    fill_path(ctx);
    let _ = ctx.fill();

    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance < 0.3 {
        set_color(ctx, COLOR_SWATCH_HAIRLINE_DARK);
    } else {
        set_color(ctx, COLOR_SWATCH_HAIRLINE);
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

    set_color(ctx, COLOR_PICKER_OUTLINE);
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

    set_color(ctx, COLOR_PICKER_OUTLINE);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

/// Draw a color indicator dot on the gradient picker. Painted opaque on
/// purpose: it marks a position on the gradient, not the color's alpha.
pub(in crate::backend::wayland::toolbar::render) fn draw_color_indicator(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    color: Color,
) {
    let radius = 5.0;

    // Draw outer white ring
    set_color(ctx, COLOR_INDICATOR_RING);
    ctx.arc(x, y, radius + 1.5, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Draw inner color circle
    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
    ctx.arc(x, y, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Draw dark outline for visibility
    set_color(ctx, SHADOW_RGBA);
    ctx.set_line_width(1.0);
    ctx.arc(x, y, radius + 1.5, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.stroke();
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Context, Format, ImageSurface};

    /// `(r, g, b)` of one pixel, 0-255. Rgb24 packs pixels as native-endian
    /// 32-bit words, so on little-endian the byte order is B, G, R, unused.
    fn pixel_at(surface: &mut ImageSurface, x: i32, y: i32) -> (u8, u8, u8) {
        let stride = surface.stride() as usize;
        let offset = y as usize * stride + x as usize * 4;
        let data = surface.data().expect("pixel data");
        (data[offset + 2], data[offset + 1], data[offset])
    }

    /// Paint one swatch of `color` on white and sample its middle.
    fn swatch_center(color: Color) -> (u8, u8, u8) {
        let size = 24.0;
        let surface = ImageSurface::create(Format::Rgb24, 32, 32).expect("surface");
        {
            let ctx = Context::new(&surface).expect("context");
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            let _ = ctx.paint();
            draw_swatch(&ctx, 4.0, 4.0, size, color, false);
        }
        let mut surface = surface;
        pixel_at(&mut surface, 4 + size as i32 / 2, 4 + size as i32 / 2)
    }

    #[test]
    fn a_translucent_swatch_shows_its_transparency() {
        let red = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        assert_eq!(swatch_center(red), (255, 0, 0), "opaque fills stay exact");

        // Half-alpha red over the checkerboard lets the gray through, so the
        // swatch cannot be mistaken for the opaque color.
        let (r, g, b) = swatch_center(Color { a: 0.5, ..red });
        assert!(r < 255, "translucent swatch painted at full red: {r}");
        assert!(
            g > 0 && b > 0,
            "checkerboard did not show through: ({r}, {g}, {b})"
        );
    }
}
