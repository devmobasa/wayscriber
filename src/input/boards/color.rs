use crate::config::BoardColorConfig;
use crate::domain::Color;
use crate::domain::color::{PALETTE_BLACK, PALETTE_WHITE};

pub fn runtime_contrast_pen_color(background: Color) -> Color {
    let [r, g, b] = runtime_contrast_pen_rgb([background.r, background.g, background.b]);
    Color { r, g, b, a: 1.0 }
}

/// Picks the auto-adjust pen color for a solid board background. The tuned
/// palette constants are used so the assigned pen bit-matches the default
/// black/white quick color slots and keeps swatch selection intact.
pub fn runtime_contrast_pen_rgb(background: [f64; 3]) -> [f64; 3] {
    let luminance = 0.2126 * background[0] + 0.7152 * background[1] + 0.0722 * background[2];
    if luminance > 0.5 {
        [PALETTE_BLACK.r, PALETTE_BLACK.g, PALETTE_BLACK.b]
    } else {
        [PALETTE_WHITE.r, PALETTE_WHITE.g, PALETTE_WHITE.b]
    }
}

pub fn board_color_from_config(config: &BoardColorConfig) -> Color {
    let rgb = config.rgb();
    Color {
        r: rgb[0],
        g: rgb[1],
        b: rgb[2],
        a: 1.0,
    }
}

pub fn board_color_to_config(color: Color) -> BoardColorConfig {
    BoardColorConfig::Rgb([color.r, color.g, color.b])
}

pub fn clamp_board_rgb(mut rgb: [f64; 3]) -> ([f64; 3], bool) {
    let mut clamped = false;
    for component in &mut rgb {
        if !(0.0..=1.0).contains(component) {
            *component = (*component).clamp(0.0, 1.0);
            clamped = true;
        }
    }
    (rgb, clamped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_contrast_uses_weighted_luminance_threshold() {
        assert_eq!(
            runtime_contrast_pen_rgb([0.5, 0.5, 0.5]),
            [PALETTE_WHITE.r, PALETTE_WHITE.g, PALETTE_WHITE.b]
        );
        assert_eq!(
            runtime_contrast_pen_rgb([0.51, 0.51, 0.51]),
            [PALETTE_BLACK.r, PALETTE_BLACK.g, PALETTE_BLACK.b]
        );
    }

    #[test]
    fn auto_adjust_pen_colors_bit_match_default_quick_color_slots() {
        let palette = crate::config::QuickColorPalette::default();

        let light_board_pen = runtime_contrast_pen_color(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });
        assert_eq!(
            Some(light_board_pen),
            palette.color_for_index(7),
            "light board auto-adjust pen must bit-match the black quick color slot"
        );

        let dark_board_pen = runtime_contrast_pen_color(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        assert_eq!(
            Some(dark_board_pen),
            palette.color_for_index(6),
            "dark board auto-adjust pen must bit-match the white quick color slot"
        );
    }

    #[test]
    fn clamp_board_rgb_reports_whether_any_component_changed() {
        assert_eq!(clamp_board_rgb([0.2, 0.3, 0.4]), ([0.2, 0.3, 0.4], false));
        assert_eq!(clamp_board_rgb([-0.5, 0.3, 1.4]), ([0.0, 0.3, 1.0], true));
    }
}
