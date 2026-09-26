//! The Pen feel panel's live smoothing preview, painted the same way by both
//! toolbar frontends.
//!
//! A fixed sample stroke carrying a hand's tremor is drawn twice: faintly as
//! it was drawn, and in the accent as the pen-release smoothing leaves it at
//! the current level. The smoothing is the real [`crate::draw::shape::smooth_path`],
//! the same function a lifted pen runs, so the preview shows what the level
//! will do to the next stroke. At Off the two coincide and only the accent
//! stroke shows.

use std::f64::consts::TAU;

use crate::ui::theme::set_color;
use crate::ui::theme::toolbar::{
    COLOR_ACCENT, COLOR_SMOOTHING_PREVIEW_BG, COLOR_SMOOTHING_PREVIEW_RAW,
};

/// How many sample units make one preview unit. The sample is authored at
/// four times the preview's size so the smoothing's rounding to whole units
/// stays far below a preview pixel.
const SAMPLE_SCALE: f64 = 4.0;
/// Sample space: the panel's preview (220 x 48 spec units) at `SAMPLE_SCALE`.
const SAMPLE_W: f64 = 880.0;
const SAMPLE_H: f64 = 192.0;
/// Points along the sample, about two preview pixels apart: roughly how
/// densely a pointer reports a brisk stroke.
const SAMPLE_POINTS: usize = 110;
/// Stroke widths and well radius in preview units.
const RAW_WIDTH: f64 = 1.5;
const SMOOTHED_WIDTH: f64 = 2.5;
const WELL_RADIUS: f64 = 6.0;

/// The sample stroke as drawn, in sample units: a gentle wave, plus tremor at
/// three frequencies so each smoothing level removes visibly more of it (one
/// pass all but erases the fastest shake; the slowest survives even the
/// maximum). Deterministic, so both frontends and the tests draw one stroke.
pub(crate) fn smoothing_preview_sample() -> Vec<(i32, i32)> {
    (0..SAMPLE_POINTS)
        .map(|index| {
            let i = index as f64;
            let t = i / (SAMPLE_POINTS - 1) as f64;
            let x = 48.0 + t * (SAMPLE_W - 96.0) + 3.0 * (1.9 * i + 0.5).sin();
            let wave = 44.0 * (TAU * 0.85 * t - 0.4).sin();
            let tremor =
                9.0 * (2.4 * i).sin() + 11.0 * (1.3 * i + 0.7).sin() + 8.0 * (0.8 * i + 2.1).sin();
            let y = SAMPLE_H / 2.0 + wave + tremor;
            (x.round() as i32, y.round() as i32)
        })
        .collect()
}

/// Paint the preview for smoothing `level` into `rect` (logical units): the
/// well, the stroke as drawn, and the stroke as smoothed on top.
pub(crate) fn draw_smoothing_preview(ctx: &cairo::Context, rect: (f64, f64, f64, f64), level: u8) {
    let (x, y, w, h) = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    // Uniform scale into the rect, centered: preview units per sample unit.
    let scale = (w / SAMPLE_W).min(h / SAMPLE_H);
    let unit = scale * SAMPLE_SCALE;
    let origin = (
        x + (w - SAMPLE_W * scale) / 2.0,
        y + (h - SAMPLE_H * scale) / 2.0,
    );
    let raw = smoothing_preview_sample();
    let smoothed = crate::draw::shape::smooth_path(&raw, level);

    let _ = ctx.save();
    set_color(ctx, COLOR_SMOOTHING_PREVIEW_BG);
    rounded_rect(ctx, rect, (WELL_RADIUS * unit).min(h / 2.0));
    let _ = ctx.fill();

    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    for (points, color, width) in [
        (&raw, COLOR_SMOOTHING_PREVIEW_RAW, RAW_WIDTH),
        (&smoothed, COLOR_ACCENT, SMOOTHED_WIDTH),
    ] {
        stroke_polyline(ctx, points, origin, scale);
        set_color(ctx, color);
        ctx.set_line_width(width * unit);
        let _ = ctx.stroke();
    }
    let _ = ctx.restore();
}

fn stroke_polyline(ctx: &cairo::Context, points: &[(i32, i32)], origin: (f64, f64), scale: f64) {
    ctx.new_path();
    for &(px, py) in points {
        ctx.line_to(
            origin.0 + f64::from(px) * scale,
            origin.1 + f64::from(py) * scale,
        );
    }
}

fn rounded_rect(ctx: &cairo::Context, rect: (f64, f64, f64, f64), radius: f64) {
    let (x, y, w, h) = rect;
    let r = radius.max(0.0).min(w / 2.0).min(h / 2.0);

    ctx.new_sub_path();
    ctx.arc(x + w - r, y + r, r, -TAU / 4.0, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, TAU / 4.0);
    ctx.arc(x + r, y + h - r, r, TAU / 4.0, TAU / 2.0);
    ctx.arc(x + r, y + r, r, TAU / 2.0, TAU * 3.0 / 4.0);
    ctx.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pixels where the as-drawn stroke shows on its own: bright and neutral,
    /// so neither the dark well nor anything the blue accent stroke covers.
    fn raw_only_pixels(level: u8) -> usize {
        let (w, h) = (220, 48);
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).expect("surface");
        {
            let ctx = cairo::Context::new(&surface).expect("context");
            ctx.set_source_rgb(0.0, 0.0, 0.0);
            let _ = ctx.paint();
            draw_smoothing_preview(&ctx, (0.0, 0.0, f64::from(w), f64::from(h)), level);
        }
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("pixels");

        let mut count = 0;
        for row in 0..h as usize {
            for column in 0..w as usize {
                // ARGB32 is a native-endian u32; opaque, so not premultiplied
                // in any way that matters here.
                let offset = row * stride + column * 4;
                let pixel = u32::from_ne_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                let channel = |shift: u32| f64::from((pixel >> shift) & 0xff) / 255.0;
                let (r, g, b) = (channel(16), channel(8), channel(0));
                let brightest = r.max(g).max(b);
                let chroma = brightest - r.min(g).min(b);
                if brightest > 0.35 && chroma < 0.12 {
                    count += 1;
                }
            }
        }
        count
    }

    fn mean_deviation(level: u8) -> f64 {
        let raw = smoothing_preview_sample();
        let smoothed = crate::draw::shape::smooth_path(&raw, level);
        let total: f64 = raw
            .iter()
            .zip(&smoothed)
            .map(|(a, b)| f64::from(a.0 - b.0).hypot(f64::from(a.1 - b.1)))
            .sum();
        total / raw.len() as f64
    }

    #[test]
    fn the_preview_runs_the_release_smoothing_and_off_keeps_the_stroke() {
        let raw = smoothing_preview_sample();

        assert_eq!(raw.len(), SAMPLE_POINTS);
        assert_eq!(crate::draw::shape::smooth_path(&raw, 0), raw);
        assert_eq!(mean_deviation(0), 0.0);
        // Each level takes visibly more of the tremor out than the last.
        for level in 1..=crate::draw::MAX_PEN_SMOOTHING {
            assert!(
                mean_deviation(level) > mean_deviation(level - 1),
                "level {level} smooths more than level {}",
                level - 1
            );
        }
        // Past two preview pixels on average at the top level.
        assert!(mean_deviation(crate::draw::MAX_PEN_SMOOTHING) > 2.0 * SAMPLE_SCALE);
    }

    #[test]
    fn the_sample_fits_its_space() {
        for (x, y) in smoothing_preview_sample() {
            assert!((0..SAMPLE_W as i32).contains(&x), "x {x}");
            assert!((0..SAMPLE_H as i32).contains(&y), "y {y}");
        }
    }

    /// At Off the smoothed stroke lies exactly on the drawn one and covers
    /// it; at the maximum the drawn stroke shows beside it.
    #[test]
    fn the_drawn_stroke_shows_only_where_smoothing_moved_it() {
        assert_eq!(raw_only_pixels(0), 0, "Off draws one stroke");
        let separated = raw_only_pixels(crate::draw::MAX_PEN_SMOOTHING);
        assert!(
            separated > 40,
            "only {separated} raw pixels show at the maximum"
        );
        assert!(
            raw_only_pixels(1) < separated,
            "a light level separates less"
        );
    }
}
