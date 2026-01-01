use crate::draw::{Color, color::*};

/// Maps keyboard characters to colors for quick color switching.
///
/// # Supported Keys (case-insensitive)
/// - `R` → Red
/// - `G` → Green
/// - `B` → Blue
/// - `Y` → Yellow
/// - `O` → Orange
/// - `P` → Pink
/// - `W` → White
/// - `K` → Black (K for blacK, since B is blue)
///
/// # Arguments
/// * `c` - Character key pressed by user
///
/// # Returns
/// - `Some(Color)` if the character maps to a predefined color
/// - `None` if the character doesn't correspond to any color
pub fn key_to_color(c: char) -> Option<Color> {
    match c.to_ascii_uppercase() {
        'R' => Some(RED),
        'G' => Some(GREEN),
        'B' => Some(BLUE),
        'Y' => Some(YELLOW),
        'O' => Some(ORANGE),
        'P' => Some(PINK),
        'W' => Some(WHITE),
        'K' => Some(BLACK), // K for blacK
        _ => None,
    }
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
