use super::*;

#[test]
fn undo_all_and_redo_all_process_entire_stack() {
    let mut state = create_test_input_state();
    let frame = state.boards.active_frame_mut();

    // Seed history with two creates
    let first = frame.add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let first_index = frame.find_index(first).unwrap();
    let first_snapshot = frame.shape(first).unwrap().clone();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(first_index, first_snapshot)],
        },
        state.undo_stack_limit,
    );

    let second = frame.add_shape(Shape::Rect {
        x: 20,
        y: 20,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second_index = frame.find_index(second).unwrap();
    let second_snapshot = frame.shape(second).unwrap().clone();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(second_index, second_snapshot)],
        },
        state.undo_stack_limit,
    );

    assert_eq!(state.boards.active_frame().undo_stack_len(), 2);

    state.undo_all_immediate();
    assert_eq!(state.boards.active_frame().shapes.len(), 0);
    assert_eq!(state.boards.active_frame().redo_stack_len(), 2);

    state.redo_all_immediate();
    assert_eq!(state.boards.active_frame().shapes.len(), 2);
    assert_eq!(state.boards.active_frame().undo_stack_len(), 2);
}

#[test]
fn undo_all_with_delay_respects_history() {
    let mut state = create_test_input_state();
    let frame = state.boards.active_frame_mut();

    let id = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let idx = frame.find_index(id).unwrap();
    let snap = frame.shape(id).unwrap().clone();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(idx, snap)],
        },
        state.undo_stack_limit,
    );

    state.start_undo_all_delayed(0);
    state.tick_delayed_history(std::time::Instant::now());
    assert_eq!(state.boards.active_frame().shapes.len(), 0);
    assert_eq!(state.boards.active_frame().redo_stack_len(), 1);
}

#[test]
fn redo_all_with_delay_replays_history() {
    let mut state = create_test_input_state();
    let frame = state.boards.active_frame_mut();

    let id = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let idx = frame.find_index(id).unwrap();
    let snap = frame.shape(id).unwrap().clone();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(idx, snap)],
        },
        state.undo_stack_limit,
    );

    state.undo_all_immediate();
    assert_eq!(state.boards.active_frame().redo_stack_len(), 1);

    state.start_redo_all_delayed(0);
    state.tick_delayed_history(std::time::Instant::now());
    assert_eq!(state.boards.active_frame().shapes.len(), 1);
    assert_eq!(state.boards.active_frame().undo_stack_len(), 1);
}
