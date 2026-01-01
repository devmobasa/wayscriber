use super::super::primitives::text_extents_for;
use super::types::Row;

const ELLIPSIS: &str = "\u{2026}";

pub(crate) fn find_match_range(haystack: &str, needle_lower: &str) -> Option<(usize, usize)> {
    if needle_lower.is_empty() {
        return None;
    }
    let haystack_lower = haystack.to_ascii_lowercase();
    haystack_lower
        .find(needle_lower)
        .map(|start| (start, start + needle_lower.len()))
}

pub(crate) fn row_matches(row: &Row, needle_lower: &str) -> bool {
    find_match_range(&row.key, needle_lower).is_some()
        || find_match_range(row.action, needle_lower).is_some()
}

pub(crate) struct HighlightStyle<'a> {
    pub(crate) font_family: &'a str,
    pub(crate) font_size: f64,
    pub(crate) font_weight: cairo::FontWeight,
    pub(crate) color: [f64; 4],
}

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

pub(crate) fn draw_segmented_text(
    ctx: &cairo::Context,
    x: f64,
    baseline: f64,
    font_size: f64,
    weight: cairo::FontWeight,
    font_family: &str,
    segments: &[(String, [f64; 4])],
) {
    let mut cursor_x = x;
    for (text, color) in segments {
        ctx.set_source_rgba(color[0], color[1], color[2], color[3]);
        ctx.move_to(cursor_x, baseline);
        let _ = ctx.show_text(text);

        let extents = text_extents_for(
            ctx,
            font_family,
            cairo::FontSlant::Normal,
            weight,
            font_size,
            text,
        );
        cursor_x += extents.width();
    }
}

pub(crate) fn ellipsize_to_fit(
    ctx: &cairo::Context,
    text: &str,
    font_family: &str,
    font_size: f64,
    weight: cairo::FontWeight,
    max_width: f64,
) -> String {
    let extents = text_extents_for(
        ctx,
        font_family,
        cairo::FontSlant::Normal,
        weight,
        font_size,
        text,
    );
    if extents.width() <= max_width {
        return text.to_string();
    }

    let ellipsis = ELLIPSIS;
    let ellipsis_extents = text_extents_for(
        ctx,
        font_family,
        cairo::FontSlant::Normal,
        weight,
        font_size,
        ellipsis,
    );
    if ellipsis_extents.width() > max_width {
        return String::new();
    }

    let mut end = text.len();
    while end > 0 {
        if !text.is_char_boundary(end) {
            end -= 1;
            continue;
        }
        let candidate = format!("{}{}", &text[..end], ellipsis);
        let candidate_extents = text_extents_for(
            ctx,
            font_family,
            cairo::FontSlant::Normal,
            weight,
            font_size,
            &candidate,
        );
        if candidate_extents.width() <= max_width {
            return candidate;
        }
        end -= 1;
    }

    ellipsis.to_string()
}
