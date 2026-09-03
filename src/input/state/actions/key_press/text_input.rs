use log::warn;

use crate::draw::Shape;
use crate::draw::shape::{
    VisualCaretDirection, caret_at_visual_selection_edge, caret_on_adjacent_visual_position,
};
use crate::input::events::Key;
use crate::input::state::core::TextEditing;
use crate::input::state::{
    DrawingState, InputEffect, InputState, TextClipboardRequest, TextCutTarget, TextInputMode,
    TextPasteEdit, TextPasteTarget,
};

use super::bindings::{fallback_unshifted_label, key_to_action_label};
use super::caret_edit::{self, MAX_TEXT_LENGTH};

impl InputState {
    pub(in crate::input::state) fn handle_text_input_key(&mut self, key: Key) {
        // Editing and caret navigation own their keys in text mode, ahead of
        // the action layer — otherwise arrows/Delete/Home/End would be swallowed
        // as tool shortcuts. Escape, F-keys, plain Return, and non-editing
        // Ctrl/Alt shortcuts (undo, exit, …) are left to fall through below.
        if self.handle_text_editing_key(key) {
            return;
        }

        let should_check_actions = match key {
            // Special keys always check for actions
            Key::Escape
            | Key::F1
            | Key::F2
            | Key::F4
            | Key::F9
            | Key::F10
            | Key::F11
            | Key::F12
            | Key::Return
            | Key::Up
            | Key::Down
            | Key::Left
            | Key::Right
            | Key::Delete
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown => true,
            // Character keys only check if modifiers are held
            Key::Char(_) => self.modifiers.ctrl || self.modifiers.alt,
            // Other keys can check as well
            _ => self.modifiers.ctrl || self.modifiers.alt,
        };

        if should_check_actions && let Some(key_str) = key_to_action_label(key) {
            if let Some(action) = self.find_action(&key_str) {
                // Actions work in text mode.
                // Exit action has special logic in handle_action.
                self.handle_action(action);
                return;
            }
            if self.modifiers.shift
                && let Some(fallback) = fallback_unshifted_label(&key_str)
                && let Some(action) = self.find_action(fallback)
            {
                self.handle_action(action);
                return;
            }
        }

        // AltGr is commonly reported as Ctrl+Alt while the produced symbol is
        // still delivered as a printable character. Shortcuts get first
        // refusal above; an unbound AltGr character remains text input.
        if let Key::Char(c) = key
            && self.modifiers.ctrl
            && self.modifiers.alt
            && !c.is_control()
        {
            let mut encoded = [0u8; 4];
            self.insert_text_at_caret(c.encode_utf8(&mut encoded));
            return;
        }

        // Handle Return key for finalizing text input (only plain Return, not Shift+Return)
        if matches!(key, Key::Return) && !self.modifiers.shift {
            let (x, y, text) = if let DrawingState::TextInput { x, y, buffer, .. } = &self.state {
                (*x, *y, buffer.clone())
            } else {
                (0, 0, String::new())
            };

            if text.is_empty() {
                if self.text_editing.edit_target().is_some() {
                    self.cancel_text_input();
                } else {
                    self.end_text_input_session();
                }
                return;
            }

            let shape = match self.text_editing.mode() {
                TextInputMode::Plain => Shape::Text {
                    x,
                    y,
                    text,
                    color: self.style.current_color,
                    size: self.style.current_font_size,
                    font_descriptor: self.style.font_descriptor.clone(),
                    background_enabled: self.style.text_background_enabled,
                    wrap_width: self.style.text_wrap_width,
                },
                TextInputMode::StickyNote => Shape::StickyNote {
                    x,
                    y,
                    text,
                    background: self.style.current_color,
                    size: self.style.current_font_size,
                    font_descriptor: self.style.font_descriptor.clone(),
                    wrap_width: self.style.text_wrap_width,
                },
            };
            let bounds = shape.bounding_box();

            if self.commit_text_edit(shape.clone()) {
                self.end_text_input_session();
                return;
            }

            let added = self
                .boards
                .active_frame_mut()
                .try_add_shape(shape, self.max_shapes_per_frame);
            if added {
                self.dirty_tracker.mark_optional_rect(bounds);
                self.needs_redraw = true;
                self.mark_session_dirty();
            } else {
                warn!(
                    "Shape limit ({}) reached; new text not added",
                    self.max_shapes_per_frame
                );
            }
            self.end_text_input_session();
        }
    }

    /// Apply caret navigation, in-place editing, and selection for keys the
    /// text editor owns. Returns whether the key was consumed (so the caller
    /// stops routing it). Non-editing keys (Escape, F-keys, plain Return, and
    /// Ctrl/Alt shortcuts like undo/exit) return `false` and fall through.
    fn handle_text_editing_key(&mut self, key: Key) -> bool {
        let ctrl = self.modifiers.ctrl;
        let alt = self.modifiers.alt;
        let shift = self.modifiers.shift;

        // No editing operation uses Alt (word ops are Ctrl, selection is Shift),
        // but several default shortcuts do — e.g. Ctrl+Alt+Delete (page delete),
        // Ctrl+Alt+ArrowUp (marker opacity). Never consume an Alt-modified key
        // here; let it fall through to the action layer, which routes those keys
        // (Delete/arrows/Home/End) to `find_action`.
        if alt {
            return false;
        }

        // Preserve established configurable Ctrl+Shift actions (board/page
        // navigation and deletion, capture, command palette, custom bindings)
        // before interpreting the same keys as word-selection/editing commands.
        if ctrl && shift && self.text_input_key_has_configured_action(key) {
            return false;
        }

        // Clipboard shortcuts need InputState-level access (pending requests and
        // selection deletion), so handle them before borrowing the buffer. The
        // backend fulfills the pending copy/paste against the system clipboard.
        //
        // Copy and cut only claim the key when there is something to publish:
        // with a collapsed caret they have no work to do, and swallowing them
        // would make configured Ctrl+C/Ctrl+X actions (screen capture by
        // default) silently unreachable for as long as an editor is open.
        if ctrl && !alt && self.is_text_input_active() {
            match key {
                Key::Char('c' | 'C') if self.copy_text_selection() => return true,
                Key::Char('x' | 'X') if self.cut_text_selection() => return true,
                Key::Char('v' | 'V') => {
                    if let Some(target) = self.capture_text_paste_target() {
                        self.emit_input_effect(InputEffect::TextPaste(target));
                    }
                    return true;
                }
                _ => {}
            }
        }

        // Only visual navigation needs a font string; avoid allocating one for
        // keys that pass through text mode.
        let font_for_navigation = matches!(
            key,
            Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End
        )
        .then(|| {
            self.style
                .font_descriptor
                .to_pango_string(self.style.current_font_size)
        });
        let changed = match self.text_editing.apply_key_edit(
            &mut self.state,
            key,
            ctrl,
            shift,
            font_for_navigation.as_deref().unwrap_or_default(),
            self.style.text_wrap_width,
        ) {
            Some(changed) => changed,
            None => return false,
        };
        if changed {
            self.needs_redraw = true;
            self.update_text_preview_dirty_from_editor();
        }
        true
    }

    fn text_input_key_has_configured_action(&self, key: Key) -> bool {
        let Some(key_str) = key_to_action_label(key) else {
            return false;
        };
        self.find_action(&key_str).is_some()
            || (self.modifiers.shift
                && fallback_unshifted_label(&key_str)
                    .is_some_and(|fallback| self.find_action(fallback).is_some()))
    }

    /// Capture the selection for a pending copy to the system clipboard.
    fn copy_text_selection(&mut self) -> bool {
        let Some(request) = self.text_editing.copy_request(&self.state) else {
            return false;
        };
        self.emit_input_effect(InputEffect::TextCopy(request));
        true
    }

    /// Capture the selection for the clipboard. Deletion remains deferred until
    /// the backend confirms successful publication.
    fn cut_text_selection(&mut self) -> bool {
        let Some(request) = self.text_editing.cut_request(&self.state) else {
            return false;
        };
        self.emit_input_effect(InputEffect::TextCopy(request));
        true
    }

    /// Insert clipboard text at the caret, then coordinate redraw and protocol
    /// effects owned by the root state.
    pub(crate) fn insert_text_at_caret(&mut self, text: &str) -> bool {
        let changed = self.text_editing.insert_text(&mut self.state, text);
        if changed {
            self.needs_redraw = true;
            self.update_text_preview_dirty_from_editor();
        }
        changed
    }

    fn capture_text_paste_target(&self) -> Option<TextPasteTarget> {
        self.text_editing.capture_paste_target(&self.state)
    }

    pub(crate) fn text_paste_target_is_current(&self, target: TextPasteTarget) -> bool {
        self.text_editing
            .paste_target_is_current(&self.state, target)
    }

    pub(crate) fn apply_text_paste(
        &mut self,
        target: TextPasteTarget,
        text: &str,
    ) -> Option<TextPasteEdit> {
        let edit = self
            .text_editing
            .apply_paste(&mut self.state, target, text)?;
        self.needs_redraw = true;
        self.update_text_preview_dirty_from_editor();
        Some(edit)
    }

    pub(crate) fn complete_text_copy(&mut self, request: TextClipboardRequest) {
        if self.text_editing.complete_copy(&mut self.state, request) {
            self.needs_redraw = true;
            self.update_text_preview_dirty_from_editor();
        }
    }
}

impl TextEditing {
    fn selected_text(&self, state: &DrawingState) -> Option<(std::ops::Range<usize>, String)> {
        let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = state
        else {
            return None;
        };
        let range = caret_edit::selection_range(*caret, *selection_anchor)?;
        buffer
            .get(range.clone())
            .map(|text| (range, text.to_string()))
    }

    fn copy_request(&self, state: &DrawingState) -> Option<TextClipboardRequest> {
        let (_, text) = self.selected_text(state)?;
        Some(TextClipboardRequest { text, cut: None })
    }

    fn cut_request(&self, state: &DrawingState) -> Option<TextClipboardRequest> {
        let (range, text) = self.selected_text(state)?;
        let (generation, revision) = self.session_identity();
        Some(TextClipboardRequest {
            text,
            cut: Some(TextCutTarget {
                generation,
                revision,
                range,
            }),
        })
    }

    fn insert_text(&mut self, state: &mut DrawingState, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = state
        else {
            return false;
        };
        caret_edit::clamp(buffer, caret, selection_anchor);
        if !caret_edit::insert_str(buffer, caret, selection_anchor, text, MAX_TEXT_LENGTH) {
            return false;
        }
        self.note_buffer_mutation();
        true
    }

    fn capture_paste_target(&self, state: &DrawingState) -> Option<TextPasteTarget> {
        let DrawingState::TextInput {
            caret,
            selection_anchor,
            ..
        } = state
        else {
            return None;
        };
        let (generation, revision) = self.session_identity();
        Some(TextPasteTarget {
            generation,
            revision,
            caret: *caret,
            selection_anchor: *selection_anchor,
        })
    }

    fn paste_target_is_current(&self, state: &DrawingState, target: TextPasteTarget) -> bool {
        self.generation_is_current(state, target.generation) && self.revision() == target.revision
    }

    fn apply_paste(
        &mut self,
        state: &mut DrawingState,
        target: TextPasteTarget,
        text: &str,
    ) -> Option<TextPasteEdit> {
        if text.is_empty() || !self.paste_target_is_current(state, target) {
            return None;
        }

        let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = state
        else {
            return None;
        };
        let valid_target = target.caret <= buffer.len()
            && buffer.is_char_boundary(target.caret)
            && target
                .selection_anchor
                .is_none_or(|anchor| anchor <= buffer.len() && buffer.is_char_boundary(anchor));
        if !valid_target {
            return None;
        }

        let replaced = caret_edit::selection_range(target.caret, target.selection_anchor)
            .unwrap_or(target.caret..target.caret);
        let old_len = buffer.len();
        let mut target_caret = target.caret;
        let mut target_anchor = target.selection_anchor;
        if !caret_edit::insert_str(
            buffer,
            &mut target_caret,
            &mut target_anchor,
            text,
            MAX_TEXT_LENGTH,
        ) {
            return None;
        }

        let inserted_len = buffer.len() + replaced.len() - old_len;
        *caret = target_caret;
        *selection_anchor = target_anchor;
        self.note_buffer_mutation();
        Some(TextPasteEdit {
            generation: target.generation,
            previous_revision: target.revision,
            revision: self.revision(),
            replaced,
            inserted_len,
        })
    }

    fn complete_copy(&mut self, state: &mut DrawingState, request: TextClipboardRequest) -> bool {
        let Some(target) = request.cut else {
            return false;
        };
        if !self.generation_is_current(state, target.generation)
            || target.revision != self.revision()
        {
            return false;
        }

        let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = state
        else {
            return false;
        };
        caret_edit::clamp(buffer, caret, selection_anchor);
        if caret_edit::selection_range(*caret, *selection_anchor) != Some(target.range.clone())
            || buffer.get(target.range) != Some(request.text.as_str())
        {
            return false;
        }
        if !caret_edit::delete_selection(buffer, caret, selection_anchor) {
            return false;
        }
        self.note_buffer_mutation();
        true
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn move_horizontal_caret(
    buffer: &str,
    font: &str,
    wrap_width: Option<i32>,
    caret: &mut usize,
    anchor: &mut Option<usize>,
    extend: bool,
    by_word: bool,
    direction: VisualCaretDirection,
) -> bool {
    if !extend && let Some(range) = caret_edit::selection_range(*caret, *anchor) {
        let fallback = match direction {
            VisualCaretDirection::Left => range.start,
            VisualCaretDirection::Right => range.end,
        };
        let target = caret_at_visual_selection_edge(
            buffer,
            font,
            wrap_width,
            range.start,
            range.end,
            direction,
        )
        .unwrap_or(fallback);
        return caret_edit::move_to_offset(caret, anchor, false, target);
    }

    let Some(adjacent) =
        caret_on_adjacent_visual_position(buffer, font, wrap_width, *caret, direction)
    else {
        return match (direction, by_word) {
            (VisualCaretDirection::Left, false) => {
                caret_edit::move_left(buffer, caret, anchor, extend)
            }
            (VisualCaretDirection::Right, false) => {
                caret_edit::move_right(buffer, caret, anchor, extend)
            }
            (VisualCaretDirection::Left, true) => {
                caret_edit::move_word_left(buffer, caret, anchor, extend)
            }
            (VisualCaretDirection::Right, true) => {
                caret_edit::move_word_right(buffer, caret, anchor, extend)
            }
        };
    };
    if !by_word {
        return caret_edit::move_to_offset(caret, anchor, extend, adjacent);
    }

    match adjacent.cmp(caret) {
        std::cmp::Ordering::Less => caret_edit::move_word_left(buffer, caret, anchor, extend),
        std::cmp::Ordering::Greater => caret_edit::move_word_right(buffer, caret, anchor, extend),
        std::cmp::Ordering::Equal => caret_edit::move_to_offset(caret, anchor, extend, adjacent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_paste_target_is_rejected_after_an_owner_managed_edit() {
        let mut editing = TextEditing::default();
        let mut state = DrawingState::text_input(0, 0, "ab".to_string());
        editing.begin_session();
        let target = editing.capture_paste_target(&state).expect("active target");

        assert!(editing.insert_text(&mut state, "c"));
        assert!(editing.apply_paste(&mut state, target, "stale").is_none());
        assert_eq!(editing.revision(), 1);
        assert!(matches!(
            state,
            DrawingState::TextInput { ref buffer, .. } if buffer == "abc"
        ));
    }

    #[test]
    fn successful_cut_completion_deletes_only_the_captured_selection() {
        let mut editing = TextEditing::default();
        let mut state = DrawingState::TextInput {
            x: 0,
            y: 0,
            buffer: "hello".to_string(),
            caret: 4,
            selection_anchor: Some(1),
        };
        editing.begin_session();
        let request = editing.cut_request(&state).expect("selected text");

        assert!(editing.complete_copy(&mut state, request));
        assert_eq!(editing.revision(), 1);
        assert!(matches!(
            state,
            DrawingState::TextInput { ref buffer, caret: 1, selection_anchor: None, .. }
                if buffer == "ho"
        ));
    }
}
