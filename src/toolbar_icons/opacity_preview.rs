//! The marker opacity slider's track and swatch, painted the same way by both
//! toolbar frontends.
//!
//! A bare percentage does not say which way is see-through, so the slider
//! shows it instead. Its track fades from clear to solid in the current color
//! across the slider's range, and the swatch beside it lays a marker stroke
//! over a few lines of sample text at the current opacity: how much of the
//! covered line still shows is what the value does.

use crate::ui::theme::{Rgba, set_color};
use crate::ui::{checkerboard_behind, draw_rounded_rect};

/// The swatch's sample page and its text lines.
const PAPER: Rgba = (0.94, 0.94, 0.92, 1.0);
const INK: Rgba = (0.30, 0.31, 0.34, 1.0);
/// Sample lines as (vertical center, length), both as fractions of the
/// swatch; the middle one is the line the marker covers.
const LINES: [(f64, f64); 3] = [(0.24, 0.70), (0.50, 1.0), (0.76, 0.50)];
const COVERED_LINE: f64 = 0.50;

/// Paint the swatch for a marker stroke in `rgb` at `opacity` into `rect`:
/// the sample page, its text, and the stroke across the middle line.
pub(crate) fn draw_opacity_swatch(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    rgb: (f64, f64, f64),
    opacity: f64,
) {
    let (x, y, w, h) = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let _ = ctx.save();
    draw_rounded_rect(ctx, x, y, w, h, (h * 0.25).min(5.0));
    set_color(ctx, PAPER);
    let _ = ctx.fill();

    let margin = (w * 0.16).round();
    let line_h = (h * 0.1).max(1.0);
    set_color(ctx, INK);
    for (center, length) in LINES {
        ctx.rectangle(
            x + margin,
            y + h * center - line_h / 2.0,
            (w - margin * 2.0) * length,
            line_h,
        );
    }
    let _ = ctx.fill();

    let band_h = h * 0.32;
    ctx.rectangle(
        x + margin * 0.5,
        y + h * COVERED_LINE - band_h / 2.0,
        w - margin,
        band_h,
    );
    ctx.set_source_rgba(rgb.0, rgb.1, rgb.2, opacity.clamp(0.0, 1.0));
    let _ = ctx.fill();
    let _ = ctx.restore();
}

/// Paint a slider track in `rect` that fades from `alpha_range.0` to
/// `alpha_range.1` of `rgb` over the checkerboard, so the knob's position
/// reads as how solid the stroke will be.
pub(crate) fn draw_opacity_track(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    rgb: (f64, f64, f64),
    alpha_range: (f64, f64),
) {
    let (x, y, w, h) = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let radius = h / 2.0;
    let _ = ctx.save();
    checkerboard_behind(ctx, 0.0, |ctx| draw_rounded_rect(ctx, x, y, w, h, radius));
    let gradient = cairo::LinearGradient::new(x, 0.0, x + w, 0.0);
    gradient.add_color_stop_rgba(0.0, rgb.0, rgb.1, rgb.2, alpha_range.0);
    gradient.add_color_stop_rgba(1.0, rgb.0, rgb.1, rgb.2, alpha_range.1);
    let _ = ctx.set_source(&gradient);
    draw_rounded_rect(ctx, x, y, w, h, radius);
    let _ = ctx.fill();
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Red channel of the swatch's covered line, center of the swatch, for a
    /// black ink line under a pure red marker at `opacity`.
    fn covered_line_red(opacity: f64) -> u8 {
        let (w, h) = (44, 20);
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).expect("surface");
        {
            let ctx = cairo::Context::new(&surface).expect("context");
            draw_opacity_swatch(
                &ctx,
                (0.0, 0.0, f64::from(w), f64::from(h)),
                (1.0, 0.0, 0.0),
                opacity,
            );
        }
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("pixels");
        let offset = (h as usize / 2) * stride + (w as usize / 2) * 4;
        let pixel = u32::from_ne_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        ((pixel >> 16) & 0xff) as u8
    }

    #[test]
    fn a_more_solid_marker_hides_more_of_the_covered_line() {
        let faint = covered_line_red(0.1);
        let solid = covered_line_red(0.9);

        assert!(
            solid > faint + 100,
            "solid {solid} should cover the ink far more than faint {faint}"
        );
    }
}
