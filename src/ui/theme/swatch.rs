//! Keeping color swatches visible against the chrome they sit on.
//!
//! A quick-color swatch is the only picture of its color, so a fill that
//! melts into the chrome behind it (the palette's black on the dark toolbar,
//! white on light chrome) reads as a gap rather than a choice. Every surface
//! that paints swatches asks the same question here, measured the same way.

use super::{Rgb, Rgba};

/// Contrast a swatch keeps against its chrome before it gets an outline.
///
/// WCAG 2 asks 3:1 of the graphical objects needed to understand a control
/// (success criterion 1.4.11, non-text contrast).
pub const SWATCH_MIN_CONTRAST: f64 = 3.0;

/// Ring around a swatch that is too close to dark chrome.
pub const SWATCH_OUTLINE_ON_DARK: Rgba = (1.0, 1.0, 1.0, 0.6);

/// Ring around a swatch that is too close to light chrome.
pub const SWATCH_OUTLINE_ON_LIGHT: Rgba = (0.0, 0.0, 0.0, 0.5);

/// Stroke width of the contrast ring, in logical pixels.
pub const SWATCH_OUTLINE_WIDTH: f64 = 1.5;

/// WCAG relative luminance of an sRGB color (channels 0.0-1.0), with the
/// transfer curve removed first.
///
/// Not [`super::relative_luminance`], which weights the channels without
/// linearizing them: close enough to pick light or dark chrome, but it rates
/// the palette's black three times brighter than it is.
pub fn srgb_luminance(color: Rgb) -> f64 {
    fn linear(channel: f64) -> f64 {
        let channel = channel.clamp(0.0, 1.0);
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * linear(color.0) + 0.7152 * linear(color.1) + 0.0722 * linear(color.2)
}

/// WCAG contrast ratio between two colors, from 1.0 (identical) to 21.0.
pub fn contrast_ratio(first: Rgb, second: Rgb) -> f64 {
    let first = srgb_luminance(first);
    let second = srgb_luminance(second);
    let (light, dark) = if first >= second {
        (first, second)
    } else {
        (second, first)
    };
    (light + 0.05) / (dark + 0.05)
}

/// The ring that keeps a swatch filled with `fill` visible on `background`,
/// or `None` when the fill already stands out.
///
/// A translucent fill is judged as it shows over the chrome. The ring takes
/// the tone that contrasts with the chrome, not with the fill: it has to be
/// seen against the chrome to draw the swatch's edge.
pub fn swatch_contrast_outline(fill: Rgba, background: Rgb) -> Option<Rgba> {
    let alpha = fill.3.clamp(0.0, 1.0);
    let shown = (
        fill.0 * alpha + background.0 * (1.0 - alpha),
        fill.1 * alpha + background.1 * (1.0 - alpha),
        fill.2 * alpha + background.2 * (1.0 - alpha),
    );
    if contrast_ratio(shown, background) >= SWATCH_MIN_CONTRAST {
        return None;
    }

    let dark_chrome =
        contrast_ratio(background, (1.0, 1.0, 1.0)) >= contrast_ratio(background, (0.0, 0.0, 0.0));
    Some(if dark_chrome {
        SWATCH_OUTLINE_ON_DARK
    } else {
        SWATCH_OUTLINE_ON_LIGHT
    })
}

/// Edge stroke for a swatch: the contrast ring when the fill needs one,
/// otherwise the surface's own quiet `hairline` at `hairline_width`.
pub fn swatch_edge_stroke(
    fill: Rgba,
    background: Rgb,
    hairline: Rgba,
    hairline_width: f64,
) -> (Rgba, f64) {
    match swatch_contrast_outline(fill, background) {
        Some(outline) => (outline, SWATCH_OUTLINE_WIDTH),
        None => (hairline, hairline_width),
    }
}

/// The RGB of a chrome surface token, for use as a swatch `background`.
pub const fn chrome_rgb(surface: Rgba) -> Rgb {
    (surface.0, surface.1, surface.2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::color::{
        PALETTE_BLACK, PALETTE_BLUE, PALETTE_GREEN, PALETTE_ORANGE, PALETTE_PINK, PALETTE_RED,
        PALETTE_WHITE, PALETTE_YELLOW,
    };
    use crate::ui::theme::{Theme, toolbar};

    fn rgba(color: crate::domain::Color) -> Rgba {
        (color.r, color.g, color.b, color.a)
    }

    #[test]
    fn contrast_ratio_spans_the_wcag_range() {
        assert!((contrast_ratio((0.0, 0.0, 0.0), (1.0, 1.0, 1.0)) - 21.0).abs() < 1e-9);
        assert!((contrast_ratio((0.4, 0.2, 0.6), (0.4, 0.2, 0.6)) - 1.0).abs() < 1e-9);
        assert_eq!(
            contrast_ratio((0.1, 0.1, 0.1), (0.9, 0.9, 0.9)),
            contrast_ratio((0.9, 0.9, 0.9), (0.1, 0.1, 0.1)),
            "order does not matter"
        );
    }

    #[test]
    fn only_the_palette_black_needs_a_ring_on_the_dark_toolbar() {
        let panel = chrome_rgb(toolbar::COLOR_PANEL_BACKGROUND);

        assert_eq!(
            swatch_contrast_outline(rgba(PALETTE_BLACK), panel),
            Some(SWATCH_OUTLINE_ON_DARK)
        );
        for color in [
            PALETTE_RED,
            PALETTE_GREEN,
            PALETTE_BLUE,
            PALETTE_YELLOW,
            PALETTE_ORANGE,
            PALETTE_PINK,
            PALETTE_WHITE,
        ] {
            assert_eq!(
                swatch_contrast_outline(rgba(color), panel),
                None,
                "{color:?}"
            );
        }
    }

    #[test]
    fn white_takes_a_dark_ring_on_light_chrome() {
        let light = chrome_rgb(Theme::light().surface_pill);

        assert_eq!(
            swatch_contrast_outline(rgba(PALETTE_WHITE), light),
            Some(SWATCH_OUTLINE_ON_LIGHT)
        );
        assert_eq!(swatch_contrast_outline(rgba(PALETTE_BLACK), light), None);
    }

    #[test]
    fn a_translucent_fill_is_judged_as_it_shows_over_the_chrome() {
        let panel = chrome_rgb(toolbar::COLOR_PANEL_BACKGROUND);

        // Opaque white stands out; nearly transparent white is the panel.
        assert_eq!(swatch_contrast_outline((1.0, 1.0, 1.0, 1.0), panel), None);
        assert_eq!(
            swatch_contrast_outline((1.0, 1.0, 1.0, 0.05), panel),
            Some(SWATCH_OUTLINE_ON_DARK)
        );
    }

    #[test]
    fn the_edge_stroke_keeps_the_quiet_hairline_where_no_ring_is_needed() {
        let panel = chrome_rgb(toolbar::COLOR_PANEL_BACKGROUND);
        let hairline = toolbar::COLOR_SWATCH_HAIRLINE;

        assert_eq!(
            swatch_edge_stroke(rgba(PALETTE_RED), panel, hairline, 1.0),
            (hairline, 1.0)
        );
        assert_eq!(
            swatch_edge_stroke(rgba(PALETTE_BLACK), panel, hairline, 1.0),
            (SWATCH_OUTLINE_ON_DARK, SWATCH_OUTLINE_WIDTH)
        );
    }
}
