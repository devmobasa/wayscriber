//! Integration tests for keyboard-driven caret editing, selection, and the
//! text clipboard, exercised through `on_key_press` and the pending-request
//! plumbing the backend drains. The byte-level movement rules themselves are
//! unit-tested in `actions::key_press::caret_edit`; these assert the wiring:
//! key rerouting, selection, and clipboard intent capture.

use super::super::*;

fn text_state(buffer: &str) -> InputState {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(0, 0, buffer.to_string());
    state
}

fn buffer(state: &InputState) -> String {
    match &state.state {
        DrawingState::TextInput { buffer, .. } => buffer.clone(),
        other => panic!("expected TextInput, got {other:?}"),
    }
}

fn caret(state: &InputState) -> usize {
    match &state.state {
        DrawingState::TextInput { caret, .. } => *caret,
        other => panic!("expected TextInput, got {other:?}"),
    }
}

fn origin(state: &InputState) -> (i32, i32) {
    match &state.state {
        DrawingState::TextInput { x, y, .. } => (*x, *y),
        other => panic!("expected TextInput, got {other:?}"),
    }
}

#[test]
fn typing_inserts_at_the_caret_not_only_the_end() {
    let mut state = text_state("ad");
    state.on_key_press(Key::Left); // caret 2 -> 1
    state.on_key_press(Key::Char('b'));
    state.on_key_press(Key::Char('c'));
    assert_eq!(buffer(&state), "abcd");
    assert_eq!(caret(&state), 3);
}

#[test]
fn arrows_home_end_move_the_caret_without_editing() {
    let mut state = text_state("abc");
    state.on_key_press(Key::Home);
    assert_eq!(caret(&state), 0);
    state.on_key_press(Key::Right);
    assert_eq!(caret(&state), 1);
    state.on_key_press(Key::End);
    assert_eq!(caret(&state), 3);
    assert_eq!(buffer(&state), "abc");
}

#[test]
fn horizontal_arrows_follow_visual_order_in_rtl_text() {
    let mut state = text_state("אבג");
    let logical_end = buffer(&state).len();
    assert_eq!(caret(&state), logical_end);

    state.on_key_press(Key::Right);
    assert!(
        caret(&state) < logical_end,
        "Right from the visual left edge of RTL text moves into the line"
    );

    state.on_key_press(Key::Left);
    assert_eq!(
        caret(&state),
        logical_end,
        "Left reverses the visual movement"
    );
}

#[test]
fn ctrl_arrows_and_selection_collapse_follow_visual_order_in_rtl_text() {
    let mut state = text_state("אבג");
    let logical_end = buffer(&state).len();

    state.modifiers.ctrl = true;
    state.on_key_press(Key::Left);
    assert_eq!(
        caret(&state),
        logical_end,
        "Ctrl+Left stays at the visual left edge"
    );
    state.on_key_press(Key::Right);
    assert!(
        caret(&state) < logical_end,
        "Ctrl+Right moves inward through an RTL word"
    );

    state.modifiers.ctrl = false;
    state.modifiers.shift = true;
    state.on_key_press(Key::Left);
    state.modifiers.shift = false;
    state.on_key_press(Key::Left);
    assert!(
        caret(&state) > 0,
        "Left collapses the RTL selection to its visual-left endpoint"
    );
}

#[test]
fn configured_ctrl_shift_navigation_falls_through_to_the_action_layer() {
    let mut state = text_state("hello");
    state.modifiers.ctrl = true;
    state.modifiers.shift = true;
    assert_eq!(
        state.find_action("ArrowLeft"),
        Some(Action::BoardPrev),
        "the default Ctrl+Shift+Left action is configured"
    );

    state.on_key_press(Key::Left);

    assert!(
        matches!(state.state, DrawingState::Idle),
        "the board shortcut must reach its action instead of becoming word-selection"
    );
}

#[test]
fn collapsing_a_selection_at_its_caret_boundary_marks_an_external_change() {
    let mut state = text_state("abcd");
    state.modifiers.shift = true;
    state.on_key_press(Key::Home);
    state.modifiers.shift = false;
    assert!(state.take_text_input_external_change_dirty());

    state.modifiers.ctrl = true;
    state.on_key_press(Key::Home);
    state.modifiers.ctrl = false;

    let DrawingState::TextInput {
        caret,
        selection_anchor,
        ..
    } = &state.state
    else {
        unreachable!();
    };
    assert_eq!((*caret, *selection_anchor), (0, None));
    assert!(
        state.take_text_input_external_change_dirty(),
        "selection collapse must redraw and update text-input-v3"
    );
}

#[test]
fn keyboard_pointer_and_paste_changes_mark_an_external_ime_update() {
    use crate::input::MouseButton;

    let mut state = text_state("abc");

    state.on_key_press(Key::Home);
    assert!(state.take_text_input_external_change_dirty());

    state.on_mouse_press_with_canvas(MouseButton::Left, 10, 0, 10, 0);
    assert!(state.take_text_input_external_change_dirty());

    assert!(state.insert_text_at_caret("X"));
    assert!(state.take_text_input_external_change_dirty());
}

#[test]
fn backspace_deletes_before_the_caret() {
    let mut state = text_state("abc");
    state.on_key_press(Key::Left); // caret 3 -> 2
    state.on_key_press(Key::Backspace); // remove 'b'
    assert_eq!(buffer(&state), "ac");
    assert_eq!(caret(&state), 1);
}

#[test]
fn delete_removes_the_character_after_the_caret() {
    let mut state = text_state("abc");
    state.on_key_press(Key::Home);
    state.on_key_press(Key::Delete);
    assert_eq!(buffer(&state), "bc");
    assert_eq!(caret(&state), 0);
}

#[test]
fn select_all_then_type_replaces_the_whole_buffer() {
    let mut state = text_state("replace me");
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('a'));
    state.modifiers.ctrl = false;
    state.on_key_press(Key::Char('X'));
    assert_eq!(buffer(&state), "X");
}

#[test]
fn shift_arrow_selects_and_backspace_deletes_the_selection() {
    let mut state = text_state("abcd");
    state.modifiers.shift = true;
    state.on_key_press(Key::Left);
    state.on_key_press(Key::Left); // select "cd"
    state.modifiers.shift = false;
    state.on_key_press(Key::Backspace);
    assert_eq!(buffer(&state), "ab");
    assert_eq!(caret(&state), 2);
}

#[test]
fn ctrl_c_captures_the_selection_without_editing() {
    let mut state = text_state("hello");
    state.modifiers.shift = true;
    state.on_key_press(Key::Home); // select the whole line
    state.modifiers.shift = false;
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('c'));
    state.modifiers.ctrl = false;
    let request = state
        .take_pending_text_copy()
        .expect("copy request is queued");
    assert_eq!(request.text, "hello");
    assert!(request.cut.is_none());
    assert_eq!(buffer(&state), "hello", "copy does not modify the buffer");
}

#[test]
fn ctrl_x_deletes_the_selection_only_after_clipboard_publication() {
    let mut state = text_state("hello");
    state.modifiers.shift = true;
    state.on_key_press(Key::Home);
    state.modifiers.shift = false;
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('x'));
    state.modifiers.ctrl = false;
    let request = state
        .take_pending_text_copy()
        .expect("cut request is queued");
    assert_eq!(request.text, "hello");
    assert!(request.cut.is_some());
    assert_eq!(
        buffer(&state),
        "hello",
        "a failed clipboard publication must leave the selection intact"
    );

    state.complete_text_copy(request);
    assert_eq!(
        buffer(&state),
        "",
        "successful publication completes the cut"
    );
}

#[test]
fn repeated_ctrl_x_requests_are_retained_before_backend_draining() {
    let mut state = text_state("hello");
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('a'));
    state.on_key_press(Key::Char('x'));
    state.on_key_press(Key::Char('x'));
    state.modifiers.ctrl = false;

    assert!(state.take_pending_text_copy().is_some());
    assert!(
        state.take_pending_text_copy().is_some(),
        "each cut key event must reach the asynchronous publication queue"
    );
}

#[test]
fn stale_cut_completion_never_deletes_later_edits() {
    let mut state = text_state("hello");
    state.modifiers.shift = true;
    state.on_key_press(Key::Home);
    state.modifiers.shift = false;
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('x'));
    state.modifiers.ctrl = false;
    let request = state
        .take_pending_text_copy()
        .expect("cut request is queued");

    state.on_key_press(Key::Char('X'));
    assert_eq!(buffer(&state), "X");
    state.complete_text_copy(request);
    assert_eq!(buffer(&state), "X");
}

#[test]
fn cut_completion_is_invalid_after_intervening_edits_restore_the_same_selection() {
    let mut state = text_state("hello");
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('a'));
    state.on_key_press(Key::Char('x'));
    state.modifiers.ctrl = false;
    let request = state
        .take_pending_text_copy()
        .expect("cut request is queued");

    // Replace the selected bytes, then rebuild and reselect byte-identical
    // text before the asynchronous clipboard publication completes.
    for ch in "world".chars() {
        state.on_key_press(Key::Char(ch));
    }
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('a'));
    state.modifiers.ctrl = false;
    for ch in "hello".chars() {
        state.on_key_press(Key::Char(ch));
    }
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('a'));
    state.modifiers.ctrl = false;

    assert_eq!(buffer(&state), "hello");
    state.complete_text_copy(request);
    assert_eq!(
        buffer(&state),
        "hello",
        "a cut may delete only the exact buffer revision it captured"
    );
}

#[test]
fn ctrl_v_requests_a_paste_that_inserts_at_the_caret() {
    let mut state = text_state("ac");
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('v'));
    state.modifiers.ctrl = false;
    assert!(
        state.take_pending_text_paste().is_some(),
        "Ctrl+V requests a paste"
    );
    assert!(
        state.take_pending_text_paste().is_none(),
        "the request is one-shot and clears when taken"
    );

    // The backend delivers clipboard text via insert_text_at_caret.
    state.on_key_press(Key::Left); // caret between 'a' and 'c'
    assert!(state.insert_text_at_caret("b"));
    assert_eq!(buffer(&state), "abc");
}

#[test]
fn repeated_ctrl_v_requests_are_retained_before_backend_draining() {
    let mut state = text_state("hello");
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('v'));
    state.on_key_press(Key::Char('v'));
    state.modifiers.ctrl = false;

    assert!(state.take_pending_text_paste().is_some());
    assert!(
        state.take_pending_text_paste().is_some(),
        "each paste key event must reach the asynchronous read queue"
    );
}

#[test]
fn delayed_paste_replaces_the_selection_captured_at_invocation() {
    let mut state = text_state("hello");
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('a'));
    state.on_key_press(Key::Char('v'));
    state.modifiers.ctrl = false;
    let target = state
        .take_pending_text_paste()
        .expect("Ctrl+V captures a paste request");

    // Clipboard reads are asynchronous. Moving the caret while the read is in
    // flight must not retarget the completion away from the invoked selection.
    state.on_key_press(Key::Home);
    assert!(state.apply_text_paste(target, "X").is_some());
    assert_eq!(buffer(&state), "X");
}

#[test]
fn paste_generation_does_not_match_a_later_text_edit() {
    let mut state = create_test_input_state();
    state.handle_action(Action::EnterTextMode);
    let first = state.text_input_generation().expect("text edit is active");

    state.cancel_text_input();
    state.handle_action(Action::EnterTextMode);

    assert!(!state.text_input_generation_is_current(first));
}

#[test]
fn first_click_places_a_new_empty_text_block() {
    use crate::input::MouseButton;

    let mut state = create_test_input_state();
    state.handle_action(Action::EnterTextMode);
    state.on_mouse_press_with_canvas(MouseButton::Left, 222, 333, 222, 333);

    assert_eq!(origin(&state), (222, 333));
}

#[test]
fn up_and_down_follow_wrapped_visual_lines() {
    let mut state = text_state("abcdefghij");
    state.text_wrap_width = Some(35);
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Home);
    state.modifiers.ctrl = false;

    state.on_key_press(Key::Down);
    let next_line = caret(&state);
    assert!(next_line > 0, "Down advances to the next visible line");
    assert!(
        next_line < buffer(&state).len(),
        "Down must not skip all wrapped lines to the buffer end"
    );

    state.on_key_press(Key::Up);
    assert_eq!(caret(&state), 0, "Up returns to the prior visible line");
}

#[test]
fn home_and_end_stay_on_the_current_wrapped_visual_line() {
    let mut state = text_state("abcdefghij");
    state.current_font_size = 20.0;
    state.text_wrap_width = Some(50);
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Home);
    state.modifiers.ctrl = false;
    state.on_key_press(Key::Down);

    state.on_key_press(Key::Right);
    state.on_key_press(Key::Home);
    let wrapped_line_start = caret(&state);
    assert!(wrapped_line_start > 0);

    state.on_key_press(Key::Right);
    state.on_key_press(Key::End);
    assert!(
        caret(&state) < buffer(&state).len(),
        "End stops at the current soft-wrapped line"
    );
    assert!(
        caret(&state) > wrapped_line_start,
        "End moves to the other edge of the current soft-wrapped line"
    );
}

#[test]
fn alt_left_drag_moves_the_whole_block_without_editing() {
    use crate::input::MouseButton;

    let mut state = text_state("hello");
    // Anchor the block away from the origin so the move is unambiguous.
    if let DrawingState::TextInput { x, y, .. } = &mut state.state {
        *x = 100;
        *y = 100;
    }

    // Grab 10px to the right of the origin, then drag +20/+15.
    state.modifiers.alt = true;
    state.on_mouse_press_with_canvas(MouseButton::Left, 110, 100, 110, 100);
    state.on_mouse_motion_with_canvas(130, 115, 130, 115);
    state.on_mouse_release_with_canvas(MouseButton::Left, 130, 115, 130, 115);
    state.modifiers.alt = false;

    // Origin followed the cursor by the drag delta, preserving the grab offset.
    assert_eq!(origin(&state), (120, 115));
    assert_eq!(buffer(&state), "hello", "moving the block never edits text");
    assert!(
        state.text_block_drag.is_none(),
        "the drag flag is cleared on release"
    );
    assert!(
        state.active_drag_button.is_none(),
        "the pointer drag is ended on release"
    );
}

#[test]
fn return_while_alt_dragging_releases_pointer_ownership() {
    use crate::input::MouseButton;

    let mut state = text_state("hello");
    state.modifiers.alt = true;
    state.on_mouse_press_with_canvas(MouseButton::Left, 2, 0, 2, 0);
    assert!(state.text_block_drag_active());
    assert!(state.has_active_pointer_interaction());

    state.modifiers.alt = false;
    state.on_key_press(Key::Return);

    assert!(matches!(state.state, DrawingState::Idle));
    assert!(!state.text_block_drag_active());
    assert!(!state.has_active_pointer_interaction());
}

#[test]
fn plain_left_drag_does_not_move_the_block() {
    use crate::input::MouseButton;

    let mut state = text_state("hello");
    if let DrawingState::TextInput { x, y, .. } = &mut state.state {
        *x = 100;
        *y = 100;
    }

    // Without Alt, a press positions the caret and a drag does not relocate.
    state.on_mouse_press_with_canvas(MouseButton::Left, 110, 100, 110, 100);
    state.on_mouse_motion_with_canvas(130, 115, 130, 115);
    state.on_mouse_release_with_canvas(MouseButton::Left, 130, 115, 130, 115);

    assert_eq!(
        origin(&state),
        (100, 100),
        "a plain drag leaves the block where it is"
    );
    assert!(state.text_block_drag.is_none());
}

#[test]
fn unbound_left_click_still_positions_the_caret_without_moving_the_block() {
    use crate::input::{DragBinding, DragToolBindings, MouseButton};

    let mut state = text_state("hello");
    if let DrawingState::TextInput { x, y, .. } = &mut state.state {
        *x = 100;
        *y = 100;
    }
    let mut bindings = DragToolBindings::default();
    bindings.left.drag = DragBinding::button_default();
    assert!(state.set_drag_tool_bindings(bindings));

    state.on_mouse_press_with_canvas(MouseButton::Left, 110, 100, 110, 100);

    assert_eq!(
        origin(&state),
        (100, 100),
        "editor clicks keep their semantics even without a drawing-tool binding"
    );
    assert!(caret(&state) < "hello".len());
}

#[test]
fn click_after_visible_preedit_maps_back_to_the_committed_buffer() {
    use crate::input::MouseButton;

    let mut state = text_state("abWXYZcd");
    if let DrawingState::TextInput {
        caret,
        selection_anchor,
        ..
    } = &mut state.state
    {
        *caret = 6;
        *selection_anchor = Some(2);
    }
    state.ime_queue_preedit(Some("MMMMMMMM".to_string()), -1, -1);
    assert!(state.ime_apply_done());

    let preview = "abMMMMMMMMcd";
    let font = state
        .font_descriptor
        .to_pango_string(state.current_font_size);
    let geometry = crate::draw::shape::caret_geometry_text(preview, &font, None, 11)
        .expect("preview caret geometry is measurable");
    let click_x = geometry.x.round() as i32;
    state.on_mouse_press_with_canvas(MouseButton::Left, click_x, 0, click_x, 0);

    assert_eq!(
        caret(&state),
        3,
        "the preview position between c/d maps into the buffer after IME removed the selection"
    );
}

#[test]
fn edit_ghost_is_hidden_in_place_and_shown_after_moving() {
    use crate::draw::Shape;

    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 100,
        y: 100,
        text: "some text".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(
        state.edit_selected_text(),
        "should enter edit mode on the existing text"
    );

    // Editing in place: the ghost stays hidden, so deletes look clean.
    assert!(
        !state.text_edit_ghost_visible(),
        "no ghost while the block sits on its original spot"
    );

    // Repositioning reveals the ghost as a 'where it was' reference.
    if let DrawingState::TextInput { x, .. } = &mut state.state {
        *x += 25;
    }
    assert!(
        state.text_edit_ghost_visible(),
        "the ghost appears once the block has moved"
    );
}

#[test]
fn alt_modified_editing_keys_fall_through_instead_of_editing() {
    // Alt-modified combinations are action shortcuts (e.g. Ctrl+Alt+Delete page
    // delete, Ctrl+Alt+ArrowUp marker opacity), never editing ops. The editor
    // must not consume them, so the caret and buffer are left untouched.
    let mut state = text_state("hello"); // caret at end (5)
    state.modifiers.alt = true;
    state.on_key_press(Key::Left);
    assert_eq!(caret(&state), 5, "Alt+Left is not caret navigation");
    state.on_key_press(Key::Backspace);
    assert_eq!(
        buffer(&state),
        "hello",
        "Alt+Backspace does not edit the buffer"
    );
    state.on_key_press(Key::Delete);
    assert_eq!(
        buffer(&state),
        "hello",
        "Alt+Delete does not edit the buffer"
    );
    state.modifiers.alt = false;
}

#[test]
fn unbound_altgr_character_is_inserted_after_shortcut_lookup() {
    let mut state = text_state("");
    state.modifiers.ctrl = true;
    state.modifiers.alt = true;

    state.on_key_press(Key::Char('@'));

    assert_eq!(buffer(&state), "@");
}

#[test]
fn ctrl_z_still_reaches_the_action_layer_in_text_mode() {
    // Editing keys are consumed by the editor, but non-editing Ctrl shortcuts
    // must still fall through: type text, then Ctrl+Z should not insert 'z'.
    let mut state = text_state("");
    state.on_key_press(Key::Char('a'));
    state.modifiers.ctrl = true;
    state.on_key_press(Key::Char('Z'));
    state.modifiers.ctrl = false;
    assert!(
        !buffer(&state).contains('z') && !buffer(&state).contains('Z'),
        "Ctrl+Z is an action, not text input"
    );
}
