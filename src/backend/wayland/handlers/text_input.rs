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
    zwp_text_input_v3::{self, ContentHint, ContentPurpose, ZwpTextInputV3},
};

use crate::backend::wayland::state::WaylandState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextInputLocalTransition {
    EnableCommitted,
    DisableCommitted,
    Leave,
}

/// Keep local lifecycle state aligned with requests the compositor actually
/// counts. Focus events invalidate pending cursor/preedit state independently
/// of enable/disable commits.
fn apply_text_input_local_transition(
    enabled: &mut bool,
    committed_serial: &mut u32,
    cursor_update_pending: &mut bool,
    transition: TextInputLocalTransition,
) {
    *enabled = matches!(transition, TextInputLocalTransition::EnableCommitted);
    if transition != TextInputLocalTransition::Leave {
        *committed_serial = committed_serial.wrapping_add(1);
    }
    *cursor_update_pending = false;
}

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
                state.text_input_focused = true;
                state.reconcile_text_input();
            }
            Event::Leave { surface } if state.surface.is_surface(&surface) => {
                // Leave invalidates the focused surface and compositor
                // state. Requests are ignored until the next Enter, so
                // clear only local state and preserve the commit serial.
                state.text_input_focused = false;
                apply_text_input_local_transition(
                    &mut state.text_input_enabled,
                    &mut state.text_input_serial,
                    &mut state.text_input_cursor_update_pending,
                    TextInputLocalTransition::Leave,
                );
                state.input_state.ime_clear();
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
        if !cursor_update_ready_after_done(
            &mut self.text_input_cursor_update_pending,
            editor_changed,
            self.text_input_enabled,
            serial,
            self.text_input_serial,
        ) {
            return;
        }
        let Some(ti) = self.text_input.clone() else {
            return;
        };
        self.report_text_cursor_rectangle(&ti);
        ti.commit();
        self.text_input_serial = self.text_input_serial.wrapping_add(1);
        self.text_input_cursor_update_pending = false;
    }

    /// Reconcile the text-input enable state against compositor focus and the
    /// active text edit: enable when both hold, disable otherwise. Idempotent
    /// and cheap to call per frame (see the event loop) so entering/leaving
    /// text mode toggles the IME without hooking every edit-mode transition.
    pub(in crate::backend::wayland) fn reconcile_text_input(&mut self) {
        let Some(ti) = self.text_input.clone() else {
            return;
        };
        // Stay disabled while another routed interaction owns keyboard input:
        // an enabled IME would commit composed text straight into the hidden
        // canvas buffer instead of Help search, the command palette, or the
        // active modal.
        let desired = self.text_input_focused
            && self.input_state.is_text_input_active()
            && !self.input_state.modal_owns_text_input();
        if desired == self.text_input_enabled {
            return;
        }
        if desired {
            ti.enable();
            ti.set_content_type(ContentHint::empty(), ContentPurpose::Normal);
            self.report_text_cursor_rectangle(&ti);
            ti.commit();
            apply_text_input_local_transition(
                &mut self.text_input_enabled,
                &mut self.text_input_serial,
                &mut self.text_input_cursor_update_pending,
                TextInputLocalTransition::EnableCommitted,
            );
        } else {
            ti.disable();
            ti.commit();
            apply_text_input_local_transition(
                &mut self.text_input_enabled,
                &mut self.text_input_serial,
                &mut self.text_input_cursor_update_pending,
                TextInputLocalTransition::DisableCommitted,
            );
            self.input_state.ime_clear();
        }
    }

    /// Report a best-effort caret rectangle so the IME positions its
    /// candidate popup near the composition. `set_cursor_rectangle` takes
    /// surface-local coordinates, but the cached preview bounds are in canvas
    /// space, so convert them through the active zoom/pan transform first.
    fn report_text_cursor_rectangle(&self, ti: &ZwpTextInputV3) {
        let Some(canvas_rect) = self.input_state.last_text_preview_bounds else {
            return;
        };
        let Some(rect) = self.input_state.screen_rect_for_canvas(canvas_rect) else {
            return;
        };
        let width = rect.width.max(1);
        let height = rect.height.max(1);
        let caret_width = width.min(2);
        // A thin caret strip at the right edge (the caret is always at the
        // end of the buffer in this append-only editor). Keep it within the
        // transformed preview even when a small zoom rounds that preview down
        // to a single surface pixel.
        ti.set_cursor_rectangle(
            rect.x.saturating_add(width - caret_width),
            rect.y,
            caret_width,
            height,
        );
    }
}

/// Record editor movement from every `done`, but publish client cursor state
/// only after the compositor reports the current commit generation. A stale
/// batch therefore defers rather than loses its caret update.
fn cursor_update_ready_after_done(
    pending: &mut bool,
    editor_changed: bool,
    enabled: bool,
    done_serial: u32,
    committed_serial: u32,
) -> bool {
    *pending |= editor_changed;
    enabled && done_serial == committed_serial && *pending
}

#[cfg(test)]
mod tests {
    use super::{
        TextInputLocalTransition, apply_text_input_local_transition, cursor_update_ready_after_done,
    };

    #[test]
    fn leave_preserves_the_last_compositor_visible_commit_serial() {
        let mut enabled = true;
        let mut committed_serial = 4;
        let mut cursor_update_pending = true;

        apply_text_input_local_transition(
            &mut enabled,
            &mut committed_serial,
            &mut cursor_update_pending,
            TextInputLocalTransition::Leave,
        );

        assert!(!enabled, "leave clears the local enabled state");
        assert!(!cursor_update_pending, "leave invalidates the old caret");
        assert_eq!(
            committed_serial, 4,
            "requests after leave are ignored and must not advance the serial"
        );

        apply_text_input_local_transition(
            &mut enabled,
            &mut committed_serial,
            &mut cursor_update_pending,
            TextInputLocalTransition::EnableCommitted,
        );
        assert_eq!(
            committed_serial, 5,
            "the next enter's enable commit remains synchronized"
        );
    }

    #[test]
    fn stale_done_defers_cursor_update_until_a_matching_serial() {
        let mut pending = false;

        assert!(!cursor_update_ready_after_done(
            &mut pending,
            true,
            true,
            2,
            3
        ));
        assert!(
            pending,
            "the stale batch's editor movement must be retained"
        );

        assert!(cursor_update_ready_after_done(
            &mut pending,
            false,
            true,
            3,
            3
        ));
    }

    #[test]
    fn disabled_text_input_retains_update_until_it_can_be_reconciled() {
        let mut pending = false;

        assert!(!cursor_update_ready_after_done(
            &mut pending,
            true,
            false,
            4,
            4
        ));
        assert!(pending);
    }
}
