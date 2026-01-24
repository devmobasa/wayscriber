use crate::draw::FontDescriptor;
use crate::util::Rect;

use super::text_cache::measure_text_cached;

const STEP_MARKER_PADDING_RATIO: f64 = 0.45;
const STEP_MARKER_PADDING_MIN: f64 = 6.0;
const STEP_MARKER_MIN_RADIUS: f64 = 10.0;

pub(crate) fn step_marker_radius(
    value: u32,
    size: f64,
    font_descriptor: &FontDescriptor,
) -> f64 {
    let text = value.to_string();
    let font_desc_str = font_descriptor.to_pango_string(size);
    let max_dim = measure_text_cached(&text, &font_desc_str, size, None)
        .map(|m| m.ink_width.max(m.ink_height))
        .unwrap_or(size * 0.6);
    let padding = (size * STEP_MARKER_PADDING_RATIO).max(STEP_MARKER_PADDING_MIN);
    (max_dim / 2.0 + padding).max(STEP_MARKER_MIN_RADIUS)
}

pub(crate) fn step_marker_outline_thickness(size: f64) -> f64 {
    (size * 0.12).max(1.5)
}

pub(crate) fn step_marker_bounds(
    x: i32,
    y: i32,
    value: u32,
    size: f64,
    font_descriptor: &FontDescriptor,
) -> Option<Rect> {
    let radius = step_marker_radius(value, size, font_descriptor);
    let outline = step_marker_outline_thickness(size);
    let total = radius + (outline / 2.0);
    let min_x = (x as f64 - total).floor() as i32;
    let max_x = (x as f64 + total).ceil() as i32;
    let min_y = (y as f64 - total).floor() as i32;
    let max_y = (y as f64 + total).ceil() as i32;
    Rect::from_min_max(min_x, min_y, max_x, max_y)
}
