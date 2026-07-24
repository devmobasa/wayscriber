//! Input-method (IME) composition state for the text/note editor.
//!
//! Drives the `zwp_text_input_v3` protocol on the state side: the backend
//! Wayland handlers translate protocol events into the `ime_queue_*` calls
//! below and apply them atomically on `ime_apply_done`, exactly mirroring
//! the protocol's double-buffered "batch then done" model. Keeping the whole
//! state machine here (off the Wayland types) makes it unit-testable.
//!
//! The text editor is append-only with an implicit caret at the end of the
//! buffer (see `DrawingState::TextInput`), so:
//! - `commit_string` appends to the buffer (like typing),
//! - `delete_surrounding_text` trims bytes from the end,
//! - the preedit (in-progress composition) is a transient overlay drawn
//!   after the buffer, never part of the committed text until the IME
//!   commits it.

use super::super::{DrawingState, InputState};

/// The active preedit (in-progress composition) shown after the buffer.
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

impl ImeCompositionState {
    /// The active preedit run for rendering, or `None` when nothing is
    /// being composed.
    pub fn preedit(&self) -> Option<&ImePreedit> {
        self.preedit.as_ref()
    }
}

impl InputState {
    /// True while a text/note edit is in progress — the gate for enabling
    /// the text-input protocol.
    pub fn is_text_input_active(&self) -> bool {
        matches!(self.state, DrawingState::TextInput { .. })
    }

    /// True when another interaction captures keyboard input ahead of the
    /// canvas editor. While one is active the canvas IME must stay disabled:
    /// composed text bypasses normal key routing and would otherwise leak
    /// straight into the hidden canvas buffer.
    pub fn modal_owns_text_input(&self) -> bool {
        self.tour_active
            || self.command_palette_is_engaged()
            || self.show_help
            || self.is_radial_menu_open()
            || self.is_color_picker_popup_open()
            || self.is_precision_entry_open()
            || self.is_context_menu_open()
            || self.is_board_picker_open()
            || self.is_properties_panel_open()
            || self.eyedropper_is_engaged()
    }

    /// Modal paths whose editing/repeat behavior must not be driven by the
    /// backend's canvas-oriented manual repeat timer. Kept narrower than
    /// [`Self::modal_owns_text_input`]: Help and board-picker search still use
    /// the normal routed repeat path even though they must disable the canvas
    /// IME.
    pub fn modal_blocks_canvas_key_repeat(&self) -> bool {
        self.command_palette_is_engaged()
            || self.is_color_picker_popup_open()
            || self.is_precision_entry_open()
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
    /// `done`. `text = None` clears the preedit.
    pub fn ime_queue_preedit(&mut self, text: Option<String>, cursor_begin: i32, cursor_end: i32) {
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

        if let DrawingState::TextInput { buffer, .. } = &mut self.state {
            // 1) delete_surrounding_text: trim `before` bytes from the caret
            //    (the end). `after` addresses text past the caret, of which
            //    there is none in this append-only editor, so it is ignored.
            if pending.delete_before > 0 {
                let target = buffer.len().saturating_sub(pending.delete_before as usize);
                let mut cut = target;
                while cut > 0 && !buffer.is_char_boundary(cut) {
                    cut -= 1;
                }
                if cut < buffer.len() {
                    buffer.truncate(cut);
                    buffer_changed = true;
                }
            }
            // 2) commit_string: insert (append) the committed text.
            if let Some(text) = pending.commit {
                for ch in text.chars() {
                    if !Self::push_text_char(buffer, ch) {
                        break;
                    }
                }
                buffer_changed = true;
            }
        }

        // 3) preedit: replace the active composition (absent → cleared).
        let preedit_changed = self.ime.preedit != pending.preedit;
        self.ime.preedit = pending.preedit;

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
