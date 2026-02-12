/// Arrowhead triangle geometry used by rendering, hit-testing, and bounds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ArrowheadTriangle {
    pub tip: (f64, f64),
    pub base: (f64, f64),
    pub left: (f64, f64),
    pub right: (f64, f64),
}

/// Calculates arrowhead triangle points matching the renderer's geometry model.
///
/// This helper must remain in sync with `render_arrow` so dirty-region bounds and
/// hit-testing stay aligned with the visual arrowhead.
#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate_arrowhead_triangle_custom(
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
) -> Option<ArrowheadTriangle> {
    let tip_x = tip_x as f64;
    let tip_y = tip_y as f64;
    let tail_x = tail_x as f64;
    let tail_y = tail_y as f64;

    let dir_x = tail_x - tip_x;
    let dir_y = tail_y - tip_y;
    let line_length = (dir_x * dir_x + dir_y * dir_y).sqrt();
    if line_length < 1.0 {
        return None;
    }

    // Direction from tip toward tail.
    let ux = dir_x / line_length;
    let uy = dir_y / line_length;

    // Perpendicular unit vector.
    let px = -uy;
    let py = ux;

    // Keep heads visible for thick strokes but avoid oversized heads on short lines.
    let scaled_length = arrow_length.max(thick * 2.5);
    let effective_length = scaled_length.min(line_length * 0.4);

    let angle_rad = arrow_angle.to_radians();
    let half_base_from_angle = effective_length * angle_rad.tan();
    let half_base = half_base_from_angle.max(thick * 0.6);

    let base_x = tip_x + ux * effective_length;
    let base_y = tip_y + uy * effective_length;

    let left_x = base_x + px * half_base;
    let left_y = base_y + py * half_base;
    let right_x = base_x - px * half_base;
    let right_y = base_y - py * half_base;

    Some(ArrowheadTriangle {
        tip: (tip_x, tip_y),
        base: (base_x, base_y),
        left: (left_x, left_y),
        right: (right_x, right_y),
    })
}
