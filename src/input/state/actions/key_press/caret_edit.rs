//! Pure caret/selection edit operations on a text buffer.
//!
//! These act on the raw `(buffer, caret, selection_anchor)` triple stored in
//! `DrawingState::TextInput`, independent of `InputState`, so the editing model
//! — grapheme-aware movement, word operations, selection replacement — is small
//! and exhaustively unit-testable.
//!
//! Conventions:
//! - `caret` is a byte offset into `buffer`, always on a UTF-8/grapheme
//!   boundary.
//! - The selection span is `min(anchor, caret)..max(anchor, caret)`; an absent
//!   or caret-equal anchor means no selection.
//! - Movement takes an `extend` flag (Shift held): when set, the anchor is
//!   pinned at the pre-move caret so the selection grows; when clear, any
//!   selection collapses.
//! - Each operation returns whether it changed anything visible, so callers can
//!   skip needless redraws at the buffer edges.

use std::ops::Range;
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

use crate::draw::shape::{VisualCaretDirection, VisualLineDirection, VisualLineEdge};
use crate::input::events::Key;
use crate::input::state::DrawingState;

use super::text_input::move_horizontal_caret;

/// Canonical layout inputs borrowed for one editor navigation operation.
/// The owner is shared with damage measurement; no destination Cairo context
/// or runtime resource is retained by the editor state.
#[derive(Clone, Copy)]
pub(in crate::input::state) struct TextNavigation<'a> {
    pub(in crate::input::state) measurer: &'a crate::draw::TextMeasurer,
    pub(in crate::input::state) font: &'a str,
    pub(in crate::input::state) wrap_width: Option<i32>,
}

/// Maximum text-buffer length in bytes, shared by keyboard entry and IME
/// commits so both enforce the same cap.
pub(in crate::input::state) const MAX_TEXT_LENGTH: usize = 10_000;

/// The active selection span, or `None` when the caret is collapsed.
pub(in crate::input::state) fn selection_range(
    caret: usize,
    anchor: Option<usize>,
) -> Option<Range<usize>> {
    let anchor = anchor?;
    if anchor == caret {
        None
    } else {
        Some(anchor.min(caret)..anchor.max(caret))
    }
}

/// Prepare the anchor before a caret move: pin it at the current caret when
/// extending a selection, or drop the selection entirely otherwise. Returns
/// whether dropping the anchor collapsed a visible selection.
fn prime_anchor(caret: usize, anchor: &mut Option<usize>, extend: bool) -> bool {
    let collapsed_selection = !extend && anchor.is_some_and(|anchor| anchor != caret);
    if extend {
        if anchor.is_none() {
            *anchor = Some(caret);
        }
    } else {
        *anchor = None;
    }
    collapsed_selection
}

/// Move to an already-resolved byte offset while applying the editor's normal
/// Shift-selection anchoring rules.
pub(in crate::input::state) fn move_to_offset(
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
    new: usize,
) -> bool {
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    if new == *caret {
        return collapsed_selection;
    }
    *caret = new;
    true
}

fn prev_grapheme(s: &str, idx: usize) -> usize {
    GraphemeCursor::new(idx, s.len(), true)
        .prev_boundary(s, 0)
        .ok()
        .flatten()
        .unwrap_or(0)
}

fn next_grapheme(s: &str, idx: usize) -> usize {
    GraphemeCursor::new(idx, s.len(), true)
        .next_boundary(s, 0)
        .ok()
        .flatten()
        .unwrap_or(s.len())
}

/// Start byte of the word at or before `caret` (skipping any run of
/// non-word characters immediately before it), for word-left / word-delete.
fn prev_word_start(s: &str, caret: usize) -> usize {
    let mut result = 0;
    for (i, _) in s.unicode_word_indices() {
        if i < caret {
            result = i;
        } else {
            break;
        }
    }
    result
}

/// End byte of the word at or after `caret`, for word-right / word-delete.
fn next_word_end(s: &str, caret: usize) -> usize {
    for (i, word) in s.unicode_word_indices() {
        let end = i + word.len();
        if end > caret {
            return end;
        }
    }
    s.len()
}

/// Byte offset of the start of the logical line containing `caret`.
fn line_start(s: &str, caret: usize) -> usize {
    s[..caret].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

/// Byte offset of the end of the logical line containing `caret` (the position
/// of the next newline, or the buffer end).
fn line_end(s: &str, caret: usize) -> usize {
    s[caret..].find('\n').map(|i| caret + i).unwrap_or(s.len())
}

/// Grapheme count of `s` (used as the column metric for vertical movement).
fn grapheme_len(s: &str) -> usize {
    s.graphemes(true).count()
}

/// Byte offset of the `col`-th grapheme boundary within `s`, clamped to the end.
fn grapheme_col_to_byte(s: &str, col: usize) -> usize {
    s.grapheme_indices(true)
        .nth(col)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Clamp `caret` and `anchor` to valid char boundaries within `buffer` (used
/// after the buffer is populated from an existing shape or otherwise mutated).
pub(in crate::input::state) fn clamp(buffer: &str, caret: &mut usize, anchor: &mut Option<usize>) {
    let clamp_one = |idx: usize| {
        let mut idx = idx.min(buffer.len());
        while idx > 0 && !buffer.is_char_boundary(idx) {
            idx -= 1;
        }
        idx
    };
    *caret = clamp_one(*caret);
    if let Some(a) = anchor {
        *a = clamp_one(*a);
    }
}

/// Delete the current selection, collapsing the caret to its start. Returns
/// whether a non-empty selection was removed. Always clears the anchor.
pub(in crate::input::state) fn delete_selection(
    buffer: &mut String,
    caret: &mut usize,
    anchor: &mut Option<usize>,
) -> bool {
    if let Some(range) = selection_range(*caret, *anchor) {
        buffer.replace_range(range.clone(), "");
        *caret = range.start;
        *anchor = None;
        true
    } else {
        *anchor = None;
        false
    }
}

/// Insert `text` at the caret (first replacing any selection), honoring
/// `max_len` in bytes. Returns whether the buffer changed.
pub(in crate::input::state) fn insert_str(
    buffer: &mut String,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    text: &str,
    max_len: usize,
) -> bool {
    let selected_len = selection_range(*caret, *anchor)
        .map(|range| range.len())
        .unwrap_or(0);
    let remaining = max_len.saturating_sub(buffer.len().saturating_sub(selected_len));
    let mut fit = text.len().min(remaining);
    while fit > 0 && !text.is_char_boundary(fit) {
        fit -= 1;
    }
    // Replacement is one edit: if none of a non-empty insertion fits, retain
    // both the selected text and its selection instead of deleting first.
    if fit == 0 && !text.is_empty() {
        return false;
    }
    let replaced = delete_selection(buffer, caret, anchor);
    if fit == 0 {
        return replaced;
    }
    buffer.insert_str(*caret, &text[..fit]);
    *caret += fit;
    true
}

/// Delete the grapheme before the caret (or the selection, if any).
pub(in crate::input::state) fn backspace(
    buffer: &mut String,
    caret: &mut usize,
    anchor: &mut Option<usize>,
) -> bool {
    if delete_selection(buffer, caret, anchor) {
        return true;
    }
    if *caret == 0 {
        return false;
    }
    let start = prev_grapheme(buffer, *caret);
    buffer.replace_range(start..*caret, "");
    *caret = start;
    true
}

/// Delete the grapheme after the caret (or the selection, if any).
pub(in crate::input::state) fn delete_forward(
    buffer: &mut String,
    caret: &mut usize,
    anchor: &mut Option<usize>,
) -> bool {
    if delete_selection(buffer, caret, anchor) {
        return true;
    }
    if *caret >= buffer.len() {
        return false;
    }
    let end = next_grapheme(buffer, *caret);
    buffer.replace_range(*caret..end, "");
    true
}

/// Delete from the previous word boundary to the caret (Ctrl+Backspace).
pub(in crate::input::state) fn delete_word_backward(
    buffer: &mut String,
    caret: &mut usize,
    anchor: &mut Option<usize>,
) -> bool {
    if delete_selection(buffer, caret, anchor) {
        return true;
    }
    if *caret == 0 {
        return false;
    }
    let mut start = prev_word_start(buffer, *caret);
    if start >= *caret {
        start = prev_grapheme(buffer, *caret);
    }
    buffer.replace_range(start..*caret, "");
    *caret = start;
    true
}

/// Delete from the caret to the next word boundary (Ctrl+Delete).
pub(in crate::input::state) fn delete_word_forward(
    buffer: &mut String,
    caret: &mut usize,
    anchor: &mut Option<usize>,
) -> bool {
    if delete_selection(buffer, caret, anchor) {
        return true;
    }
    if *caret >= buffer.len() {
        return false;
    }
    let mut end = next_word_end(buffer, *caret);
    if end <= *caret {
        end = next_grapheme(buffer, *caret);
    }
    buffer.replace_range(*caret..end, "");
    true
}

/// Move the caret one grapheme left. A collapsing (non-extending) move over a
/// selection snaps to its left edge instead of stepping past it.
pub(in crate::input::state) fn move_left(
    buffer: &str,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
) -> bool {
    if !extend && let Some(range) = selection_range(*caret, *anchor) {
        *caret = range.start;
        *anchor = None;
        return true;
    }
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    let new = prev_grapheme(buffer, *caret);
    if new == *caret {
        return collapsed_selection;
    }
    *caret = new;
    true
}

/// Move the caret one grapheme right (snapping to the selection's right edge on
/// a collapsing move).
pub(in crate::input::state) fn move_right(
    buffer: &str,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
) -> bool {
    if !extend && let Some(range) = selection_range(*caret, *anchor) {
        *caret = range.end;
        *anchor = None;
        return true;
    }
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    let new = next_grapheme(buffer, *caret);
    if new == *caret {
        return collapsed_selection;
    }
    *caret = new;
    true
}

/// Move the caret to the start of the previous word (Ctrl+Left).
pub(in crate::input::state) fn move_word_left(
    buffer: &str,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
) -> bool {
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    let new = prev_word_start(buffer, *caret);
    if new >= *caret {
        return collapsed_selection;
    }
    *caret = new;
    true
}

/// Move the caret to the end of the next word (Ctrl+Right).
pub(in crate::input::state) fn move_word_right(
    buffer: &str,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
) -> bool {
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    let new = next_word_end(buffer, *caret);
    if new <= *caret {
        return collapsed_selection;
    }
    *caret = new;
    true
}

/// Move the caret to the start of its logical line (Home).
pub(in crate::input::state) fn move_line_home(
    buffer: &str,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
) -> bool {
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    let new = line_start(buffer, *caret);
    if new == *caret {
        return collapsed_selection;
    }
    *caret = new;
    true
}

/// Move the caret to the end of its logical line (End).
pub(in crate::input::state) fn move_line_end(
    buffer: &str,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
) -> bool {
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    let new = line_end(buffer, *caret);
    if new == *caret {
        return collapsed_selection;
    }
    *caret = new;
    true
}

/// Move the caret to the very start of the buffer (Ctrl+Home).
pub(in crate::input::state) fn move_document_start(
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
) -> bool {
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    if *caret == 0 {
        return collapsed_selection;
    }
    *caret = 0;
    true
}

/// Move the caret to the very end of the buffer (Ctrl+End).
pub(in crate::input::state) fn move_document_end(
    buffer: &str,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
) -> bool {
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    if *caret == buffer.len() {
        return collapsed_selection;
    }
    *caret = buffer.len();
    true
}

/// Move the caret up one logical line, preserving the grapheme column. At the
/// first line the caret goes to the buffer start. (Visual/wrapped-line movement
/// is layered on later via Pango; this handles the unwrapped multiline case.)
pub(in crate::input::state) fn move_up(
    buffer: &str,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
) -> bool {
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    let ls = line_start(buffer, *caret);
    if ls == 0 {
        if *caret == 0 {
            return collapsed_selection;
        }
        *caret = 0;
        return true;
    }
    let col = grapheme_len(&buffer[ls..*caret]);
    let prev_line_start = line_start(buffer, ls - 1);
    let prev_line = &buffer[prev_line_start..ls - 1];
    *caret = prev_line_start + grapheme_col_to_byte(prev_line, col);
    true
}

/// Move the caret down one logical line, preserving the grapheme column. At the
/// last line the caret goes to the buffer end.
pub(in crate::input::state) fn move_down(
    buffer: &str,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
) -> bool {
    let collapsed_selection = prime_anchor(*caret, anchor, extend);
    let le = line_end(buffer, *caret);
    if le == buffer.len() {
        if *caret == buffer.len() {
            return collapsed_selection;
        }
        *caret = buffer.len();
        return true;
    }
    let ls = line_start(buffer, *caret);
    let col = grapheme_len(&buffer[ls..*caret]);
    let next_line_start = le + 1;
    let next_line = &buffer[next_line_start..line_end(buffer, next_line_start)];
    *caret = next_line_start + grapheme_col_to_byte(next_line, col);
    true
}

/// Select the whole buffer (Ctrl+A).
pub(in crate::input::state) fn select_all(
    buffer: &str,
    caret: &mut usize,
    anchor: &mut Option<usize>,
) -> bool {
    if buffer.is_empty() {
        return false;
    }
    *anchor = Some(0);
    *caret = buffer.len();
    true
}

impl crate::input::state::core::TextEditing {
    /// Apply one editor-owned key. `None` means the key belongs to the action
    /// layer; `Some` reports whether the visible editor state changed.
    pub(in crate::input::state) fn apply_key_edit(
        &mut self,
        state: &mut DrawingState,
        key: Key,
        ctrl: bool,
        shift: bool,
        navigation: TextNavigation<'_>,
    ) -> Option<bool> {
        let mutates_buffer = match key {
            Key::Char(_) | Key::Space => !ctrl,
            Key::Return => shift,
            Key::Backspace | Key::Delete => true,
            _ => false,
        };
        let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = state
        else {
            return None;
        };
        clamp(buffer, caret, selection_anchor);

        let anchor = selection_anchor;
        let changed = match key {
            Key::Char(c) if !ctrl => {
                let mut encoded = [0u8; 4];
                insert_str(
                    buffer,
                    caret,
                    anchor,
                    c.encode_utf8(&mut encoded),
                    MAX_TEXT_LENGTH,
                )
            }
            Key::Char('a' | 'A') if ctrl => select_all(buffer, caret, anchor),
            Key::Space if !ctrl => insert_str(buffer, caret, anchor, " ", MAX_TEXT_LENGTH),
            Key::Return if shift => insert_str(buffer, caret, anchor, "\n", MAX_TEXT_LENGTH),
            Key::Backspace if ctrl => delete_word_backward(buffer, caret, anchor),
            Key::Backspace => backspace(buffer, caret, anchor),
            Key::Delete if ctrl => delete_word_forward(buffer, caret, anchor),
            Key::Delete => delete_forward(buffer, caret, anchor),
            Key::Left => move_horizontal_caret(
                buffer,
                navigation,
                caret,
                anchor,
                shift,
                ctrl,
                VisualCaretDirection::Left,
            ),
            Key::Right => move_horizontal_caret(
                buffer,
                navigation,
                caret,
                anchor,
                shift,
                ctrl,
                VisualCaretDirection::Right,
            ),
            Key::Up => navigation
                .measurer
                .caret_on_adjacent_visual_line(
                    buffer,
                    navigation.font,
                    navigation.wrap_width,
                    *caret,
                    VisualLineDirection::Up,
                )
                .map(|new| move_to_offset(caret, anchor, shift, new))
                .unwrap_or_else(|| move_up(buffer, caret, anchor, shift)),
            Key::Down => navigation
                .measurer
                .caret_on_adjacent_visual_line(
                    buffer,
                    navigation.font,
                    navigation.wrap_width,
                    *caret,
                    VisualLineDirection::Down,
                )
                .map(|new| move_to_offset(caret, anchor, shift, new))
                .unwrap_or_else(|| move_down(buffer, caret, anchor, shift)),
            Key::Home if ctrl => move_document_start(caret, anchor, shift),
            Key::Home => navigation
                .measurer
                .caret_on_visual_line_edge(
                    buffer,
                    navigation.font,
                    navigation.wrap_width,
                    *caret,
                    VisualLineEdge::Start,
                )
                .map(|new| move_to_offset(caret, anchor, shift, new))
                .unwrap_or_else(|| move_line_home(buffer, caret, anchor, shift)),
            Key::End if ctrl => move_document_end(buffer, caret, anchor, shift),
            Key::End => navigation
                .measurer
                .caret_on_visual_line_edge(
                    buffer,
                    navigation.font,
                    navigation.wrap_width,
                    *caret,
                    VisualLineEdge::End,
                )
                .map(|new| move_to_offset(caret, anchor, shift, new))
                .unwrap_or_else(|| move_line_end(buffer, caret, anchor, shift)),
            _ => return None,
        };

        if changed && mutates_buffer {
            self.note_buffer_mutation();
        }
        Some(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drive an op over a `(buffer, caret, anchor)` triple and return the
    /// resulting `(buffer, caret, anchor)` for terse assertions.
    struct Ed {
        buf: String,
        caret: usize,
        anchor: Option<usize>,
    }
    impl Ed {
        fn at(text: &str, caret: usize) -> Self {
            Ed {
                buf: text.to_string(),
                caret,
                anchor: None,
            }
        }
        fn sel(text: &str, anchor: usize, caret: usize) -> Self {
            Ed {
                buf: text.to_string(),
                caret,
                anchor: Some(anchor),
            }
        }
    }

    #[test]
    fn owner_key_edits_advance_revision_only_for_buffer_mutations() {
        let measurer = crate::draw::TextMeasurer::default();
        let mut editing = crate::input::state::core::TextEditing::default();
        let mut state = DrawingState::text_input(0, 0, "ab".to_string());
        editing.begin_session();

        assert_eq!(
            editing.apply_key_edit(
                &mut state,
                Key::Char('c'),
                false,
                false,
                TextNavigation {
                    measurer: &measurer,
                    font: "",
                    wrap_width: None
                }
            ),
            Some(true)
        );
        assert_eq!(editing.revision(), 1);
        assert_eq!(
            editing.apply_key_edit(
                &mut state,
                Key::Left,
                false,
                false,
                TextNavigation {
                    measurer: &measurer,
                    font: "",
                    wrap_width: None
                }
            ),
            Some(true)
        );
        assert_eq!(editing.revision(), 1, "caret motion is not a buffer edit");
    }

    #[test]
    fn insert_appends_at_caret_and_advances() {
        let mut e = Ed::at("ac", 1);
        assert!(insert_str(
            &mut e.buf,
            &mut e.caret,
            &mut e.anchor,
            "b",
            100
        ));
        assert_eq!((e.buf.as_str(), e.caret), ("abc", 2));
    }

    #[test]
    fn insert_replaces_selection() {
        let mut e = Ed::sel("hello", 1, 4); // "ell" selected
        assert!(insert_str(
            &mut e.buf,
            &mut e.caret,
            &mut e.anchor,
            "i",
            100
        ));
        assert_eq!((e.buf.as_str(), e.caret, e.anchor), ("hio", 2, None));
    }

    #[test]
    fn insert_respects_max_len_on_a_char_boundary() {
        // Budget of 1 byte remaining must not split the 3-byte '好'.
        let mut e = Ed::at("ab", 2);
        assert!(!insert_str(
            &mut e.buf,
            &mut e.caret,
            &mut e.anchor,
            "好",
            3
        ));
        assert_eq!(e.buf, "ab");
    }

    #[test]
    fn replacement_is_atomic_when_no_complete_character_fits() {
        let mut e = Ed::sel(&"a".repeat(MAX_TEXT_LENGTH), 0, 1);

        assert!(!insert_str(
            &mut e.buf,
            &mut e.caret,
            &mut e.anchor,
            "好",
            MAX_TEXT_LENGTH
        ));

        assert_eq!(e.buf.len(), MAX_TEXT_LENGTH);
        assert_eq!(&e.buf[..1], "a");
        assert_eq!((e.caret, e.anchor), (1, Some(0)));
    }

    #[test]
    fn backspace_removes_grapheme_before_caret() {
        let mut e = Ed::at("a你b", 4); // caret after '你' (byte 4)
        assert!(backspace(&mut e.buf, &mut e.caret, &mut e.anchor));
        assert_eq!((e.buf.as_str(), e.caret), ("ab", 1));
    }

    #[test]
    fn backspace_at_start_is_a_no_op() {
        let mut e = Ed::at("x", 0);
        assert!(!backspace(&mut e.buf, &mut e.caret, &mut e.anchor));
        assert_eq!(e.buf, "x");
    }

    #[test]
    fn delete_forward_removes_grapheme_after_caret() {
        let mut e = Ed::at("a你b", 1);
        assert!(delete_forward(&mut e.buf, &mut e.caret, &mut e.anchor));
        assert_eq!((e.buf.as_str(), e.caret), ("ab", 1));
    }

    #[test]
    fn backspace_deletes_selection_first() {
        let mut e = Ed::sel("abcd", 1, 3);
        assert!(backspace(&mut e.buf, &mut e.caret, &mut e.anchor));
        assert_eq!((e.buf.as_str(), e.caret, e.anchor), ("ad", 1, None));
    }

    #[test]
    fn word_backspace_deletes_a_whole_word() {
        let mut e = Ed::at("foo bar", 7);
        assert!(delete_word_backward(
            &mut e.buf,
            &mut e.caret,
            &mut e.anchor
        ));
        assert_eq!((e.buf.as_str(), e.caret), ("foo ", 4));
    }

    #[test]
    fn word_delete_forward_removes_next_word() {
        let mut e = Ed::at("foo bar", 0);
        assert!(delete_word_forward(&mut e.buf, &mut e.caret, &mut e.anchor));
        assert_eq!((e.buf.as_str(), e.caret), (" bar", 0));
    }

    #[test]
    fn left_right_step_by_grapheme() {
        let mut e = Ed::at("a你b", 1);
        assert!(move_right(&e.buf, &mut e.caret, &mut e.anchor, false));
        assert_eq!(e.caret, 4, "steps over the 3-byte char");
        assert!(move_left(&e.buf, &mut e.caret, &mut e.anchor, false));
        assert_eq!(e.caret, 1);
    }

    #[test]
    fn unshifted_left_collapses_selection_to_its_start() {
        let mut e = Ed::sel("abcd", 1, 3);
        assert!(move_left(&e.buf, &mut e.caret, &mut e.anchor, false));
        assert_eq!((e.caret, e.anchor), (1, None));
    }

    #[test]
    fn shift_left_extends_selection_from_the_caret() {
        let mut e = Ed::at("abcd", 3);
        assert!(move_left(&e.buf, &mut e.caret, &mut e.anchor, true));
        assert_eq!((e.caret, e.anchor), (2, Some(3)));
        assert_eq!(selection_range(e.caret, e.anchor), Some(2..3));
    }

    #[test]
    fn word_moves_land_on_word_boundaries() {
        let mut e = Ed::at("foo bar baz", 5); // inside "bar"
        assert!(move_word_left(&e.buf, &mut e.caret, &mut e.anchor, false));
        assert_eq!(e.caret, 4, "start of 'bar'");
        assert!(move_word_right(&e.buf, &mut e.caret, &mut e.anchor, false));
        assert_eq!(e.caret, 7, "end of 'bar'");
    }

    #[test]
    fn home_and_end_stay_within_the_logical_line() {
        let mut e = Ed::at("ab\ncd\nef", 7); // in the third line, at 'f'
        assert!(move_line_home(&e.buf, &mut e.caret, &mut e.anchor, false));
        assert_eq!(e.caret, 6, "start of third line");
        assert!(move_line_end(&e.buf, &mut e.caret, &mut e.anchor, false));
        assert_eq!(e.caret, 8, "end of third line");
    }

    #[test]
    fn boundary_move_reports_when_it_only_collapses_a_selection() {
        let mut e = Ed::sel("abcd", 4, 0);

        assert!(move_document_start(&mut e.caret, &mut e.anchor, false));
        assert_eq!((e.caret, e.anchor), (0, None));
    }

    #[test]
    fn up_and_down_preserve_the_grapheme_column() {
        let mut e = Ed::at("abcd\nef\nghij", 12); // end of "ghij"
        assert!(move_up(&e.buf, &mut e.caret, &mut e.anchor, false));
        // Column 4 clamps to the short middle line "ef" (length 2).
        assert_eq!(e.caret, 7, "end of 'ef'");
        assert!(move_up(&e.buf, &mut e.caret, &mut e.anchor, false));
        assert_eq!(e.caret, 2, "column 2 of first line");
        assert!(move_down(&e.buf, &mut e.caret, &mut e.anchor, false));
        assert_eq!(e.caret, 7, "back onto 'ef', clamped");
    }

    #[test]
    fn up_at_first_line_goes_to_document_start() {
        let mut e = Ed::at("abc\ndef", 2);
        assert!(move_up(&e.buf, &mut e.caret, &mut e.anchor, false));
        assert_eq!(e.caret, 0);
    }

    #[test]
    fn select_all_spans_the_whole_buffer() {
        let mut e = Ed::at("hello", 2);
        assert!(select_all(&e.buf, &mut e.caret, &mut e.anchor));
        assert_eq!(selection_range(e.caret, e.anchor), Some(0..5));
    }

    #[test]
    fn clamp_snaps_off_boundary_indices_down() {
        let buf = "a你"; // '你' occupies bytes 1..4
        let mut caret = 3; // mid-char
        let mut anchor = Some(2);
        clamp(buf, &mut caret, &mut anchor);
        assert_eq!((caret, anchor), (1, Some(1)));
    }
}
