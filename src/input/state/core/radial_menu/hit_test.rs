use super::state::sub_ring_child_count;
use super::{COLOR_SEGMENT_COUNT, RadialMenuLayout, RadialSegmentId, TOOL_SEGMENT_COUNT};
use std::f64::consts::PI;

/// Perform a hit-test on the radial menu and return the segment under (x, y).
pub fn hit_test_radial(
    layout: &RadialMenuLayout,
    expanded_sub_ring: Option<u8>,
    x: f64,
    y: f64,
) -> Option<RadialSegmentId> {
    let dx = x - layout.center_x;
    let dy = y - layout.center_y;
    let dist = (dx * dx + dy * dy).sqrt();

    // Center circle
    if dist <= layout.center_radius {
        return Some(RadialSegmentId::Center);
    }

    // Compute tool angle: 0 at top, clockwise, in [0, 2*PI).
    // Add half-segment offset so boundaries align with rendered tool wedges.
    let tool_half_seg = PI / TOOL_SEGMENT_COUNT as f64; // == tool_seg_angle / 2
    let tool_angle = normalize_angle(dy.atan2(dx) + PI / 2.0 + tool_half_seg);

    // Sub-ring band (checked before tool ring when a sub-ring is expanded)
    if let Some(parent_idx) = expanded_sub_ring
        && dist >= layout.sub_inner
        && dist <= layout.sub_outer
    {
        let child_count = sub_ring_child_count(parent_idx);
        if child_count > 0 {
            let segment_angle = 2.0 * PI / TOOL_SEGMENT_COUNT as f64;
            let parent_start = segment_angle * parent_idx as f64;
            let parent_end = parent_start + segment_angle;
            if tool_angle >= parent_start && tool_angle < parent_end {
                let child_angle = segment_angle / child_count as f64;
                let offset = tool_angle - parent_start;
                let child_idx = (offset / child_angle).floor() as u8;
                let child_idx = child_idx.min(child_count as u8 - 1);
                return Some(RadialSegmentId::SubTool(parent_idx, child_idx));
            }
        }
    }

    // Tool ring
    if dist >= layout.tool_inner && dist <= layout.tool_outer {
        let idx = angle_to_segment(tool_angle, TOOL_SEGMENT_COUNT);
        return Some(RadialSegmentId::Tool(idx));
    }

    // Color ring
    if dist >= layout.color_inner && dist <= layout.color_outer {
        // Color ring uses its own segment size and render offset.
        let color_half_seg = PI / COLOR_SEGMENT_COUNT as f64; // == color_seg_angle / 2
        let color_angle = normalize_angle(dy.atan2(dx) + PI / 2.0 + color_half_seg);
        let idx = angle_to_segment(color_angle, COLOR_SEGMENT_COUNT);
        return Some(RadialSegmentId::Color(idx));
    }

    None
}

/// Map an angle (0 at top, clockwise) to a segment index.
fn angle_to_segment(angle: f64, count: usize) -> u8 {
    let segment_angle = 2.0 * PI / count as f64;
    let idx = (angle / segment_angle).floor() as u8;
    idx.min(count as u8 - 1)
}

/// Normalize an angle to the range [0, 2*PI).
fn normalize_angle(a: f64) -> f64 {
    let two_pi = 2.0 * PI;
    ((a % two_pi) + two_pi) % two_pi
}
