//! `zwp_text_input_v3` (IME) event handling for the text/note editor.
//!
//! The manager is bound at startup and a seat-bound `text_input` object is
//! created when a keyboard capability appears (see `handlers/seat.rs`). This
//! module translates the protocol's batched events into the `InputState`
//! IME state machine (`ime_queue_*` / `ime_apply_done`) and drives the
//! enable/disable lifecycle against the current text-edit state.
//!
//! Coordination with raw keys: while an input method is composing, the
//! compositor consumes the keys and delivers `preedit_string`/`commit_string`
//! instead of `wl_keyboard` key events, so there is no double-insertion — the
//! existing keysym path only fires for keys the IME does not consume.
//!
//! Single-seat scope: exactly one `text_input` object is created, bound to the
//! first seat that advertises a keyboard (with lifecycle failover; see
//! `handlers/seat.rs`). Simultaneous IME on multiple seats, or a touch-only
//! seat's on-screen keyboard, are out of scope — the target is a physical
//! keyboard driving fcitx5/ibus-style input.

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{self, ChangeCause, ContentHint, ContentPurpose, ZwpTextInputV3},
};

use crate::backend::wayland::state::WaylandState;

impl Dispatch<ZwpTextInputManagerV3, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTextInputManagerV3,
        _event: <ZwpTextInputManagerV3 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // The manager has no events.
    }
}

impl Dispatch<ZwpTextInputV3, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTextInputV3,
        event: <ZwpTextInputV3 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use zwp_text_input_v3::Event;
        match event {
            // Compositor text-input focus gained/lost. Only our overlay
            // surface counts; enable() is driven from the reconcile below,
            // which also requires an active text edit.
            Event::Enter { surface } if state.surface.is_surface(&surface) => {
                state.text_input.enter();
                state.reconcile_text_input();
            }
            Event::Leave { surface } if state.surface.is_surface(&surface) => {
                // Leave invalidates the focused surface and compositor
                // state. Requests are ignored until the next Enter, so
                // clear only local state and preserve the commit serial.
                state.text_input.leave();
                state.input_state.ime_clear();
                state.input_state.take_text_input_cursor_rect_dirty();
                state.input_state.take_text_input_external_change_dirty();
            }
            // Batched composition events: accumulate, apply on Done.
            Event::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                state
                    .input_state
                    .ime_queue_preedit(text, cursor_begin, cursor_end);
            }
            // A null `commit_string` overwrites (retracts) any commit queued
            // earlier in the same batch — the pending state is double-buffered.
            Event::CommitString { text } => {
                state.input_state.ime_queue_commit(text);
            }
            Event::DeleteSurroundingText {
                before_length,
                after_length,
            } => {
                state
                    .input_state
                    .ime_queue_delete_surrounding(before_length, after_length);
            }
            Event::Done { serial } => state.on_ime_done(serial),
            // Future protocol additions are ignored until explicitly supported.
            _ => {}
        }
    }
}

impl WaylandState {
    /// Apply a completed IME batch (`done`) to the editor and, if the text
    /// moved, refresh the caret rectangle so the candidate popup follows the
    /// composition. `needs_redraw` is set by the state machine.
    ///
    /// The editor changes are always applied (never drop text the user has
    /// committed), but the follow-up caret-rectangle commit is sent only when
    /// the compositor has processed all of our commits — a `done` whose serial
    /// is behind our commit count is a reply to a superseded
    /// enable/disable/update still in flight, and committing more state against
    /// it would only pile on out-of-order updates.
    fn on_ime_done(&mut self, serial: u32) {
        let editor_changed = self.input_state.ime_apply_done();
        let cursor_dirty = self.input_state.take_text_input_cursor_rect_dirty();
        self.text_input.collect_editor_changes(
            cursor_dirty,
            self.input_state.take_text_input_external_change_dirty(),
        );
        if !self
            .text_input
            .cursor_update_ready_after_done(editor_changed, serial)
        {
            return;
        }
        let Some(ti) = self.text_input.protocol() else {
            return;
        };
        if !self.report_text_cursor_rectangle(&ti, self.text_input.external_change_pending()) {
            return;
        }
        ti.commit();
        self.text_input.cursor_update_committed();
    }

    /// Reconcile the text-input enable state against compositor focus and the
    /// active text edit: enable when both hold, disable otherwise. Idempotent
    /// and cheap to call per frame (see the event loop) so entering/leaving
    /// text mode toggles the IME without hooking every edit-mode transition.
    pub(in crate::backend::wayland) fn reconcile_text_input(&mut self) {
        self.text_input.collect_editor_changes(
            self.input_state.take_text_input_cursor_rect_dirty(),
            self.input_state.take_text_input_external_change_dirty(),
        );
        let Some(ti) = self.text_input.protocol() else {
            return;
        };
        // Stay disabled while another routed interaction owns keyboard input:
        // an enabled IME would commit composed text straight into the hidden
        // canvas buffer instead of Help search, the command palette, or the
        // active modal.
        //
        // Also stay disabled while the complete selection cannot fit the
        // protocol's bounded surrounding-text request. Disabling clears stale
        // context without applying an empty value that some compositors treat
        // as permanently unsupported; collapsing the selection enables again
        // with fresh data.
        let desired = self.text_input.is_focused()
            && self.input_state.is_text_input_active()
            && !self.input_state.modal_owns_text_input()
            && self.input_state.text_input_surrounding_available();
        if desired != self.text_input.is_enabled() && desired {
            ti.enable();
            ti.set_content_type(ContentHint::empty(), ContentPurpose::Normal);
            self.report_text_cursor_rectangle(&ti, false);
            ti.commit();
            self.text_input.enabled_committed();
        } else if desired != self.text_input.is_enabled() {
            ti.disable();
            ti.commit();
            self.text_input.disabled_committed();
            self.input_state.ime_clear();
            self.input_state.take_text_input_cursor_rect_dirty();
            self.input_state.take_text_input_external_change_dirty();
        } else if self.text_input.cursor_update_ready()
            && self.report_text_cursor_rectangle(&ti, self.text_input.external_change_pending())
        {
            ti.commit();
            self.text_input.cursor_update_committed();
        }
    }

    /// Report a best-effort caret rectangle so the IME positions its
    /// candidate popup near the composition. `set_cursor_rectangle` takes
    /// surface-local coordinates, but the cached preview bounds are in canvas
    /// space, so convert them through the active zoom/pan transform first.
    fn report_text_cursor_rectangle(&self, ti: &ZwpTextInputV3, external_change: bool) -> bool {
        if external_change {
            ti.set_text_change_cause(ChangeCause::Other);
        }
        if let Some((text, cursor, anchor)) = self.input_state.text_input_surrounding_state()
            && let (Ok(cursor), Ok(anchor)) = (i32::try_from(cursor), i32::try_from(anchor))
        {
            ti.set_surrounding_text(text, cursor, anchor);
        }

        // Prefer the exact caret position so the candidate popup sits at the
        // composition point — correct mid-buffer and in wrapped/multiline text.
        if let Some(caret_canvas) = self.input_state.caret_cursor_rect_canvas()
            && let Some(rect) = self.input_state.screen_rect_for_canvas(caret_canvas)
        {
            ti.set_cursor_rectangle(rect.x, rect.y, rect.width.clamp(1, 4), rect.height.max(1));
            return true;
        }

        // Fallback: the right edge of the cached preview bounds, kept within the
        // transformed preview even when a small zoom rounds it to one pixel.
        let Some(canvas_rect) = self.input_state.last_text_preview_bounds else {
            return false;
        };
        let Some(rect) = self.input_state.screen_rect_for_canvas(canvas_rect) else {
            return false;
        };
        let width = rect.width.max(1);
        let height = rect.height.max(1);
        let caret_width = width.min(2);
        ti.set_cursor_rectangle(
            rect.x.saturating_add(width - caret_width),
            rect.y,
            caret_width,
            height,
        );
        true
    }
}
