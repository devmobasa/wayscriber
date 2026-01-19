use super::*;

#[test]
fn mouse_drag_creates_shapes_for_each_tool() {
    let mut state = create_test_input_state();

    // Pen
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_motion(10, 10);
    state.on_mouse_release(MouseButton::Left, 10, 10);
    assert_eq!(state.boards.active_frame().shapes.len(), 1);
    state.clear_selection();

    // Line (Shift)
    state.modifiers.shift = true;
    state.on_mouse_press(MouseButton::Left, 20, 20);
    state.on_mouse_release(MouseButton::Left, 25, 25);
    assert_eq!(state.boards.active_frame().shapes.len(), 2);
    state.clear_selection();

    // Rectangle (Ctrl)
    state.modifiers.shift = false;
    state.modifiers.ctrl = true;
    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_release(MouseButton::Left, 45, 45);
    assert_eq!(state.boards.active_frame().shapes.len(), 3);
    state.clear_selection();

    // Ellipse (Tab)
    state.modifiers.ctrl = false;
    state.modifiers.tab = true;
    state.on_mouse_press(MouseButton::Left, 60, 60);
    state.on_mouse_release(MouseButton::Left, 64, 64);
    assert_eq!(state.boards.active_frame().shapes.len(), 4);
    state.clear_selection();

    // Arrow (Ctrl+Shift)
    state.modifiers.tab = false;
    state.modifiers.ctrl = true;
    state.modifiers.shift = true;
    state.on_mouse_press(MouseButton::Left, 80, 80);
    state.on_mouse_release(MouseButton::Left, 86, 86);
    assert_eq!(state.boards.active_frame().shapes.len(), 5);
}

#[test]
fn toggle_click_highlight_action_changes_state() {
    let mut state = create_test_input_state();
    assert!(!state.click_highlight_enabled());

    state.handle_action(Action::ToggleClickHighlight);
    assert!(state.click_highlight_enabled());
    assert!(state.needs_redraw);

    state.needs_redraw = false;
    state.handle_action(Action::ToggleClickHighlight);
    assert!(!state.click_highlight_enabled());
    assert!(state.needs_redraw);
}

#[test]
fn highlight_tool_prevents_drawing() {
    let mut state = create_test_input_state();
    assert_eq!(state.active_tool(), Tool::Pen);
    assert!(!state.highlight_tool_active());

    state.handle_action(Action::ToggleHighlightTool);
    assert!(state.highlight_tool_active());
    assert_eq!(state.active_tool(), Tool::Highlight);

    // Enable highlight effect to ensure no shapes are added while clicks happen
    state.handle_action(Action::ToggleClickHighlight);

    let initial_shapes = state.boards.active_frame().shapes.len();
    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_release(MouseButton::Left, 20, 20);
    assert_eq!(state.boards.active_frame().shapes.len(), initial_shapes);
    assert!(matches!(state.state, DrawingState::Idle));

    // Toggle highlight tool off and ensure pen drawing resumes
    state.handle_action(Action::ToggleHighlightTool);
    assert!(!state.highlight_tool_active());
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 5, 5);
    assert_eq!(state.boards.active_frame().shapes.len(), initial_shapes + 1);
}

#[test]
fn sync_highlight_color_marks_dirty_when_pen_color_changes() {
    let mut state = create_test_input_state();
    state.needs_redraw = false;
    state.current_color = Color {
        r: 0.25,
        g: 0.5,
        b: 0.75,
        a: 1.0,
    };
    state.sync_highlight_color();
    assert!(state.needs_redraw);
}
