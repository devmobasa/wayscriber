use crate::config::BoardColorConfig;
use crate::domain::Color;
use crate::domain::color::{BLACK, WHITE};

pub fn runtime_contrast_pen_color(background: Color) -> Color {
    let [r, g, b] = runtime_contrast_pen_rgb([background.r, background.g, background.b]);
    Color { r, g, b, a: 1.0 }
}

pub fn runtime_contrast_pen_rgb(background: [f64; 3]) -> [f64; 3] {
    let luminance = 0.2126 * background[0] + 0.7152 * background[1] + 0.0722 * background[2];
    if luminance > 0.5 {
        [BLACK.r, BLACK.g, BLACK.b]
    } else {
        [WHITE.r, WHITE.g, WHITE.b]
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
        assert_eq!(runtime_contrast_pen_rgb([0.5, 0.5, 0.5]), [1.0, 1.0, 1.0]);
        assert_eq!(
            runtime_contrast_pen_rgb([0.51, 0.51, 0.51]),
            [0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn clamp_board_rgb_reports_whether_any_component_changed() {
        assert_eq!(clamp_board_rgb([0.2, 0.3, 0.4]), ([0.2, 0.3, 0.4], false));
        assert_eq!(clamp_board_rgb([-0.5, 0.3, 1.4]), ([0.0, 0.3, 1.0], true));
    }
}
