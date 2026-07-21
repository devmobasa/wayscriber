//! RGBA color type and predefined color constants.

use serde::{Deserialize, Serialize};

/// Represents an RGBA color with floating-point components.
///
/// All components are in the range 0.0 (minimum) to 1.0 (maximum).
///
/// # Examples
///
/// ```
/// use wayscriber::draw::Color;
/// let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
/// let semi_transparent_blue = Color { r: 0.0, g: 0.0, b: 1.0, a: 0.5 };
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    /// Red component (0.0 = no red, 1.0 = full red)
    pub r: f64,
    /// Green component (0.0 = no green, 1.0 = full green)
    pub g: f64,
    /// Blue component (0.0 = no blue, 1.0 = full blue)
    pub b: f64,
    /// Alpha/transparency (0.0 = fully transparent, 1.0 = fully opaque)
    pub a: f64,
}

impl Color {
    /// Creates a new color from RGBA components.
    ///
    /// All values should be in the range 0.0 to 1.0.
    /// This method is kept for future extensibility (custom colors in config file).
    #[allow(dead_code)]
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }
}

/// Predefined red color (R=1.0, G=0.0, B=0.0)
pub const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// Predefined green color (R=0.0, G=1.0, B=0.0)
pub const GREEN: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};

/// Predefined blue color (R=0.0, G=0.0, B=1.0)
pub const BLUE: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

/// Predefined yellow color (R=1.0, G=1.0, B=0.0)
pub const YELLOW: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};

/// Predefined orange color (R=1.0, G=0.5, B=0.0)
pub const ORANGE: Color = Color {
    r: 1.0,
    g: 0.5,
    b: 0.0,
    a: 1.0,
};

/// Predefined pink/magenta color (R=1.0, G=0.0, B=1.0)
pub const PINK: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

/// Predefined white color (R=1.0, G=1.0, B=1.0)
pub const WHITE: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// Predefined black color (R=0.0, G=0.0, B=0.0)
pub const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// Fully transparent color - kept for future use (e.g., effects, config file)
#[allow(dead_code)]
pub const TRANSPARENT: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

/// Converts 8-bit RGB components (as in `#RRGGBB` hex) to an opaque [`Color`].
const fn rgb8(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f64 / 255.0,
        g: g as f64 / 255.0,
        b: b as f64 / 255.0,
        a: 1.0,
    }
}

// Tuned quick-color palette.
//
// These constants are the single source of truth for the named color table
// (`util::name_to_color`), the built-in quick color slot defaults, and the
// board auto-adjust pen colors. Deriving all of them from the same values
// keeps exact color equality intact, which swatch selection relies on.

/// Tuned palette red `#F5333F` (named color "red", quick color slot 0)
pub const PALETTE_RED: Color = rgb8(0xF5, 0x33, 0x3F);

/// Tuned palette green `#2EC27E` (named color "green", quick color slot 1)
pub const PALETTE_GREEN: Color = rgb8(0x2E, 0xC2, 0x7E);

/// Tuned palette blue `#3584E4` (named color "blue", quick color slot 2)
pub const PALETTE_BLUE: Color = rgb8(0x35, 0x84, 0xE4);

/// Tuned palette yellow `#F6D32D` (named color "yellow", quick color slot 3)
pub const PALETTE_YELLOW: Color = rgb8(0xF6, 0xD3, 0x2D);

/// Tuned palette orange `#FF7800` (named color "orange", quick color slot 4)
pub const PALETTE_ORANGE: Color = rgb8(0xFF, 0x78, 0x00);

/// Tuned palette pink `#C061CB` (named color "pink", quick color slot 5)
pub const PALETTE_PINK: Color = rgb8(0xC0, 0x61, 0xCB);

/// Tuned palette white `#FFFFFF` (named color "white", quick color slot 6)
pub const PALETTE_WHITE: Color = WHITE;

/// Tuned palette black `#241F31` (named color "black", quick color slot 7)
pub const PALETTE_BLACK: Color = rgb8(0x24, 0x1F, 0x31);

/// Convert an HSV triple (all components in 0.0–1.0) to an opaque RGB color.
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Color {
    let h = (h - h.floor()).clamp(0.0, 1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color { r, g, b, a: 1.0 }
}

/// Convert RGB components (0.0–1.0) to an HSV triple in 0.0–1.0.
/// Hue is 0.0 for grays (delta = 0); callers that need hue continuity
/// across gray colors must remember the last meaningful hue themselves.
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

#[cfg(test)]
mod hsv_tests {
    use super::*;

    #[test]
    fn hsv_round_trips_through_rgb() {
        for &(h, s, v) in &[
            (0.0, 1.0, 1.0),
            (0.33, 0.5, 0.8),
            (0.66, 1.0, 0.4),
            (0.91, 0.2, 0.95),
        ] {
            let color = hsv_to_rgb(h, s, v);
            let (rh, rs, rv) = rgb_to_hsv(color.r, color.g, color.b);
            assert!((rh - h).abs() < 1e-6, "hue: {rh} vs {h}");
            assert!((rs - s).abs() < 1e-6, "sat: {rs} vs {s}");
            assert!((rv - v).abs() < 1e-6, "val: {rv} vs {v}");
        }
    }

    #[test]
    fn grays_report_zero_hue_and_saturation() {
        let (h, s, v) = rgb_to_hsv(0.5, 0.5, 0.5);
        assert_eq!(h, 0.0);
        assert_eq!(s, 0.0);
        assert!((v - 0.5).abs() < 1e-9);
    }
}
