use crate::draw::{Color, color::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigHexColorError {
    MissingHash,
    InvalidLength,
    InvalidDigits,
}

/// Maps color name strings to Color values.
///
/// Used by the configuration system to parse color names from the config file.
///
/// # Supported Names (case-insensitive)
/// - "red", "green", "blue", "yellow", "orange", "pink", "white", "black"
///
/// # Arguments
/// * `name` - Color name string
///
/// # Returns
/// - `Some(Color)` if the name matches a predefined color
/// - `None` if the name is not recognized
pub fn name_to_color(name: &str) -> Option<Color> {
    match name.to_lowercase().as_str() {
        "red" => Some(RED),
        "green" => Some(GREEN),
        "blue" => Some(BLUE),
        "yellow" => Some(YELLOW),
        "orange" => Some(ORANGE),
        "pink" => Some(PINK),
        "white" => Some(WHITE),
        "black" => Some(BLACK),
        _ => None,
    }
}

/// Parses config-facing hex colors.
///
/// Config hex intentionally accepts only `#RRGGBB`. Runtime UI helpers may
/// support looser input forms, but config files should stay predictable.
pub fn parse_config_hex_color(value: &str) -> Result<Color, ConfigHexColorError> {
    let trimmed = value.trim();
    let Some(hex) = trimmed.strip_prefix('#') else {
        return Err(ConfigHexColorError::MissingHash);
    };
    if hex.len() != 6 {
        return Err(ConfigHexColorError::InvalidLength);
    }
    if !hex.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ConfigHexColorError::InvalidDigits);
    }
    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ConfigHexColorError::InvalidDigits)?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ConfigHexColorError::InvalidDigits)?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ConfigHexColorError::InvalidDigits)?;
    Ok(Color {
        r: f64::from(r) / 255.0,
        g: f64::from(g) / 255.0,
        b: f64::from(b) / 255.0,
        a: 1.0,
    })
}

/// Maps a Color value to its human-readable name.
///
/// Uses approximate matching (threshold-based) to identify colors.
/// Used by the UI status bar to display the current color name.
///
/// # Arguments
/// * `color` - The color to identify
///
/// # Returns
/// A static string with the color name, or "Custom" if the color doesn't
/// match any predefined color.
pub fn color_to_name(color: &Color) -> &'static str {
    // Match colors approximately with 0.1 tolerance
    if color.r > 0.9 && color.g < 0.1 && color.b < 0.1 {
        "Red"
    } else if color.r < 0.1 && color.g > 0.9 && color.b < 0.1 {
        "Green"
    } else if color.r < 0.1 && color.g < 0.1 && color.b > 0.9 {
        "Blue"
    } else if color.r > 0.9 && color.g > 0.9 && color.b < 0.1 {
        "Yellow"
    } else if color.r > 0.9 && (0.4..=0.6).contains(&color.g) && color.b < 0.1 {
        "Orange"
    } else if color.r > 0.9 && color.g < 0.1 && color.b > 0.9 {
        "Pink"
    } else if color.r > 0.9 && color.g > 0.9 && color.b > 0.9 {
        "White"
    } else if color.r < 0.1 && color.g < 0.1 && color.b < 0.1 {
        "Black"
    } else {
        "Custom"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_to_color_is_case_insensitive() {
        assert_eq!(name_to_color("Orange"), Some(ORANGE));
        assert_eq!(name_to_color("WHITE"), Some(WHITE));
    }

    #[test]
    fn parse_config_hex_color_accepts_hash_rrggbb_only() {
        let color = parse_config_hex_color("#FF8040").expect("valid hex color");
        assert_eq!(
            color,
            Color {
                r: 1.0,
                g: 128.0 / 255.0,
                b: 64.0 / 255.0,
                a: 1.0,
            }
        );
    }

    #[test]
    fn parse_config_hex_color_rejects_loose_runtime_forms() {
        assert_eq!(
            parse_config_hex_color("FF8040"),
            Err(ConfigHexColorError::MissingHash)
        );
        assert_eq!(
            parse_config_hex_color("0xFF8040"),
            Err(ConfigHexColorError::MissingHash)
        );
        assert_eq!(
            parse_config_hex_color("#F84"),
            Err(ConfigHexColorError::InvalidLength)
        );
    }

    #[test]
    fn parse_config_hex_color_rejects_invalid_hex_digits() {
        assert_eq!(
            parse_config_hex_color("#GG0000"),
            Err(ConfigHexColorError::InvalidDigits)
        );
    }

    #[test]
    fn color_to_name_matches_approximate_orange_band() {
        let color = Color {
            r: 0.95,
            g: 0.5,
            b: 0.05,
            a: 1.0,
        };
        assert_eq!(color_to_name(&color), "Orange");
    }

    #[test]
    fn color_to_name_returns_custom_outside_thresholds() {
        let color = Color {
            r: 0.85,
            g: 0.5,
            b: 0.05,
            a: 1.0,
        };
        assert_eq!(color_to_name(&color), "Custom");
    }
}
