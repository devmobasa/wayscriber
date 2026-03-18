use crate::draw::FontDescriptor;
use crate::util::Rect;

use super::text_cache::measure_text_cached;

const STEP_MARKER_PADDING_RATIO: f64 = 0.45;
const STEP_MARKER_PADDING_MIN: f64 = 6.0;
const STEP_MARKER_MIN_RADIUS: f64 = 10.0;

pub(crate) fn step_marker_radius(value: u32, size: f64, font_descriptor: &FontDescriptor) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_marker_outline_thickness_has_minimum_floor() {
        assert_eq!(step_marker_outline_thickness(1.0), 1.5);
        assert_eq!(step_marker_outline_thickness(20.0), 2.4);
    }

    #[test]
    fn step_marker_radius_grows_with_font_size() {
        let font = FontDescriptor::default();
        assert!(step_marker_radius(1, 32.0, &font) > step_marker_radius(1, 12.0, &font));
    }

    #[test]
    fn step_marker_radius_grows_for_multi_digit_labels() {
        let font = FontDescriptor::default();
        assert!(step_marker_radius(88, 18.0, &font) >= step_marker_radius(8, 18.0, &font));
    }

    #[test]
    fn step_marker_bounds_are_centered_around_marker_position() {
        let font = FontDescriptor::default();
        let bounds = step_marker_bounds(50, 75, 3, 18.0, &font).expect("step marker bounds");

        assert!(bounds.contains(50, 75));
        assert!(bounds.width > 0);
        assert!(bounds.height > 0);
    }
}
