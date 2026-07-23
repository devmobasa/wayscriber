//! Shared substring match-highlighting for search UIs (help overlay,
//! command palette). Matching itself is fuzzy; this only draws a background
//! behind a literal substring range, so a fuzzy-only (subsequence) match
//! simply draws no highlight — the same graceful degradation both callers
//! want.

use super::primitives::text_extents_for;

/// Case-insensitive substring range (byte offsets) of `needle_lower` inside
/// `haystack`, or `None` when it does not appear literally. `needle_lower`
/// must already be lowercased by the caller.
pub(crate) fn find_match_range(haystack: &str, needle_lower: &str) -> Option<(usize, usize)> {
    if needle_lower.is_empty() {
        return None;
    }
    let haystack_lower = haystack.to_ascii_lowercase();
    haystack_lower
        .find(needle_lower)
        .map(|start| (start, start + needle_lower.len()))
}

pub(crate) struct HighlightStyle<'a> {
    pub(crate) font_family: &'a str,
    pub(crate) font_size: f64,
    pub(crate) font_weight: cairo::FontWeight,
    pub(crate) color: [f64; 4],
}

/// Fill a padded rectangle behind the glyphs of `text[range]`, positioned
/// from the text's left edge `x` and `baseline`. The caller draws the text
/// itself over the top afterwards.
pub(crate) fn draw_highlight(
    ctx: &cairo::Context,
    x: f64,
    baseline: f64,
    text: &str,
    range: (usize, usize),
    style: &HighlightStyle<'_>,
) {
    let (start, end) = range;
    if start >= end || end > text.len() {
        return;
    }
    if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return;
    }
    let prefix = &text[..start];
    let matched = &text[start..end];
    if matched.is_empty() {
        return;
    }

    let prefix_extents = text_extents_for(
        ctx,
        style.font_family,
        cairo::FontSlant::Normal,
        style.font_weight,
        style.font_size,
        prefix,
    );
    let match_extents = text_extents_for(
        ctx,
        style.font_family,
        cairo::FontSlant::Normal,
        style.font_weight,
        style.font_size,
        matched,
    );

    let pad_x = 2.0;
    let pad_y = 2.0;
    let highlight_x = x + prefix_extents.width() - pad_x;
    let highlight_y = baseline + match_extents.y_bearing() - pad_y;
    let highlight_width = match_extents.width() + pad_x * 2.0;
    let highlight_height = match_extents.height() + pad_y * 2.0;

    ctx.set_source_rgba(
        style.color[0],
        style.color[1],
        style.color[2],
        style.color[3],
    );
    ctx.rectangle(highlight_x, highlight_y, highlight_width, highlight_height);
    let _ = ctx.fill();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_match_range_matches_against_a_mixed_case_haystack() {
        // The needle is pre-lowercased by contract; only the haystack is
        // lowered here, so a mixed-case label still matches.
        assert_eq!(find_match_range("Toggle Toolbar", "tool"), Some((7, 11)));
        assert_eq!(find_match_range("Toggle Toolbar", "toggle"), Some((0, 6)));
    }

    #[test]
    fn find_match_range_returns_none_for_absent_or_empty() {
        assert_eq!(find_match_range("Zoom In", "xyz"), None);
        assert_eq!(find_match_range("Zoom In", ""), None);
    }

    #[test]
    fn find_match_range_reports_byte_offsets_for_the_first_occurrence() {
        // Two occurrences: the first one wins.
        assert_eq!(find_match_range("a b a", "a"), Some((0, 1)));
    }
}
