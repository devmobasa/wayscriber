//! WCAG 2 contrast for the text and surface tokens that are drawn together.
//!
//! [`super::relative_luminance`] weights gamma-encoded channels, which suits
//! its threshold decisions but is not the WCAG metric, so the ratio here
//! linearizes sRGB first. A translucent text token is composited over its
//! surface the way Cairo paints it, on the gamma-encoded values.

use super::{Rgba, Theme, overlay, popup};

/// WCAG AA minimum for body text.
const AA_BODY_TEXT: f64 = 4.5;

fn linear(channel: f64) -> f64 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn wcag_luminance((r, g, b, _): Rgba) -> f64 {
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// `color` painted over the opaque `surface`.
fn over(color: Rgba, surface: Rgba) -> Rgba {
    let alpha = color.3;
    let mix = |fg: f64, bg: f64| alpha * fg + (1.0 - alpha) * bg;

    (
        mix(color.0, surface.0),
        mix(color.1, surface.1),
        mix(color.2, surface.2),
        1.0,
    )
}

fn contrast_ratio(text: Rgba, surface: Rgba) -> f64 {
    assert_eq!(surface.3, 1.0, "contrast is measured on an opaque surface");
    let text = wcag_luminance(over(text, surface));
    let surface = wcag_luminance(surface);
    let (lighter, darker) = if text > surface {
        (text, surface)
    } else {
        (surface, text)
    };

    (lighter + 0.05) / (darker + 0.05)
}

#[test]
fn the_ratio_matches_the_wcag_reference_points() {
    let white = (1.0, 1.0, 1.0, 1.0);
    let grey = 119.0 / 255.0;

    // Black on white is the WCAG maximum; #777 on white is the familiar 4.48:1.
    assert!((contrast_ratio((0.0, 0.0, 0.0, 1.0), white) - 21.0).abs() < 1e-9);
    assert!((contrast_ratio((grey, grey, grey, 1.0), white) - 4.48).abs() < 0.01);
    assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-9);
    // Half-transparent black over white reads as the mid grey it paints.
    assert!(
        (contrast_ratio((0.0, 0.0, 0.0, 0.5), white) - contrast_ratio((0.5, 0.5, 0.5, 1.0), white))
            .abs()
            < 1e-9
    );
}

/// The surfaces a theme's text tokens are drawn on. Cards are translucent
/// washes, so they are measured over the panel they sit on.
fn text_surfaces(theme: &Theme) -> [(&'static str, Rgba); 4] {
    [
        ("surface_pill", theme.surface_pill),
        ("surface_panel", theme.surface_panel),
        ("surface_popover", theme.surface_popover),
        (
            "surface_card over surface_panel",
            over(theme.surface_card, theme.surface_panel),
        ),
    ]
}

#[test]
fn theme_text_tokens_meet_aa_on_every_theme_surface() {
    for (variant, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
        let texts = [
            ("text_primary", theme.text_primary),
            ("text_secondary", theme.text_secondary),
            ("text_tertiary", theme.text_tertiary),
        ];
        for (text_name, text) in texts {
            for (surface_name, surface) in text_surfaces(&theme) {
                let ratio = contrast_ratio(text, surface);

                assert!(
                    ratio >= AA_BODY_TEXT,
                    "{variant} {text_name} on {surface_name} is {ratio:.2}:1"
                );
            }
        }
    }
}

#[test]
fn popup_hint_rows_meet_aa_and_the_dim_hint_stays_dimmer() {
    // The color picker and precise-entry popups draw their key hints on the
    // modal background.
    let surface = popup::bg_modal();
    let dim = contrast_ratio(overlay::TEXT_HINT_DIM, surface);
    let hint = contrast_ratio(overlay::TEXT_HINT, surface);

    assert!(dim >= AA_BODY_TEXT, "TEXT_HINT_DIM is {dim:.2}:1");
    assert!(hint >= AA_BODY_TEXT, "TEXT_HINT is {hint:.2}:1");
    assert!(dim < hint, "TEXT_HINT_DIM must stay dimmer than TEXT_HINT");
}
