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
