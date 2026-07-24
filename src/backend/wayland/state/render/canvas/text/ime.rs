use std::ops::Range;

use crate::input::state::ImePreedit;

/// Construct the preview and optional absolute byte range used to display the
/// compositor's preedit cursor. Protocol offsets are clamped down to valid
/// UTF-8 boundaries before slicing.
pub(super) fn build_text_preview(
    buffer: &str,
    preedit: Option<&ImePreedit>,
    cursor_glyph: &str,
) -> (String, Option<Range<usize>>) {
    let Some(preedit) = preedit else {
        return (format!("{buffer}{cursor_glyph}"), None);
    };

    if preedit.cursor_begin == -1 && preedit.cursor_end == -1 {
        return (format!("{buffer}{}", preedit.text), None);
    }

    let begin = usize::try_from(preedit.cursor_begin)
        .ok()
        .map(|offset| clamp_char_boundary(&preedit.text, offset));
    let end = usize::try_from(preedit.cursor_end)
        .ok()
        .map(|offset| clamp_char_boundary(&preedit.text, offset));

    match (begin, end) {
        (Some(begin), Some(end)) if begin != end => {
            let start = begin.min(end);
            let end = begin.max(end);
            (
                format!("{buffer}{}", preedit.text),
                Some(buffer.len() + start..buffer.len() + end),
            )
        }
        (Some(cursor), _) | (None, Some(cursor)) => (
            format!(
                "{buffer}{}{cursor_glyph}{}",
                &preedit.text[..cursor],
                &preedit.text[cursor..]
            ),
            None,
        ),
        (None, None) => (format!("{buffer}{}", preedit.text), None),
    }
}

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

/// Clamp `byte` to the nearest UTF-8 char boundary at or below it (and to
/// the string length), so slicing the preedit at an IME cursor offset never
/// splits a multi-byte character.
fn clamp_char_boundary(s: &str, byte: usize) -> usize {
    let mut idx = byte.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::{build_text_preview, paint_preedit_selection};
    use crate::input::state::ImePreedit;

    #[test]
    fn collapsed_preedit_cursor_is_inserted_at_its_utf8_byte_offset() {
        let preedit = ImePreedit {
            text: "你a".to_string(),
            cursor_begin: 3,
            cursor_end: 3,
        };

        assert_eq!(
            build_text_preview("base", Some(&preedit), "|"),
            ("base你|a".to_string(), None)
        );
    }

    #[test]
    fn non_collapsed_preedit_cursor_becomes_a_normalized_highlight_range() {
        let preedit = ImePreedit {
            text: "abcd".to_string(),
            cursor_begin: 4,
            cursor_end: 1,
        };

        assert_eq!(
            build_text_preview("xy", Some(&preedit), "|"),
            ("xyabcd".to_string(), Some(3..6))
        );
    }

    #[test]
    fn minus_one_pair_hides_the_preedit_cursor() {
        let preedit = ImePreedit {
            text: "compose".to_string(),
            cursor_begin: -1,
            cursor_end: -1,
        };

        assert_eq!(
            build_text_preview("base", Some(&preedit), "|"),
            ("basecompose".to_string(), None)
        );
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
}
