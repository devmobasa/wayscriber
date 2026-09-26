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

/// Paint the opacity track inside the slider's full `rect`. Gradient stops
/// follow stroke alpha over the knob's inset travel, including any plateau
/// at the canvas's minimum opacity.
pub(crate) fn draw_opacity_track(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    rgb: (f64, f64, f64),
    alpha_stops: [(f64, f64); 4],
) {
    let (x, y, w, h) = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let track_h = (h * 0.5).min(8.0);
    let track_y = y + (h - track_h) / 2.0;
    let knob_r = (h / 2.0).min(7.0);
    let _ = ctx.save();
    checkerboard_behind(ctx, 0.0, |ctx| {
        draw_rounded_rect(ctx, x, track_y, w, track_h, track_h / 2.0)
    });
    let gradient = cairo::LinearGradient::new(x + knob_r, 0.0, x + w - knob_r, 0.0);
    for (position, alpha) in alpha_stops {
        gradient.add_color_stop_rgba(position, rgb.0, rgb.1, rgb.2, alpha);
    }
    let _ = ctx.set_source(&gradient);
    draw_rounded_rect(ctx, x, track_y, w, track_h, track_h / 2.0);
    let _ = ctx.fill();
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarBindingHints;
    use crate::ui::toolbar::model::StylePillSlider;

    #[test]
    fn the_opacity_track_matches_stroke_alpha_through_the_minimum_plateau() {
        let state = make_test_input_state();
        let mut snapshot = crate::ui::toolbar::ToolbarSnapshot::from_input_with_bindings(
            &state,
            ToolbarBindingHints::default(),
        );
        snapshot.color.r = 0.0;
        snapshot.color.g = 0.0;
        snapshot.color.b = 0.0;
        let h = 24;

        for (w, color_alpha, setting, expected_alpha) in [
            (1000, 0.2, 0.2, 0.05),
            (1000, 0.2, 0.25, 0.05),
            (1000, 0.2, 0.5, 0.1),
            (1000, 0.2, 0.9, 0.18),
            (1000, 0.0, 0.5, 0.05),
            (1000, 0.02, 0.9, 0.05),
            (101, 1.0, 0.2, 0.2),
            (101, 1.0, 0.9, 0.9),
        ] {
            snapshot.color.a = color_alpha;
            snapshot.marker_opacity = setting;
            let paint = StylePillSlider::Opacity
                .opacity_paint(&snapshot)
                .expect("marker opacity paint");
            let t = (setting - 0.05) / 0.85;
            let x = (7.0 + t * (f64::from(w) - 14.0)).floor() as usize;
            let sample = |alpha_stops| {
                let mut surface =
                    cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).expect("surface");
                {
                    let ctx = cairo::Context::new(&surface).expect("context");
                    draw_opacity_track(
                        &ctx,
                        (0.0, 0.0, f64::from(w), f64::from(h)),
                        paint.rgb,
                        alpha_stops,
                    );
                }
                surface.flush();
                let stride = surface.stride() as usize;
                let data = surface.data().expect("pixels");
                let offset = (h as usize / 2) * stride + x * 4;
                let pixel = u32::from_ne_bytes(data[offset..offset + 4].try_into().unwrap());
                f64::from((pixel >> 16) & 0xff)
            };
            let background = sample([(0.0, 0.0); 4]);
            let actual = sample(paint.alpha_stops);
            let expected = background * (1.0 - expected_alpha);

            assert!(
                (actual - expected).abs() <= 1.5,
                "color alpha {color_alpha}, setting {setting}: pixel {actual}, expected {expected}"
            );
        }
    }

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
