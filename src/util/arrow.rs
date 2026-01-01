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
