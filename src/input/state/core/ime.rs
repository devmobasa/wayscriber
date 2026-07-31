//! Input-method (IME) composition state for the text/note editor.
//!
//! Drives the `zwp_text_input_v3` protocol on the state side: the backend
//! Wayland handlers translate protocol events into the `ime_queue_*` calls
//! below and apply them atomically on `ime_apply_done`, exactly mirroring
//! the protocol's double-buffered "batch then done" model. Keeping the whole
//! state machine here (off the Wayland types) makes it unit-testable.
//!
//! The text editor stores an explicit caret and optional selection in
//! `DrawingState::TextInput`: commits replace the selection at the caret,
//! surrounding deletes operate immediately before it, and preedit text is a
//! transient insertion preview that never enters the committed buffer until
//! the IME commits it.

use std::ops::Range;

use super::super::{DrawingState, InputState};

const MAX_SURROUNDING_TEXT_BYTES: usize = 4_000;

/// The active preedit (in-progress composition) shown at the editor caret.
/// `cursor_begin`/`cursor_end` are byte offsets into `text` describing the
/// IME's cursor within the composition (both -1 means "hide cursor").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImePreedit {
    pub text: String,
    pub cursor_begin: i32,
    pub cursor_end: i32,
}

/// Double-buffered pending changes accumulated between `done` events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ImePending {
    commit: Option<String>,
    preedit: Option<ImePreedit>,
    /// Distinguishes a nullable preedit event from no event in this batch.
    preedit_received: bool,
    delete_before: u32,
    delete_after: u32,
}

/// IME composition state stored on `InputState`.
#[derive(Debug, Clone, Default)]
pub struct ImeCompositionState {
    /// The active preedit rendered after the buffer, if any.
    preedit: Option<ImePreedit>,
    /// Changes queued since the last `done`, applied together on `done`.
    pending: ImePending,
}

/// The single effective text-editor preview consumed by rendering, damage, and
/// text-input-v3 cursor reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextInputPreview {
    pub(crate) text: String,
    pub(crate) highlight: Option<Range<usize>>,
    pub(crate) underline: Option<Range<usize>>,
    /// Normal editor caret drawn as a separate vertical line.
    pub(crate) caret: Option<usize>,
    /// Cursor offset used for the compositor candidate rectangle. Unlike
    /// `caret`, this remains present while an IME owns the visible cursor.
    pub(crate) ime_cursor: Option<usize>,
    /// How the transient preedit insertion maps back to the committed buffer
    /// for pointer hit-testing. Absent when preview and buffer offsets match.
    hit_test_map: Option<TextPreviewHitMap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextPreviewHitMap {
    buffer_replacement: Range<usize>,
    preview_replacement: Range<usize>,
}

impl TextInputPreview {
    /// Translate a byte offset in the rendered preview back to the committed
    /// buffer. A hit inside preedit text resolves to its insertion point;
    /// offsets after it account for any selection the preedit replaced.
    pub(crate) fn buffer_offset_for_preview_offset(&self, preview_offset: usize) -> usize {
        let Some(map) = &self.hit_test_map else {
            return preview_offset.min(self.text.len());
        };
        let preview_offset = preview_offset.min(self.text.len());
        if preview_offset <= map.preview_replacement.start {
            return preview_offset;
        }
        if preview_offset < map.preview_replacement.end {
            return map.buffer_replacement.start;
        }

        let original_buffer_len = self
            .text
            .len()
            .saturating_sub(map.preview_replacement.len())
            .saturating_add(map.buffer_replacement.len());
        map.buffer_replacement
            .end
            .saturating_add(preview_offset - map.preview_replacement.end)
            .min(original_buffer_len)
    }
}

impl ImeCompositionState {
    /// The active preedit run for rendering, or `None` when nothing is
    /// being composed.
    pub fn preedit(&self) -> Option<&ImePreedit> {
        self.preedit.as_ref()
    }
}

impl InputState {
    /// Start a distinct text-edit session for async completion identity.
    pub(crate) fn begin_text_input_session(&mut self) {
        self.text_input_generation = self.text_input_generation.wrapping_add(1);
        self.text_input_revision = 0;
        self.pending_text_paste.clear();
    }

    pub(crate) fn note_text_buffer_mutation(&mut self) {
        self.text_input_revision = self.text_input_revision.wrapping_add(1);
    }

    pub(crate) fn text_input_generation(&self) -> Option<u64> {
        self.is_text_input_active()
            .then_some(self.text_input_generation)
    }

    pub(crate) fn text_input_generation_is_current(&self, generation: u64) -> bool {
        self.text_input_generation() == Some(generation)
    }

    /// True while a text/note edit is in progress — the gate for enabling
    /// the text-input protocol.
    pub fn is_text_input_active(&self) -> bool {
        matches!(self.state, DrawingState::TextInput { .. })
    }

    /// Surrounding committed text and the directional cursor/anchor byte
    /// offsets reported to text-input-v3. Preedit text stays excluded. When a
    /// complete selection cannot fit the protocol limit, returns `None`; the
    /// backend temporarily disables the protocol object rather than applying
    /// an empty value that may make later surrounding-text updates ineffective.
    pub(crate) fn text_input_surrounding_state(&self) -> Option<(String, usize, usize)> {
        let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = &self.state
        else {
            return None;
        };
        let caret = clamp_char_boundary(buffer, *caret);
        let anchor = selection_anchor
            .map(|anchor| clamp_char_boundary(buffer, anchor))
            .unwrap_or(caret);
        surrounding_text_window(buffer, caret, anchor)
    }

    /// Whether the active selection can be represented in text-input-v3's
    /// bounded surrounding-text request without allocating the actual window.
    pub(crate) fn text_input_surrounding_available(&self) -> bool {
        let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = &self.state
        else {
            return false;
        };
        let caret = clamp_char_boundary(buffer, *caret);
        let anchor = selection_anchor
            .map(|anchor| clamp_char_boundary(buffer, anchor))
            .unwrap_or(caret);
        caret.abs_diff(anchor) <= MAX_SURROUNDING_TEXT_BYTES
    }

    /// Build the authoritative preview for the active text edit. A composing
    /// preedit replaces any selection and carries its own cursor; without a
    /// preedit, the normal caret and selection remain in buffer coordinates.
    pub(crate) fn text_input_preview(&self, cursor_glyph: &str) -> Option<TextInputPreview> {
        let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = &self.state
        else {
            return None;
        };
        let selection = selection_anchor.and_then(|anchor| {
            (anchor != *caret).then_some(anchor.min(*caret)..anchor.max(*caret))
        });
        Some(build_text_input_preview(
            buffer,
            *caret,
            selection,
            self.ime_preedit(),
            cursor_glyph,
        ))
    }

    /// The active preedit run (byte-cursor included) for the renderer.
    pub fn ime_preedit(&self) -> Option<&ImePreedit> {
        self.ime.preedit()
    }

    /// Queue committed text (`commit_string`) to append on the next `done`.
    /// `text = None` overwrites (cancels) any previously queued commit —
    /// these events replace double-buffered pending state, so a null
    /// `commit_string` must clear an earlier non-null one in the same batch.
    pub fn ime_queue_commit(&mut self, text: Option<String>) {
        self.ime.pending.commit = text;
    }

    /// Queue the in-progress composition (`preedit_string`) for the next
    /// `done`. `text = None` clears the preedit but still carries the event's
    /// selection-removal semantics.
    pub fn ime_queue_preedit(&mut self, text: Option<String>, cursor_begin: i32, cursor_end: i32) {
        self.ime.pending.preedit_received = true;
        self.ime.pending.preedit = text.map(|text| ImePreedit {
            text,
            cursor_begin,
            cursor_end,
        });
    }

    /// Queue a surrounding-text deletion (`delete_surrounding_text`), in
    /// UTF-8 bytes around the caret, for the next `done`.
    pub fn ime_queue_delete_surrounding(&mut self, before_length: u32, after_length: u32) {
        self.ime.pending.delete_before = before_length;
        self.ime.pending.delete_after = after_length;
    }

    /// Apply the queued composition changes to the editor and reset the
    /// pending batch (the protocol `done` event). Returns whether anything
    /// visible changed. No-op (and clears any stale state) when a text edit
    /// is not active.
    pub fn ime_apply_done(&mut self) -> bool {
        if !self.is_text_input_active() {
            self.ime = ImeCompositionState::default();
            return false;
        }

        let pending = std::mem::take(&mut self.ime.pending);
        let mut buffer_changed = false;

        if let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = &mut self.state
        {
            use crate::input::state::actions::key_press::caret_edit;
            caret_edit::clamp(buffer, caret, selection_anchor);

            // 1) delete_surrounding_text: before/after lengths exclude an
            //    active selection, so delete outward from the selection edges
            //    and retain its direction for the following commit replacement.
            if pending.delete_before > 0 || pending.delete_after > 0 {
                let original_caret = *caret;
                let original_anchor = *selection_anchor;
                let selection = caret_edit::selection_range(original_caret, original_anchor);
                let before_edge = selection
                    .as_ref()
                    .map(|range| range.start)
                    .unwrap_or(original_caret);
                let after_edge = selection
                    .as_ref()
                    .map(|range| range.end)
                    .unwrap_or(original_caret);
                let mut start = before_edge.saturating_sub(pending.delete_before as usize);
                while start > 0 && !buffer.is_char_boundary(start) {
                    start -= 1;
                }
                let mut end = after_edge
                    .saturating_add(pending.delete_after as usize)
                    .min(buffer.len());
                while end < buffer.len() && !buffer.is_char_boundary(end) {
                    end += 1;
                }
                let mut surrounding_changed = false;
                if end > after_edge {
                    buffer.replace_range(after_edge..end, "");
                    surrounding_changed = true;
                }
                let deleted_before = before_edge - start;
                if deleted_before > 0 {
                    buffer.replace_range(start..before_edge, "");
                    surrounding_changed = true;
                }
                if surrounding_changed {
                    buffer_changed = true;
                    if let (Some(selection), Some(anchor)) = (selection, original_anchor) {
                        let new_start = selection.start - deleted_before;
                        let new_end = selection.end - deleted_before;
                        if original_caret <= anchor {
                            *caret = new_start;
                            *selection_anchor = Some(new_end);
                        } else {
                            *caret = new_end;
                            *selection_anchor = Some(new_start);
                        }
                    } else {
                        *caret = start;
                        *selection_anchor = None;
                    }
                }
            }
            // 2) commit_string: insert the committed text at the caret
            //    (replacing any selection), advancing the caret past it.
            if let Some(text) = pending.commit
                && caret_edit::insert_str(
                    buffer,
                    caret,
                    selection_anchor,
                    &text,
                    caret_edit::MAX_TEXT_LENGTH,
                )
            {
                buffer_changed = true;
            }

            // 3) A new preedit starts at the cursor after the committed edits.
            //    Its text remains transient, but text-input-v3 requires any
            //    remaining selected committed text to be removed immediately.
            if pending.preedit_received
                && caret_edit::delete_selection(buffer, caret, selection_anchor)
            {
                buffer_changed = true;
            }
        }

        // 4) preedit: replace the active composition (absent → cleared).
        let preedit_changed = self.ime.preedit != pending.preedit;
        self.ime.preedit = pending.preedit;

        if buffer_changed {
            self.note_text_buffer_mutation();
        }
        let changed = buffer_changed || preedit_changed;
        if changed {
            self.needs_redraw = true;
            self.update_text_preview_dirty();
        }
        changed
    }

    /// Drop all composition state (on focus loss / disable / edit exit).
    /// Returns whether a visible preedit was cleared.
    pub fn ime_clear(&mut self) -> bool {
        let had_preedit = self.ime.preedit.is_some();
        self.ime = ImeCompositionState::default();
        if had_preedit {
            self.needs_redraw = true;
            self.update_text_preview_dirty();
        }
        had_preedit
    }
}

/// Build a UTF-8-safe text-input-v3 surrounding-text window. The protocol
/// caps this request at 4000 bytes and requires the complete selection.
fn surrounding_text_window(
    buffer: &str,
    caret: usize,
    anchor: usize,
) -> Option<(String, usize, usize)> {
    let selection_start = caret.min(anchor);
    let selection_end = caret.max(anchor);
    let selection_len = selection_end - selection_start;
    if selection_len > MAX_SURROUNDING_TEXT_BYTES {
        return None;
    }
    if buffer.len() <= MAX_SURROUNDING_TEXT_BYTES {
        return Some((buffer.to_string(), caret, anchor));
    }

    let spare = MAX_SURROUNDING_TEXT_BYTES - selection_len;
    let before_budget = spare / 2;
    let after_budget = spare - before_budget;

    let mut start = selection_start.saturating_sub(before_budget);
    while start < selection_start && !buffer.is_char_boundary(start) {
        start += 1;
    }
    let mut end = selection_end.saturating_add(after_budget).min(buffer.len());
    while end > selection_end && !buffer.is_char_boundary(end) {
        end -= 1;
    }

    Some((
        buffer[start..end].to_string(),
        caret - start,
        anchor - start,
    ))
}

pub(crate) fn build_text_input_preview(
    buffer: &str,
    caret: usize,
    selection: Option<Range<usize>>,
    preedit: Option<&ImePreedit>,
    cursor_glyph: &str,
) -> TextInputPreview {
    let caret = clamp_char_boundary(buffer, caret);

    if let Some(preedit) = preedit {
        if let Some(range) = selection.filter(|range| range.start < range.end) {
            let start = clamp_char_boundary(buffer, range.start);
            let end = clamp_char_boundary(buffer, range.end);
            let mut effective = String::with_capacity(buffer.len() - (end - start));
            effective.push_str(&buffer[..start]);
            effective.push_str(&buffer[end..]);
            return preedit_preview(&effective, start, range, preedit, cursor_glyph);
        }
        return preedit_preview(buffer, caret, caret..caret, preedit, cursor_glyph);
    }

    let highlight = selection.and_then(|selection| {
        let start = clamp_char_boundary(buffer, selection.start);
        let end = clamp_char_boundary(buffer, selection.end);
        (start < end).then_some(start..end)
    });
    TextInputPreview {
        text: buffer.to_string(),
        highlight,
        underline: None,
        caret: Some(caret),
        ime_cursor: Some(caret),
        hit_test_map: None,
    }
}

fn preedit_preview(
    buffer: &str,
    caret: usize,
    buffer_replacement: Range<usize>,
    preedit: &ImePreedit,
    cursor_glyph: &str,
) -> TextInputPreview {
    let pre = &buffer[..caret];
    let post = &buffer[caret..];
    let composed = &preedit.text;
    let base = pre.len();

    let hit_test_map = |inserted_len: usize| {
        Some(TextPreviewHitMap {
            buffer_replacement: buffer_replacement.clone(),
            preview_replacement: base..base + inserted_len,
        })
    };

    if preedit.cursor_begin == -1 && preedit.cursor_end == -1 {
        return TextInputPreview {
            text: format!("{pre}{composed}{post}"),
            highlight: None,
            underline: Some(base..base + composed.len()),
            caret: None,
            ime_cursor: Some(base + composed.len()),
            hit_test_map: hit_test_map(composed.len()),
        };
    }

    let begin = usize::try_from(preedit.cursor_begin)
        .ok()
        .map(|offset| clamp_char_boundary(composed, offset));
    let end = usize::try_from(preedit.cursor_end)
        .ok()
        .map(|offset| clamp_char_boundary(composed, offset));

    match (begin, end) {
        (Some(begin), Some(end)) if begin != end => {
            let start = begin.min(end);
            let finish = begin.max(end);
            TextInputPreview {
                text: format!("{pre}{composed}{post}"),
                highlight: Some(base + start..base + finish),
                underline: Some(base..base + composed.len()),
                caret: None,
                ime_cursor: Some(base + end),
                hit_test_map: hit_test_map(composed.len()),
            }
        }
        (Some(cursor), _) | (None, Some(cursor)) => TextInputPreview {
            text: format!(
                "{pre}{}{cursor_glyph}{}{post}",
                &composed[..cursor],
                &composed[cursor..]
            ),
            highlight: None,
            underline: Some(base..base + composed.len() + cursor_glyph.len()),
            caret: None,
            ime_cursor: Some(base + cursor),
            hit_test_map: hit_test_map(composed.len() + cursor_glyph.len()),
        },
        (None, None) => TextInputPreview {
            text: format!("{pre}{composed}{post}"),
            highlight: None,
            underline: Some(base..base + composed.len()),
            caret: None,
            ime_cursor: Some(base + composed.len()),
            hit_test_map: hit_test_map(composed.len()),
        },
    }
}

fn clamp_char_boundary(s: &str, byte: usize) -> usize {
    let mut idx = byte.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}
