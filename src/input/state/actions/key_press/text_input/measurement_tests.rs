use super::*;

fn text_state(buffer: &str) -> InputState {
    let mut state = crate::input::state::test_support::make_test_input_state();
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

#[test]
fn explicit_horizontal_arrows_follow_visual_order_in_rtl_text() {
    let measurer = TextMeasurer::default();
    let mut state = text_state("אבג");
    let logical_end = buffer(&state).len();
    assert_eq!(caret(&state), logical_end);

    assert!(state.handle_text_editing_key_with(&measurer, Key::Right));
    assert!(
        caret(&state) < logical_end,
        "Right from the visual left edge of RTL text moves into the line"
    );

    assert!(state.handle_text_editing_key_with(&measurer, Key::Left));
    assert_eq!(
        caret(&state),
        logical_end,
        "Left reverses the visual movement"
    );
}

#[test]
fn explicit_ctrl_arrows_and_selection_collapse_follow_visual_order_in_rtl_text() {
    let measurer = TextMeasurer::default();
    let mut state = text_state("אבג");
    let logical_end = buffer(&state).len();

    state.modifiers.ctrl = true;
    assert!(state.handle_text_editing_key_with(&measurer, Key::Left));
    assert_eq!(
        caret(&state),
        logical_end,
        "Ctrl+Left stays at the visual left edge"
    );
    assert!(state.handle_text_editing_key_with(&measurer, Key::Right));
    assert!(
        caret(&state) < logical_end,
        "Ctrl+Right moves inward through an RTL word"
    );

    state.modifiers.ctrl = false;
    state.modifiers.shift = true;
    assert!(state.handle_text_editing_key_with(&measurer, Key::Left));
    state.modifiers.shift = false;
    assert!(state.handle_text_editing_key_with(&measurer, Key::Left));
    assert!(
        caret(&state) > 0,
        "Left collapses the RTL selection to its visual-left endpoint"
    );
}

#[test]
fn explicit_up_and_down_follow_wrapped_visual_lines() {
    let measurer = TextMeasurer::default();
    let mut state = text_state("abcdefghij");
    state.style.text_wrap_width = Some(35);
    state.modifiers.ctrl = true;
    assert!(state.handle_text_editing_key_with(&measurer, Key::Home));
    state.modifiers.ctrl = false;

    assert!(state.handle_text_editing_key_with(&measurer, Key::Down));
    let next_line = caret(&state);
    assert!(next_line > 0, "Down advances to the next visible line");
    assert!(
        next_line < buffer(&state).len(),
        "Down must not skip all wrapped lines to the buffer end"
    );

    assert!(state.handle_text_editing_key_with(&measurer, Key::Up));
    assert_eq!(caret(&state), 0, "Up returns to the prior visible line");
}

#[test]
fn explicit_home_and_end_stay_on_the_current_wrapped_visual_line() {
    let measurer = TextMeasurer::default();
    let mut state = text_state("abcdefghij");
    state.style.current_font_size = 20.0;
    state.style.text_wrap_width = Some(50);
    state.modifiers.ctrl = true;
    assert!(state.handle_text_editing_key_with(&measurer, Key::Home));
    state.modifiers.ctrl = false;
    assert!(state.handle_text_editing_key_with(&measurer, Key::Down));

    assert!(state.handle_text_editing_key_with(&measurer, Key::Right));
    assert!(state.handle_text_editing_key_with(&measurer, Key::Home));
    let wrapped_line_start = caret(&state);
    assert!(wrapped_line_start > 0);

    assert!(state.handle_text_editing_key_with(&measurer, Key::Right));
    assert!(state.handle_text_editing_key_with(&measurer, Key::End));
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
fn explicit_finalize_commits_utf8_edit_and_empty_edit_restores_original() {
    let measurer = TextMeasurer::default();
    let mut state = text_state("");
    state.view.set_screen_dimensions(800, 600);
    let shape = Shape::Text {
        x: 100,
        y: 150,
        text: "original".into(),
        color: state.style.current_color,
        size: state.style.current_font_size,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: false,
        wrap_width: Some(140),
    };
    let id = state.boards.active_frame_mut().add_shape(shape);
    state.set_selection(vec![id]);
    assert!(state.edit_selected_text_with(&measurer));
    state.modifiers.ctrl = true;
    assert!(state.handle_text_editing_key_with(&measurer, Key::Char('a')));
    state.modifiers.ctrl = false;
    assert!(state.insert_text_at_caret_with(&measurer, "שלום café"));
    state.finalize_text_input_with(&measurer);
    assert!(matches!(state.state, DrawingState::Idle));
    let committed = state.boards.active_frame().shape(id).unwrap().shape.clone();
    assert!(matches!(&committed, Shape::Text { text, .. } if text == "שלום café"));
    let action = state
        .boards
        .active_frame_mut()
        .undo_last()
        .expect("edit undo");
    state.apply_action_side_effects_with(&measurer, &action);
    assert!(
        matches!(&state.boards.active_frame().shape(id).unwrap().shape, Shape::Text { text, .. } if text == "original")
    );
    state.set_selection(vec![id]);
    assert!(state.edit_selected_text_with(&measurer));
    state.modifiers.ctrl = true;
    assert!(state.handle_text_editing_key_with(&measurer, Key::Char('a')));
    state.modifiers.ctrl = false;
    assert!(state.handle_text_editing_key_with(&measurer, Key::Backspace));
    state.finalize_text_input_with(&measurer);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(
        matches!(&state.boards.active_frame().shape(id).unwrap().shape, Shape::Text { text, .. } if text == "original")
    );
}
