//! Angle <-> thickness mapping for the outermost size ring.
//!
//! The gauge is the drag-along-arc inverse of the retired center thickness
//! gauge: value grows linearly with clockwise arc position between
//! [`MIN_STROKE_THICKNESS`] and [`MAX_STROKE_THICKNESS`]. The arc leaves a
//! gap at the bottom (classic gauge form): minimum at bottom-left, maximum at
//! bottom-right, clockwise over the top.

use std::f64::consts::PI;

use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

/// Arc start (minimum value), in raw atan2/Cairo angle space (0 = east,
/// clockwise-positive in y-down screen coordinates): bottom-left.
pub const SIZE_RING_ARC_START: f64 = 0.75 * PI;

/// Angular span of the gauge: 270 degrees clockwise from the start, ending
/// bottom-right and leaving the bottom quarter open.
pub const SIZE_RING_ARC_SPAN: f64 = 1.5 * PI;

/// Angle (raw atan2 space) for a thickness value, clamped to the gauge span.
pub fn size_ring_angle_for_value(value: f64) -> f64 {
    let frac = ((value - MIN_STROKE_THICKNESS) / (MAX_STROKE_THICKNESS - MIN_STROKE_THICKNESS))
        .clamp(0.0, 1.0);
    SIZE_RING_ARC_START + frac * SIZE_RING_ARC_SPAN
}

/// Thickness value for an angle (raw atan2 space): the exact inverse of
/// [`size_ring_angle_for_value`] within the span. Angles in the bottom gap
/// snap to the nearest end so drags past the ends pin to min/max.
pub fn size_ring_value_for_angle(angle: f64) -> f64 {
    let rel = normalize_rel(angle);
    let rel = if rel <= SIZE_RING_ARC_SPAN {
        rel
    } else if rel - SIZE_RING_ARC_SPAN <= (2.0 * PI - SIZE_RING_ARC_SPAN) / 2.0 {
        SIZE_RING_ARC_SPAN
    } else {
        0.0
    };
    MIN_STROKE_THICKNESS
        + (rel / SIZE_RING_ARC_SPAN) * (MAX_STROKE_THICKNESS - MIN_STROKE_THICKNESS)
}

/// Whether an angle (raw atan2 space) falls inside the gauge arc (hit-testing
/// excludes the bottom gap).
pub fn size_ring_angle_in_span(angle: f64) -> bool {
    normalize_rel(angle) <= SIZE_RING_ARC_SPAN
}

/// Angle relative to the arc start, normalized to [0, 2*PI).
fn normalize_rel(angle: f64) -> f64 {
    let two_pi = 2.0 * PI;
    ((angle - SIZE_RING_ARC_START) % two_pi + two_pi) % two_pi
}
