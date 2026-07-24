use std::ops::Range;

/// Paint a Pango-backed highlight for a non-collapsed preedit cursor range.
pub(super) fn paint_preedit_selection(
    ctx: &cairo::Context,
    x: i32,
    y: i32,
    text: &str,
    selection: Range<u32>,
    font_desc: &str,
    wrap_width: Option<i32>,
) {
    let layout = pangocairo::functions::create_layout(ctx);
    let font_desc = pango::FontDescription::from_string(font_desc);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    if let Some(width) = wrap_width {
        let width = width.max(1);
        let width_pango = (i64::from(width) * i64::from(pango::SCALE)).min(i64::from(i32::MAX));
        layout.set_width(width_pango as i32);
        layout.set_wrap(pango::WrapMode::WordChar);
    }

    let attrs = pango::AttrList::new();
    let mut background = pango::AttrColor::new_background(0x2f2f, 0x9e9e, 0xb7b7);
    background.set_start_index(selection.start);
    background.set_end_index(selection.end);
    attrs.insert(background);
    let mut alpha = pango::AttrInt::new_background_alpha(0x7000);
    alpha.set_start_index(selection.start);
    alpha.set_end_index(selection.end);
    attrs.insert(alpha);
    layout.set_attributes(Some(&attrs));

    let baseline = layout.baseline() as f64 / pango::SCALE as f64;
    ctx.save().ok();
    ctx.move_to(x as f64, y as f64 - baseline);
    // The transparent source suppresses a second glyph pass; Pango's explicit
    // background attributes still paint the selected range.
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    pangocairo::functions::show_layout(ctx, &layout);
    ctx.restore().ok();
}

/// Paint a preedit underline using the complete Pango layout. Applying the
/// attribute to byte indices in that layout keeps it attached to the intended
/// glyph run for RTL, mixed-direction, context-shaped, wrapped, and multiline
/// text. The glyph source stays transparent so this pass adds only decoration.
pub(super) fn paint_preedit_underline(
    ctx: &cairo::Context,
    origin: (i32, i32),
    text: &str,
    range: Range<u32>,
    font_desc: &str,
    wrap_width: Option<i32>,
    color: crate::draw::Color,
) {
    if range.is_empty() || usize::try_from(range.end).map_or(true, |end| end > text.len()) {
        return;
    }

    let layout = pangocairo::functions::create_layout(ctx);
    let font_desc = pango::FontDescription::from_string(font_desc);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    if let Some(width) = wrap_width {
        let width = width.max(1);
        let width_pango = (i64::from(width) * i64::from(pango::SCALE)).min(i64::from(i32::MAX));
        layout.set_width(width_pango as i32);
        layout.set_wrap(pango::WrapMode::WordChar);
    }

    let attrs = pango::AttrList::new();
    let mut underline = pango::AttrInt::new_underline(pango::Underline::Single);
    underline.set_start_index(range.start);
    underline.set_end_index(range.end);
    attrs.insert(underline);
    let channel = |value: f64| (value.clamp(0.0, 1.0) * f64::from(u16::MAX)).round() as u16;
    let mut underline_color =
        pango::AttrColor::new_underline_color(channel(color.r), channel(color.g), channel(color.b));
    underline_color.set_start_index(range.start);
    underline_color.set_end_index(range.end);
    attrs.insert(underline_color);
    let mut foreground_alpha = pango::AttrInt::new_foreground_alpha(0);
    foreground_alpha.set_start_index(0);
    foreground_alpha.set_end_index(u32::try_from(text.len()).unwrap_or(u32::MAX));
    attrs.insert(foreground_alpha);
    layout.set_attributes(Some(&attrs));

    let baseline = layout.baseline() as f64 / pango::SCALE as f64;
    ctx.save().ok();
    ctx.move_to(origin.0 as f64, origin.1 as f64 - baseline);
    ctx.set_source_rgba(0.0, 0.0, 0.0, color.a.clamp(0.0, 1.0));
    pangocairo::functions::show_layout(ctx, &layout);
    ctx.restore().ok();
}

#[cfg(test)]
mod tests {
    use super::{paint_preedit_selection, paint_preedit_underline};
    use crate::draw::Color;
    use crate::input::state::{ImePreedit, build_text_input_preview as build_text_preview};

    #[test]
    fn plain_caret_leaves_the_text_intact_and_reports_its_offset() {
        // The caret is a separate line, never injected into the run, so the
        // buffer text is unchanged and its byte offset is reported.
        let p = build_text_preview("ac", 1, None, None, "_");
        assert_eq!(p.text, "ac");
        assert_eq!(p.caret, Some(1));
        assert_eq!(p.highlight, None);
        assert_eq!(p.underline, None);
    }

    #[test]
    fn plain_caret_snaps_off_boundary_offsets_down() {
        // A caret at byte 2 inside the 3-byte '你' must snap down to a boundary.
        let p = build_text_preview("你a", 2, None, None, "_");
        assert_eq!(p.text, "你a");
        assert_eq!(p.caret, Some(0));
    }

    #[test]
    fn a_selection_highlights_the_span_and_reports_the_caret_edge() {
        let p = build_text_preview("hello", 4, Some(1..4), None, "_");
        assert_eq!(
            p.text, "hello",
            "no caret glyph is injected under a selection"
        );
        assert_eq!(p.highlight, Some(1..4));
        assert_eq!(p.caret, Some(4), "the caret sits at the selection edge");
    }

    #[test]
    fn collapsed_preedit_cursor_is_inserted_at_the_caret_offset() {
        let preedit = ImePreedit {
            text: "你a".to_string(),
            cursor_begin: 3,
            cursor_end: 3,
        };

        // Buffer "base|end" composing at the caret (byte 4).
        let p = build_text_preview("baseend", 4, None, Some(&preedit), "|");
        assert_eq!(p.text, "base你|aend");
        assert_eq!(p.highlight, None);
        // Underline spans the composition plus the injected caret glyph.
        assert_eq!(p.underline, Some(4..4 + "你a".len() + 1));
    }

    #[test]
    fn non_collapsed_preedit_cursor_becomes_a_normalized_highlight_range() {
        let preedit = ImePreedit {
            text: "abcd".to_string(),
            cursor_begin: 4,
            cursor_end: 1,
        };

        let p = build_text_preview("xy", 2, None, Some(&preedit), "|");
        assert_eq!(p.text, "xyabcd");
        assert_eq!(p.highlight, Some(3..6));
        assert_eq!(p.underline, Some(2..6));
    }

    #[test]
    fn preedit_over_a_selection_replaces_the_selected_text() {
        let preedit = ImePreedit {
            text: "X".to_string(),
            cursor_begin: 1,
            cursor_end: 1,
        };
        // "hello world" with "hello" selected, composing "X": the composition
        // takes the selection's place instead of appearing beside it.
        let p = build_text_preview("hello world", 0, Some(0..5), Some(&preedit), "|");
        assert!(
            p.text.starts_with('X'),
            "composition sits where the selection was: {}",
            p.text
        );
        assert!(
            !p.text.contains("hello"),
            "the selected text is removed from the preview: {}",
            p.text
        );
        assert!(
            p.text.contains("world"),
            "unselected text remains: {}",
            p.text
        );
    }

    #[test]
    fn minus_one_pair_hides_the_preedit_cursor() {
        let preedit = ImePreedit {
            text: "compose".to_string(),
            cursor_begin: -1,
            cursor_end: -1,
        };

        let p = build_text_preview("base", 4, None, Some(&preedit), "|");
        assert_eq!(p.text, "basecompose");
        assert_eq!(p.highlight, None);
        assert_eq!(p.underline, Some(4..4 + "compose".len()));
    }

    #[test]
    fn preedit_selection_paints_a_visible_highlight_without_a_second_text_pass() {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 160, 60).unwrap();
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            paint_preedit_selection(&ctx, 8, 36, "selected", 0..8, "Sans 20", None);
        }
        surface.flush();
        let data = surface.data().unwrap();

        assert!(
            data.iter().any(|byte| *byte != 0),
            "the range background must paint even though the duplicate glyph source is transparent"
        );
    }

    #[test]
    fn preedit_underline_paints_a_mixed_direction_range_from_the_full_layout() {
        let text = "abc אבג xyz";
        let start = text.find('א').unwrap();
        let end = text.find(" xyz").unwrap();
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 240, 80).unwrap();
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            paint_preedit_underline(
                &ctx,
                (8, 48),
                text,
                u32::try_from(start).unwrap()..u32::try_from(end).unwrap(),
                "Sans 24",
                None,
                Color {
                    r: 0.9,
                    g: 0.2,
                    b: 0.1,
                    a: 1.0,
                },
            );
        }
        surface.flush();
        let data = surface.data().unwrap();

        assert!(
            data.iter().any(|byte| *byte != 0),
            "the full Pango layout must paint the RTL preedit underline"
        );
    }
}
