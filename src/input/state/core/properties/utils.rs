use crate::config::QuickColorPaletteEntry;
use crate::draw::Color;
use crate::time_utils::format_unix_millis;

/// The palette slot `offset` steps from `current`, wrapping at both ends.
///
/// A color the palette does not hold starts from outside it: a forward step
/// lands on the first slot and a backward step on the last, rather than
/// skipping whichever slot sits next to an arbitrary starting guess.
pub(super) fn palette_step(len: usize, current: Option<usize>, offset: i32) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let Some(index) = current else {
        return Some(if offset < 0 { len - 1 } else { 0 });
    };
    Some(cycle_index(index, len, offset))
}

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

pub(super) fn color_palette_index(
    palette: &[QuickColorPaletteEntry],
    color: Color,
) -> Option<usize> {
    palette
        .iter()
        .position(|entry| color_eq(&entry.color, &color))
}

/// The palette's name for `color`, or "Custom" for a color it does not hold.
pub(super) fn color_label(palette: &[QuickColorPaletteEntry], color: Color) -> String {
    color_palette_index(palette, color)
        .map(|index| palette[index].label.clone())
        .unwrap_or_else(|| "Custom".to_string())
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
    use crate::domain::color::{PALETTE_BLUE, PALETTE_GREEN, PALETTE_RED};

    fn palette() -> Vec<QuickColorPaletteEntry> {
        [
            ("Red", PALETTE_RED),
            ("Green", PALETTE_GREEN),
            ("Blue", PALETTE_BLUE),
        ]
        .into_iter()
        .map(|(label, color)| QuickColorPaletteEntry {
            label: label.to_string(),
            color,
        })
        .collect()
    }

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
    fn palette_step_enters_the_palette_at_the_near_end_from_a_custom_color() {
        assert_eq!(palette_step(3, None, 1), Some(0));
        assert_eq!(palette_step(3, None, -1), Some(2));
        assert_eq!(palette_step(3, Some(2), 1), Some(0));
        assert_eq!(palette_step(0, None, 1), None);
    }

    #[test]
    fn color_palette_index_and_label_use_approximate_rgb_matching() {
        let near_green = Color {
            r: PALETTE_GREEN.r - 0.009,
            g: PALETTE_GREEN.g,
            b: PALETTE_GREEN.b + 0.009,
            a: 0.25,
        };

        assert_eq!(color_palette_index(&palette(), near_green), Some(1));
        assert_eq!(color_label(&palette(), near_green), "Green");
    }

    #[test]
    fn color_label_returns_custom_outside_palette_tolerance() {
        let custom = Color {
            r: 0.13,
            g: 0.27,
            b: 0.61,
            a: 1.0,
        };

        assert_eq!(color_palette_index(&palette(), custom), None);
        assert_eq!(color_label(&palette(), custom), "Custom");
    }

    #[test]
    fn approx_eq_uses_stable_threshold_comparisons() {
        assert!(approx_eq(&1.0, &1.009));
        assert!(!approx_eq(&1.0, &1.011));
    }
}
