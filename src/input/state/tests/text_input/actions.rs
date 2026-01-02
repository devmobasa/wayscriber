use super::super::*;

#[test]
fn test_redo_restores_shape_after_undo() {
    let mut state = create_test_input_state();

    {
        let frame = state.boards.active_frame_mut();
        let shape_id = frame.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: state.current_color,
            thick: state.current_thickness,
        });

        let index = frame.find_index(shape_id).unwrap();
        let snapshot = frame.shape(shape_id).unwrap().clone();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, snapshot)],
            },
            state.undo_stack_limit,
        );
    }

    assert_eq!(state.boards.active_frame().shapes.len(), 1);

    state.handle_action(Action::Undo);
    assert_eq!(state.boards.active_frame().shapes.len(), 0);

    state.handle_action(Action::Redo);
    assert_eq!(state.boards.active_frame().shapes.len(), 1);
}

#[test]
fn capture_action_sets_pending_and_clears_modifiers() {
    let mut state = create_test_input_state();
    state.modifiers.ctrl = true;
    state.modifiers.shift = true;
    state.modifiers.alt = true;

    state.handle_action(Action::CaptureClipboardFull);

    assert!(!state.modifiers.ctrl);
    assert!(!state.modifiers.shift);
    assert!(!state.modifiers.alt);

    assert_eq!(
        state.take_pending_capture_action(),
        Some(Action::CaptureClipboardFull)
    );
    assert!(state.take_pending_capture_action().is_none());
}
