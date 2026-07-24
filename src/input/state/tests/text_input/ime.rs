//! IME (zwp_text_input_v3) composition state-machine tests. These drive the
//! `ime_queue_*` / `ime_apply_done` API the backend Wayland handlers call,
//! and assert the append-only editor buffer + transient preedit behave per
//! the protocol's batch-then-done model.

use super::super::*;

fn enter_text_mode(state: &mut InputState) {
    state.state = DrawingState::TextInput {
        x: 100,
        y: 100,
        buffer: String::new(),
    };
}

fn buffer(state: &InputState) -> String {
    match &state.state {
        DrawingState::TextInput { buffer, .. } => buffer.clone(),
        other => panic!("expected TextInput, got {other:?}"),
    }
}

#[test]
fn commit_string_appends_to_the_buffer_on_done() {
    let mut state = create_test_input_state();
    enter_text_mode(&mut state);

    state.ime_queue_commit(Some("你好".to_string()));
    assert_eq!(buffer(&state), "", "nothing applies before done");
    assert!(state.ime_apply_done());
    assert_eq!(buffer(&state), "你好");
    assert!(
        state.ime_preedit().is_none(),
        "no preedit after a pure commit"
    );
}

#[test]
fn preedit_is_transient_and_not_part_of_the_buffer() {
    let mut state = create_test_input_state();
    enter_text_mode(&mut state);

    // Compose: a preedit shows but is not committed text yet.
    state.ime_queue_preedit(Some("ni".to_string()), 2, 2);
    assert!(state.ime_apply_done());
    assert_eq!(buffer(&state), "", "preedit never enters the buffer");
    assert_eq!(state.ime_preedit().map(|p| p.text.as_str()), Some("ni"));

    // Commit replaces the preedit with real text and clears the preedit.
    state.ime_queue_commit(Some("你".to_string()));
    state.ime_queue_preedit(None, 0, 0);
    assert!(state.ime_apply_done());
    assert_eq!(buffer(&state), "你");
    assert!(state.ime_preedit().is_none());
}

#[test]
fn preedit_clears_when_no_preedit_is_queued_next_cycle() {
    let mut state = create_test_input_state();
    enter_text_mode(&mut state);

    state.ime_queue_preedit(Some("wip".to_string()), 3, 3);
    state.ime_apply_done();
    assert!(state.ime_preedit().is_some());

    // A done with an empty batch resets the preedit (protocol semantics).
    assert!(state.ime_apply_done());
    assert!(state.ime_preedit().is_none());
}

#[test]
fn delete_surrounding_text_trims_bytes_before_the_caret_then_commits() {
    let mut state = create_test_input_state();
    enter_text_mode(&mut state);
    state.ime_queue_commit(Some("abcd".to_string()));
    state.ime_apply_done();
    assert_eq!(buffer(&state), "abcd");

    // Delete the last 2 bytes, then commit a replacement (Korean/Japanese
    // correction style): delete applies before the commit insert.
    state.ime_queue_delete_surrounding(2, 0);
    state.ime_queue_commit(Some("XY".to_string()));
    assert!(state.ime_apply_done());
    assert_eq!(buffer(&state), "abXY");
}

#[test]
fn delete_surrounding_text_respects_utf8_char_boundaries() {
    let mut state = create_test_input_state();
    enter_text_mode(&mut state);
    state.ime_queue_commit(Some("a你".to_string())); // '你' is 3 bytes
    state.ime_apply_done();
    assert_eq!(buffer(&state), "a你");

    // Asking to delete 1 byte must not split the 3-byte char; it snaps back
    // to the char boundary and removes the whole '你'.
    state.ime_queue_delete_surrounding(1, 0);
    assert!(state.ime_apply_done());
    assert_eq!(buffer(&state), "a");
}

#[test]
fn null_commit_string_cancels_an_earlier_queued_commit() {
    let mut state = create_test_input_state();
    enter_text_mode(&mut state);

    // A compositor may queue a commit and then retract it with a null
    // commit_string before the same done; the retraction must win.
    state.ime_queue_commit(Some("draft".to_string()));
    state.ime_queue_commit(None);
    assert!(
        !state.ime_apply_done(),
        "a cancelled commit leaves nothing to apply"
    );
    assert_eq!(
        buffer(&state),
        "",
        "the retracted text must not be inserted"
    );
}

#[test]
fn help_overlay_owns_input_while_canvas_text_edit_remains_active() {
    let mut state = create_test_input_state();
    enter_text_mode(&mut state);

    state.show_help = true;

    assert!(state.is_text_input_active());
    assert!(
        state.modal_owns_text_input(),
        "Help routes text into its search field, not the hidden canvas editor"
    );
    assert!(
        !state.modal_blocks_canvas_key_repeat(),
        "Help search still relies on the backend's routed repeat timer"
    );
}

#[test]
fn ime_events_are_ignored_outside_text_mode() {
    let mut state = create_test_input_state();
    // Not in text mode.
    state.ime_queue_commit(Some("nope".to_string()));
    state.ime_queue_preedit(Some("nope".to_string()), 0, 0);
    assert!(!state.ime_apply_done(), "no-op when no text edit is active");
    assert!(state.ime_preedit().is_none());
    assert!(matches!(state.state, DrawingState::Idle));
}

#[test]
fn ime_clear_drops_the_active_preedit() {
    let mut state = create_test_input_state();
    enter_text_mode(&mut state);
    state.ime_queue_preedit(Some("half".to_string()), 4, 4);
    state.ime_apply_done();
    assert!(state.ime_preedit().is_some());

    assert!(
        state.ime_clear(),
        "clearing an active preedit reports a change"
    );
    assert!(state.ime_preedit().is_none());
    assert!(!state.ime_clear(), "clearing again is a no-op");
}

#[test]
fn finalizing_the_edit_drops_composition_state() {
    let mut state = create_test_input_state();
    enter_text_mode(&mut state);
    state.ime_queue_commit(Some("hi".to_string()));
    state.ime_apply_done();
    state.ime_queue_preedit(Some("mid".to_string()), 3, 3);
    state.ime_apply_done();
    assert!(state.ime_preedit().is_some());

    // Return commits the text and must leave no dangling preedit.
    state.on_key_press(Key::Return);
    assert!(state.ime_preedit().is_none());
    assert!(matches!(state.state, DrawingState::Idle));
}
