//! IME (zwp_text_input_v3) composition state-machine tests. These drive the
//! `ime_queue_*` / `ime_apply_done` API the backend Wayland handlers call,
//! and assert the caret-aware editor buffer + transient preedit behave per
//! the protocol's batch-then-done model.

use super::super::*;

fn enter_text_mode(state: &mut InputState) {
    state.state = DrawingState::text_input(100, 100, String::new());
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
        !state.take_text_input_external_change_dirty(),
        "IME-authored text retains the protocol's InputMethod change cause"
    );
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
fn commit_inserts_at_the_caret_and_advances_it() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, "ad".to_string());
    // Place the caret between 'a' and 'd'.
    if let DrawingState::TextInput { caret, .. } = &mut state.state {
        *caret = 1;
    }

    state.ime_queue_commit(Some("bc".to_string()));
    assert!(state.ime_apply_done());
    assert_eq!(
        buffer(&state),
        "abcd",
        "commit lands at the caret, not the end"
    );
    if let DrawingState::TextInput { caret, .. } = &state.state {
        assert_eq!(*caret, 3, "caret advances past the inserted text");
    } else {
        panic!("expected TextInput");
    }
}

#[test]
fn delete_surrounding_trims_before_the_caret_not_the_buffer_end() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, "abXcd".to_string());
    // Caret sits just after 'X' (byte 3).
    if let DrawingState::TextInput { caret, .. } = &mut state.state {
        *caret = 3;
    }

    // Delete one byte before the caret, then commit a replacement.
    state.ime_queue_delete_surrounding(1, 0);
    state.ime_queue_commit(Some("Y".to_string()));
    assert!(state.ime_apply_done());
    assert_eq!(buffer(&state), "abYcd");
}

#[test]
fn delete_surrounding_removes_bytes_on_both_sides_of_the_caret() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, "abXcd".to_string());
    if let DrawingState::TextInput { caret, .. } = &mut state.state {
        *caret = 3;
    }

    state.ime_queue_delete_surrounding(1, 1);
    state.ime_queue_commit(Some("Y".to_string()));
    assert!(state.ime_apply_done());

    assert_eq!(buffer(&state), "abYd");
}

#[test]
fn delete_surrounding_excludes_the_selection_before_commit_replaces_it() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, "0123456789".to_string());
    if let DrawingState::TextInput {
        caret,
        selection_anchor,
        ..
    } = &mut state.state
    {
        *caret = 7;
        *selection_anchor = Some(3); // "3456"
    }

    // Delete "12" before and "78" after the selection, then replace the
    // still-active selection with the commit.
    state.ime_queue_delete_surrounding(2, 2);
    state.ime_queue_commit(Some("X".to_string()));
    assert!(state.ime_apply_done());

    assert_eq!(buffer(&state), "0X9");
}

#[test]
fn delete_surrounding_preserves_a_reverse_selection_for_commit() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, "0123456789".to_string());
    if let DrawingState::TextInput {
        caret,
        selection_anchor,
        ..
    } = &mut state.state
    {
        *caret = 3;
        *selection_anchor = Some(7); // reverse selection of "3456"
    }

    state.ime_queue_delete_surrounding(2, 2);
    state.ime_queue_commit(Some("X".to_string()));
    assert!(state.ime_apply_done());

    assert_eq!(buffer(&state), "0X9");
}

#[test]
fn surrounding_text_preserves_cursor_and_selection_direction() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, "hello".to_string());
    if let DrawingState::TextInput {
        caret,
        selection_anchor,
        ..
    } = &mut state.state
    {
        *caret = 1;
        *selection_anchor = Some(4);
    }

    assert_eq!(
        state.text_input_surrounding_state(),
        Some(("hello".to_string(), 1, 4))
    );
}

#[test]
fn surrounding_text_windows_long_buffers_on_utf8_boundaries() {
    let mut state = create_test_input_state();
    let text = format!("{}你{}", "a".repeat(5_000), "b".repeat(4_997));
    state.state = DrawingState::text_input(0, 0, text);
    if let DrawingState::TextInput { caret, .. } = &mut state.state {
        *caret = 5_003;
    }

    let (surrounding, cursor, anchor) = state
        .text_input_surrounding_state()
        .expect("a caret-centered protocol window fits");
    assert!(surrounding.len() <= 4_000);
    assert!(surrounding.is_char_boundary(cursor));
    assert_eq!(cursor, anchor);
    assert_eq!(&surrounding[cursor - "你".len()..cursor], "你");
}

#[test]
fn oversized_selection_withholds_surrounding_text_until_context_fits_again() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, "a".repeat(5_000));
    if let DrawingState::TextInput {
        caret,
        selection_anchor,
        ..
    } = &mut state.state
    {
        *caret = 4_500;
        *selection_anchor = Some(499);
    }

    assert_eq!(
        state.text_input_surrounding_state(),
        None,
        "an applied empty value may make the compositor ignore later context"
    );

    if let DrawingState::TextInput {
        selection_anchor, ..
    } = &mut state.state
    {
        *selection_anchor = None;
    }
    let (surrounding, cursor, anchor) = state
        .text_input_surrounding_state()
        .expect("collapsing the selection restores a reportable context window");
    assert!(!surrounding.is_empty());
    assert_eq!(cursor, anchor);
}

#[test]
fn only_committed_ime_edits_advance_the_text_buffer_revision() {
    let mut state = create_test_input_state();
    enter_text_mode(&mut state);
    let initial = state.text_input_revision;

    state.ime_queue_preedit(Some("draft".to_string()), 5, 5);
    assert!(state.ime_apply_done());
    assert_eq!(
        state.text_input_revision, initial,
        "transient preedit changes do not mutate committed text"
    );

    state.ime_queue_commit(Some("done".to_string()));
    assert!(state.ime_apply_done());
    assert_eq!(
        state.text_input_revision,
        initial.wrapping_add(1),
        "an IME commit invalidates deferred cuts from the prior revision"
    );
}

#[test]
fn preedit_replacing_a_forward_selection_has_one_effective_preview_and_cursor() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, "hello world".to_string());
    if let DrawingState::TextInput {
        caret,
        selection_anchor,
        ..
    } = &mut state.state
    {
        *selection_anchor = Some(0);
        *caret = 5;
    }
    state.ime_queue_preedit(Some("X".to_string()), 1, 1);
    state.ime_apply_done();

    let preview = state
        .text_input_preview("|")
        .expect("active text input has a preview");

    assert_eq!(preview.text, "X| world");
    assert_eq!(preview.caret, None, "the IME owns the visible cursor");
    assert_eq!(preview.highlight, None);
    assert_eq!(preview.underline, Some(0..2));
    assert_eq!(
        preview.ime_cursor,
        Some(1),
        "candidate geometry follows the preedit cursor at the replacement point"
    );
}

#[test]
fn preedit_start_removes_selection_and_invalidates_pending_clipboard_edit() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, "hello world".to_string());
    if let DrawingState::TextInput {
        caret,
        selection_anchor,
        ..
    } = &mut state.state
    {
        *selection_anchor = Some(0);
        *caret = 5;
    }

    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('v'));
    state.modifiers.ctrl = false;
    let stale_paste = state
        .take_pending_text_paste()
        .expect("Ctrl+V captures the selected buffer revision");
    let initial_revision = state.text_input_revision;

    state.ime_queue_preedit(Some("X".to_string()), 1, 1);
    assert!(state.ime_apply_done());

    assert_eq!(buffer(&state), " world");
    if let DrawingState::TextInput {
        caret,
        selection_anchor,
        ..
    } = &state.state
    {
        assert_eq!(*caret, 0, "the caret collapses to the selection start");
        assert_eq!(*selection_anchor, None);
    } else {
        panic!("expected TextInput");
    }
    assert_eq!(
        state.text_input_revision,
        initial_revision.wrapping_add(1),
        "removing selected committed text invalidates deferred clipboard work"
    );

    state.ime_queue_preedit(None, 0, 0);
    assert!(state.ime_apply_done());
    assert_eq!(
        buffer(&state),
        " world",
        "canceling the preedit must not restore the removed selection"
    );
    assert!(
        state.apply_text_paste(stale_paste, "stale").is_none(),
        "the paste captured before composition must remain invalid"
    );
}

#[test]
fn null_preedit_event_still_removes_the_existing_selection() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, "hello".to_string());
    if let DrawingState::TextInput {
        caret,
        selection_anchor,
        ..
    } = &mut state.state
    {
        *selection_anchor = Some(1);
        *caret = 4;
    }
    let initial_revision = state.text_input_revision;

    state.ime_queue_preedit(None, 0, 0);
    assert!(state.ime_apply_done());

    assert_eq!(buffer(&state), "ho");
    assert!(state.ime_preedit().is_none());
    assert_eq!(state.text_input_revision, initial_revision.wrapping_add(1));
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
