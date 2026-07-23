use super::super::primitives::text_extents_for;
use super::types::Row;
// The palette's fuzzy scorer/tokenizer and its shared static search-model
// scorer, reused directly so help search and the command palette rank
// identically (no per-surface reimplementation).
use crate::input::state::{action_meta_token_score, fuzzy_score, query_tokens};
use crate::ui_text::{UiTextStyle, draw_text_baseline};

// Substring match highlighting is shared with the command palette; matching
// itself is fuzzy (see [`row_matches`]), so a fuzzy-only match draws none.
pub(crate) use crate::ui::text_highlight::{HighlightStyle, draw_highlight, find_match_range};

const ELLIPSIS: &str = "\u{2026}";

/// Fuzzy row match: every query token must fuzzy-match the shortcut string
/// (`key`), the visible action description, or — for rows that carry an action
/// id — the command palette's shared static search model (label + short label +
/// description + category + aliases). Reusing that model is what lets an alias
/// query like "pie menu" resolve the radial-menu row, whose shortcut and label
/// never spell "pie". Mirrors the palette's all-tokens rule.
pub(crate) fn row_matches(row: &Row, needle_lower: &str) -> bool {
    let tokens = query_tokens(needle_lower);
    if tokens.is_empty() {
        return false;
    }
    let meta = row.action_id.and_then(crate::config::action_meta);
    tokens.iter().all(|token| {
        fuzzy_score(token, &row.key) > 0
            || fuzzy_score(token, row.action) > 0
            || meta.is_some_and(|meta| action_meta_token_score(meta, token) > 0)
    })
}

/// Fuzzy section-title match (keeps whole sections visible, as before).
pub(crate) fn title_matches(title: &str, needle_lower: &str) -> bool {
    let tokens = query_tokens(needle_lower);
    if tokens.is_empty() {
        return false;
    }
    tokens.iter().all(|token| fuzzy_score(token, title) > 0)
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
    let style = UiTextStyle {
        family: font_family,
        slant: cairo::FontSlant::Normal,
        weight,
        size: font_size,
    };
    for (text, color) in segments {
        ctx.set_source_rgba(color[0], color[1], color[2], color[3]);
        let extents = draw_text_baseline(ctx, style, text, cursor_x, baseline, None);
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

#[cfg(test)]
mod tests {
    use super::super::types::row;
    use super::row_matches;
    use crate::config::Action;

    #[test]
    fn row_matches_on_shortcut_and_visible_label() {
        let r = row("Middle Click / M", "Radial Menu").with_action(Action::ToggleRadialMenu);
        assert!(row_matches(&r, "radial"));
        assert!(row_matches(&r, "middle"));
    }

    #[test]
    fn row_matches_action_alias_via_shared_palette_model() {
        // "pie" lives only in ToggleRadialMenu's search_aliases ("pie menu"),
        // never in the row's shortcut ("Middle Click / M") or its visible label
        // ("Radial Menu"). Reusing the command palette's static search model is
        // what makes an alias-only query resolve the row.
        let r = row("Middle Click / M", "Radial Menu").with_action(Action::ToggleRadialMenu);
        assert!(row_matches(&r, "pie"));
        assert!(row_matches(&r, "pie menu"));
    }

    #[test]
    fn row_without_action_id_cannot_borrow_aliases() {
        // A gesture-only row carries no action id, so it matches on its own
        // text but never on another action's aliases.
        let r = row("Shift+Drag", "Straight line");
        assert!(row_matches(&r, "line"));
        assert!(!row_matches(&r, "pie"));
    }

    #[test]
    fn row_requires_every_token_to_match_somewhere() {
        let r = row("Middle Click / M", "Radial Menu").with_action(Action::ToggleRadialMenu);
        assert!(!row_matches(&r, "pie zznomatchxyz"));
    }
}
