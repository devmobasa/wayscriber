use crate::draw::{BLACK, BLUE, Color, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
use crate::time_utils::format_unix_millis;

pub(super) const SELECTION_COLORS: [(&str, Color); 8] = [
    ("Red", RED),
    ("Green", GREEN),
    ("Blue", BLUE),
    ("Yellow", YELLOW),
    ("Orange", ORANGE),
    ("Pink", PINK),
    ("White", WHITE),
    ("Black", BLACK),
];

pub(super) fn cycle_index(index: usize, len: usize, offset: i32) -> usize {
    if len == 0 {
        return 0;
    }
    let len_i = len as i32;
    let mut next = index as i32 + offset;
    if next < 0 {
        next = (next % len_i + len_i) % len_i;
    } else {
        next %= len_i;
    }
    next as usize
}

pub(super) fn color_palette_index(color: Color) -> Option<usize> {
    SELECTION_COLORS
        .iter()
        .position(|(_, candidate)| color_eq(candidate, &color))
}

pub(super) fn color_label(color: Color) -> String {
    for (name, candidate) in SELECTION_COLORS {
        if color_eq(&candidate, &color) {
            return name.to_string();
        }
    }
    "Custom".to_string()
}

pub(super) fn color_eq(a: &Color, b: &Color) -> bool {
    approx_eq(&a.r, &b.r) && approx_eq(&a.g, &b.g) && approx_eq(&a.b, &b.b)
}

pub(super) fn approx_eq(a: &f64, b: &f64) -> bool {
    (*a - *b).abs() <= 0.01
}

pub(super) fn format_timestamp(ms: u64) -> Option<String> {
    format_unix_millis(ms, "%Y-%m-%d %H:%M")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_index_wraps_forward_and_backward() {
        assert_eq!(cycle_index(0, 8, -1), 7);
        assert_eq!(cycle_index(7, 8, 1), 0);
        assert_eq!(cycle_index(2, 8, 10), 4);
    }

    #[test]
    fn cycle_index_returns_zero_for_empty_palettes() {
        assert_eq!(cycle_index(5, 0, 3), 0);
    }

    #[test]
    fn color_palette_index_and_label_use_approximate_rgb_matching() {
        let near_red = Color {
            r: RED.r - 0.009,
            g: RED.g,
            b: RED.b + 0.009,
            a: 0.25,
        };

        assert_eq!(color_palette_index(near_red), Some(0));
        assert_eq!(color_label(near_red), "Red");
    }

    #[test]
    fn color_label_returns_custom_outside_palette_tolerance() {
        let custom = Color {
            r: 0.13,
            g: 0.27,
            b: 0.61,
            a: 1.0,
        };

        assert_eq!(color_palette_index(custom), None);
        assert_eq!(color_label(custom), "Custom");
    }

    #[test]
    fn approx_eq_uses_stable_threshold_comparisons() {
        assert!(approx_eq(&1.0, &1.009));
        assert!(!approx_eq(&1.0, &1.011));
    }
}
