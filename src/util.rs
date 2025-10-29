//! Utility functions for colors, geometry, and arrowhead calculations.
//!
//! This module provides:
//! - Key-to-color mapping for keyboard shortcuts (constants moved to draw::color)
//! - Arrowhead geometry calculations
//! - Ellipse bounding box calculations

use crate::draw::{Color, color::*};

// ============================================================================
// Arrowhead Geometry
// ============================================================================

/// Calculates arrowhead points with custom length and angle.
///
/// Creates a V-shaped arrowhead at position (x1, y1) pointing in the direction
/// from (x2, y2) to (x1, y1). The arrowhead length is automatically capped at
/// 30% of the line length to prevent weird-looking arrows on short lines.
///
/// # Arguments
/// * `x1` - Arrowhead tip X coordinate
/// * `y1` - Arrowhead tip Y coordinate
/// * `x2` - Arrow tail X coordinate
/// * `y2` - Arrow tail Y coordinate
/// * `length` - Desired arrowhead length in pixels (will be capped at 30% of line length)
/// * `angle_degrees` - Arrowhead angle in degrees (angle between arrowhead lines and main line)
///
/// # Returns
/// Array of two points `[(left_x, left_y), (right_x, right_y)]` for the arrowhead lines.
/// If the line is too short (< 1 pixel), both points equal (x1, y1).
pub fn calculate_arrowhead_custom(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    length: f64,
    angle_degrees: f64,
) -> [(f64, f64); 2] {
    let dx = (x1 - x2) as f64; // Direction from END to START (reversed)
    let dy = (y1 - y2) as f64;
    let line_length = (dx * dx + dy * dy).sqrt();

    if line_length < 1.0 {
        // Line too short for arrowhead
        return [(x1 as f64, y1 as f64), (x1 as f64, y1 as f64)];
    }

    // Normalize direction vector (pointing from end to start)
    let ux = dx / line_length;
    let uy = dy / line_length;

    // Arrowhead length (max 30% of line length to avoid weird-looking arrows on short lines)
    let arrow_length = length.min(line_length * 0.3);

    // Convert angle to radians
    let angle = angle_degrees.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    // Left side of arrowhead (at START point)
    let left_x = x1 as f64 - arrow_length * (ux * cos_a - uy * sin_a);
    let left_y = y1 as f64 - arrow_length * (uy * cos_a + ux * sin_a);

    // Right side of arrowhead (at START point)
    let right_x = x1 as f64 - arrow_length * (ux * cos_a + uy * sin_a);
    let right_y = y1 as f64 - arrow_length * (uy * cos_a - ux * sin_a);

    [(left_x, left_y), (right_x, right_y)]
}

// ============================================================================
// Color Mapping
// ============================================================================

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

// ============================================================================
// Geometry Utilities
// ============================================================================

/// Clamps a value to a specified range.
///
/// Kept for future use (e.g., dirty region optimization, bounds checking).
#[allow(dead_code)]
pub fn clamp(val: i32, min: i32, max: i32) -> i32 {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

/// Axis-aligned rectangle helper used for dirty region tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    /// Creates a new rectangle. Width/height must be non-negative.
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Option<Self> {
        if width <= 0 || height <= 0 {
            None
        } else {
            Some(Self {
                x,
                y,
                width,
                height,
            })
        }
    }

    /// Builds a rectangle from min/max bounds (inclusive min, exclusive max).
    pub fn from_min_max(min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Option<Self> {
        let width = max_x - min_x;
        let height = max_y - min_y;
        Self::new(min_x, min_y, width, height)
    }

    // /// Expands this rectangle to include another rectangle.
    // pub fn expand_to_include(&mut self, other: Rect) {
    //     let min_x = self.x.min(other.x);
    //     let min_y = self.y.min(other.y);
    //     let max_x = (self.x + self.width).max(other.x + other.width);
    //     let max_y = (self.y + self.height).max(other.y + other.height);

    //     self.x = min_x;
    //     self.y = min_y;
    //     self.width = max_x - min_x;
    //     self.height = max_y - min_y;
    // }

    // /// Returns a rectangle that covers both input rectangles.
    // pub fn union(self, other: Rect) -> Rect {
    //     let mut rect = self;
    //     rect.expand_to_include(other);
    //     rect
    // }

    // /// Expands the rectangle evenly in all directions by `amount`.
    // pub fn inflate(&mut self, amount: i32) {
    //     self.x -= amount;
    //     self.y -= amount;
    //     self.width += amount * 2;
    //     self.height += amount * 2;
    // }

    // /// Clamps the rectangle to the given bounds.
    // pub fn clamp_to_bounds(&mut self, width: i32, height: i32) {
    //     let max_x = (self.x + self.width).clamp(0, width);
    //     let max_y = (self.y + self.height).clamp(0, height);
    //     self.x = self.x.clamp(0, width);
    //     self.y = self.y.clamp(0, height);
    //     self.width = (max_x - self.x).max(0);
    //     self.height = (max_y - self.y).max(0);
    // }

    /// Returns true if rectangle has a positive area.
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// Calculates ellipse parameters from two corner points.
///
/// Converts a drag rectangle (from corner to corner) into ellipse parameters
/// (center point and radii) suitable for Cairo's ellipse rendering.
///
/// # Arguments
/// * `x1` - First corner X coordinate
/// * `y1` - First corner Y coordinate
/// * `x2` - Opposite corner X coordinate
/// * `y2` - Opposite corner Y coordinate
///
/// # Returns
/// Tuple `(cx, cy, rx, ry)` where:
/// - `cx`, `cy` = center point coordinates
/// - `rx` = horizontal radius (half width)
/// - `ry` = vertical radius (half height)
pub fn ellipse_bounds(x1: i32, y1: i32, x2: i32, y2: i32) -> (i32, i32, i32, i32) {
    let cx = (x1 + x2) / 2;
    let cy = (y1 + y2) / 2;
    let rx = ((x2 - x1).abs()) / 2;
    let ry = ((y2 - y1).abs()) / 2;
    (cx, cy, rx, ry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{BLACK, RED, WHITE};

    #[test]
    fn arrowhead_caps_at_thirty_percent_of_line_length() {
        let [(lx, ly), _] = calculate_arrowhead_custom(10, 10, 0, 10, 100.0, 30.0);
        let distance = ((10.0 - lx).powi(2) + (10.0 - ly).powi(2)).sqrt();
        assert!((distance - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn arrowhead_handles_degenerate_lines() {
        let [(lx, ly), (rx, ry)] = calculate_arrowhead_custom(5, 5, 5, 5, 15.0, 45.0);
        assert_eq!((lx, ly), (5.0, 5.0));
        assert_eq!((rx, ry), (5.0, 5.0));
    }

    #[test]
    fn ellipse_bounds_compute_center_and_radii() {
        let (cx, cy, rx, ry) = ellipse_bounds(0, 0, 10, 4);
        assert_eq!((cx, cy, rx, ry), (5, 2, 5, 2));
    }

    #[test]
    fn key_and_name_color_mappings_round_trip() {
        assert_eq!(key_to_color('r').unwrap(), RED);
        assert_eq!(key_to_color('K').unwrap(), BLACK);
        assert!(key_to_color('x').is_none());
        assert_eq!(name_to_color("white").unwrap(), WHITE);
        assert!(name_to_color("chartreuse").is_none());
    }

    #[test]
    fn color_to_name_matches_known_colors() {
        assert_eq!(color_to_name(&RED), "Red");
        assert_eq!(color_to_name(&BLACK), "Black");
        assert_eq!(
            color_to_name(&Color {
                r: 0.42,
                g: 0.42,
                b: 0.42,
                a: 1.0
            }),
            "Custom"
        );
    }
}
