use log::warn;

use crate::draw::Shape;
use crate::draw::shape::{
    VisualCaretDirection, VisualLineDirection, VisualLineEdge, caret_at_visual_selection_edge,
    caret_on_adjacent_visual_line, caret_on_adjacent_visual_position, caret_on_visual_line_edge,
};
use crate::input::events::Key;
use crate::input::state::{
    DrawingState, InputState, TextClipboardRequest, TextCutTarget, TextInputMode, TextPasteEdit,
    TextPasteTarget,
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
                if self.text_edit_target.is_some() {
                    self.cancel_text_input();
                } else {
                    self.end_text_input_session();
                }
                return;
            }

            let shape = match self.text_input_mode {
                TextInputMode::Plain => Shape::Text {
                    x,
                    y,
                    text,
                    color: self.current_color,
                    size: self.current_font_size,
                    font_descriptor: self.font_descriptor.clone(),
                    background_enabled: self.text_background_enabled,
                    wrap_width: self.text_wrap_width,
                },
                TextInputMode::StickyNote => Shape::StickyNote {
                    x,
                    y,
                    text,
                    background: self.current_color,
                    size: self.current_font_size,
                    font_descriptor: self.font_descriptor.clone(),
                    wrap_width: self.text_wrap_width,
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
                        self.pending_text_paste.push_back(target);
                    }
                    return true;
                }
                _ => {}
            }
        }

        let mutates_buffer = match key {
            Key::Char(_) | Key::Space => !ctrl,
            Key::Return => shift,
            Key::Backspace | Key::Delete => true,
            _ => false,
        };
        // Only Pango-resolved navigation needs a font string; building one for
        // every key would allocate on each Escape, F-key, and unbound shortcut
        // that merely passes through text mode.
        let font_for_navigation = matches!(
            key,
            Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End
        )
        .then(|| self.font_descriptor.to_pango_string(self.current_font_size));
        let font = font_for_navigation.as_deref().unwrap_or_default();
        let wrap_width = self.text_wrap_width;

        let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = &mut self.state
        else {
            return false;
        };
        // Guard against a caret desynced by external buffer edits (e.g. IME or
        // tests mutating the buffer directly).
        caret_edit::clamp(buffer, caret, selection_anchor);

        let anchor = selection_anchor;
        let changed = match key {
            // Plain text insertion (Ctrl/Alt combinations fall through to the
            // action layer so shortcuts like undo/exit still work).
            Key::Char(c) if !ctrl && !alt => {
                let mut encoded = [0u8; 4];
                caret_edit::insert_str(
                    buffer,
                    caret,
                    anchor,
                    c.encode_utf8(&mut encoded),
                    MAX_TEXT_LENGTH,
                )
            }
            Key::Char('a' | 'A') if ctrl && !alt => caret_edit::select_all(buffer, caret, anchor),
            Key::Space if !ctrl && !alt => {
                caret_edit::insert_str(buffer, caret, anchor, " ", MAX_TEXT_LENGTH)
            }
            // Shift+Enter inserts a newline; plain Return finalizes below.
            Key::Return if shift => {
                caret_edit::insert_str(buffer, caret, anchor, "\n", MAX_TEXT_LENGTH)
            }
            Key::Backspace if ctrl => caret_edit::delete_word_backward(buffer, caret, anchor),
            Key::Backspace => caret_edit::backspace(buffer, caret, anchor),
            Key::Delete if ctrl => caret_edit::delete_word_forward(buffer, caret, anchor),
            Key::Delete => caret_edit::delete_forward(buffer, caret, anchor),
            Key::Left => move_horizontal_caret(
                buffer,
                font,
                wrap_width,
                caret,
                anchor,
                shift,
                ctrl,
                VisualCaretDirection::Left,
            ),
            Key::Right => move_horizontal_caret(
                buffer,
                font,
                wrap_width,
                caret,
                anchor,
                shift,
                ctrl,
                VisualCaretDirection::Right,
            ),
            Key::Up => caret_on_adjacent_visual_line(
                buffer,
                font,
                wrap_width,
                *caret,
                VisualLineDirection::Up,
            )
            .map(|new| caret_edit::move_to_offset(caret, anchor, shift, new))
            .unwrap_or_else(|| caret_edit::move_up(buffer, caret, anchor, shift)),
            Key::Down => caret_on_adjacent_visual_line(
                buffer,
                font,
                wrap_width,
                *caret,
                VisualLineDirection::Down,
            )
            .map(|new| caret_edit::move_to_offset(caret, anchor, shift, new))
            .unwrap_or_else(|| caret_edit::move_down(buffer, caret, anchor, shift)),
            Key::Home if ctrl => caret_edit::move_document_start(caret, anchor, shift),
            Key::Home => {
                caret_on_visual_line_edge(buffer, font, wrap_width, *caret, VisualLineEdge::Start)
                    .map(|new| caret_edit::move_to_offset(caret, anchor, shift, new))
                    .unwrap_or_else(|| caret_edit::move_line_home(buffer, caret, anchor, shift))
            }
            Key::End if ctrl => caret_edit::move_document_end(buffer, caret, anchor, shift),
            Key::End => {
                caret_on_visual_line_edge(buffer, font, wrap_width, *caret, VisualLineEdge::End)
                    .map(|new| caret_edit::move_to_offset(caret, anchor, shift, new))
                    .unwrap_or_else(|| caret_edit::move_line_end(buffer, caret, anchor, shift))
            }
            // Not an editing key — let the action layer handle it.
            _ => return false,
        };

        if changed {
            if mutates_buffer {
                self.note_text_buffer_mutation();
            }
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

    /// The currently selected text, if any (for copy/cut).
    fn selected_text(&self) -> Option<(std::ops::Range<usize>, String)> {
        let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = &self.state
        else {
            return None;
        };
        let range = caret_edit::selection_range(*caret, *selection_anchor)?;
        buffer
            .get(range.clone())
            .map(|text| (range, text.to_string()))
    }

    /// Capture the selection for a pending copy to the system clipboard.
    /// Returns whether there was a selection to publish.
    fn copy_text_selection(&mut self) -> bool {
        let Some((_, text)) = self.selected_text() else {
            return false;
        };
        self.pending_text_copy
            .push_back(TextClipboardRequest { text, cut: None });
        true
    }

    /// Capture the selection for the clipboard. Deletion is deferred until the
    /// backend confirms successful publication, so a failed copy cannot lose
    /// the user's text. Returns whether there was a selection to publish.
    fn cut_text_selection(&mut self) -> bool {
        let Some((range, text)) = self.selected_text() else {
            return false;
        };
        self.pending_text_copy.push_back(TextClipboardRequest {
            text,
            cut: Some(TextCutTarget {
                generation: self.text_input_generation,
                revision: self.text_input_revision,
                range,
            }),
        });
        true
    }

    /// Insert clipboard text at the caret (replacing any selection). Multi-line
    /// text is kept as-is. Called by the backend once the clipboard read
    /// completes; returns whether the buffer changed.
    pub(crate) fn insert_text_at_caret(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        if let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = &mut self.state
        {
            caret_edit::clamp(buffer, caret, selection_anchor);
            if caret_edit::insert_str(buffer, caret, selection_anchor, text, MAX_TEXT_LENGTH) {
                self.note_text_buffer_mutation();
                self.needs_redraw = true;
                self.update_text_preview_dirty_from_editor();
                return true;
            }
        }
        false
    }

    fn capture_text_paste_target(&self) -> Option<TextPasteTarget> {
        let DrawingState::TextInput {
            caret,
            selection_anchor,
            ..
        } = &self.state
        else {
            return None;
        };
        Some(TextPasteTarget {
            generation: self.text_input_generation,
            revision: self.text_input_revision,
            caret: *caret,
            selection_anchor: *selection_anchor,
        })
    }

    pub(crate) fn text_paste_target_is_current(&self, target: TextPasteTarget) -> bool {
        self.text_input_generation_is_current(target.generation)
            && self.text_input_revision == target.revision
    }

    /// Apply a clipboard result at the selection/caret that invoked it. The
    /// revision check rejects unrelated intervening buffer edits, while plain
    /// caret movement cannot retarget the asynchronous completion.
    pub(crate) fn apply_text_paste(
        &mut self,
        target: TextPasteTarget,
        text: &str,
    ) -> Option<TextPasteEdit> {
        if text.is_empty() || !self.text_paste_target_is_current(target) {
            return None;
        }

        let mut applied = None;
        if let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = &mut self.state
        {
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
            if caret_edit::insert_str(
                buffer,
                &mut target_caret,
                &mut target_anchor,
                text,
                MAX_TEXT_LENGTH,
            ) {
                let inserted_len = buffer.len() + replaced.len() - old_len;
                *caret = target_caret;
                *selection_anchor = target_anchor;
                applied = Some((replaced, inserted_len));
            }
        }

        let (replaced, inserted_len) = applied?;
        self.note_text_buffer_mutation();
        self.needs_redraw = true;
        self.update_text_preview_dirty_from_editor();
        Some(TextPasteEdit {
            generation: target.generation,
            previous_revision: target.revision,
            revision: self.text_input_revision,
            replaced,
            inserted_len,
        })
    }

    /// Take the text captured for a pending clipboard copy/cut, if any.
    pub(crate) fn take_pending_text_copy(&mut self) -> Option<TextClipboardRequest> {
        self.pending_text_copy.pop_front()
    }

    /// Complete a successful text clipboard publication. Copies need no state
    /// change; cuts delete only if the originating selection is still intact.
    pub(crate) fn complete_text_copy(&mut self, request: TextClipboardRequest) {
        let Some(target) = request.cut else {
            return;
        };
        if !self.text_input_generation_is_current(target.generation) {
            return;
        }
        if target.revision != self.text_input_revision {
            return;
        }

        let mut changed = false;
        if let DrawingState::TextInput {
            buffer,
            caret,
            selection_anchor,
            ..
        } = &mut self.state
        {
            caret_edit::clamp(buffer, caret, selection_anchor);
            if caret_edit::selection_range(*caret, *selection_anchor) == Some(target.range.clone())
                && buffer.get(target.range) == Some(request.text.as_str())
            {
                changed = caret_edit::delete_selection(buffer, caret, selection_anchor);
            }
        }
        if changed {
            self.note_text_buffer_mutation();
            self.needs_redraw = true;
            self.update_text_preview_dirty_from_editor();
        }
    }

    /// Take and clear a pending clipboard-paste request.
    pub(crate) fn take_pending_text_paste(&mut self) -> Option<TextPasteTarget> {
        self.pending_text_paste.pop_front()
    }
}

#[allow(clippy::too_many_arguments)]
fn move_horizontal_caret(
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
