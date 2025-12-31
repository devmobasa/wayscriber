use super::*;

#[test]
fn context_menu_respects_enable_flag() {
    let mut state = create_test_input_state();
    state.set_context_menu_enabled(false);
    state.toggle_context_menu_via_keyboard();
    assert!(!state.is_context_menu_open());

    state.set_context_menu_enabled(true);
    state.toggle_context_menu_via_keyboard();
    assert!(state.is_context_menu_open());
}

#[test]
fn shape_menu_includes_select_this_entry_whenever_hovered() {
    let mut state = create_test_input_state();
    let first = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 40,
        y: 40,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![first, second]);
    state.open_context_menu(
        (0, 0),
        vec![first, second],
        ContextMenuKind::Shape,
        Some(first),
    );

    let entries = state.context_menu_entries();
    assert!(
        entries
            .iter()
            .any(|entry| entry.label == "Select This Shape"),
        "Expected Select This Shape entry to be present for multi-selection"
    );

    state.set_selection(vec![first]);
    state.open_context_menu((0, 0), vec![first], ContextMenuKind::Shape, Some(first));

    let entries_single = state.context_menu_entries();
    assert!(
        entries_single
            .iter()
            .any(|entry| entry.label == "Select This Shape"),
        "Expected Select This Shape entry even for single selection"
    );
}

#[test]
fn undo_all_and_redo_all_process_entire_stack() {
    let mut state = create_test_input_state();
    let frame = state.canvas_set.active_frame_mut();

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

    assert_eq!(state.canvas_set.active_frame().undo_stack_len(), 2);

    state.undo_all_immediate();
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 0);
    assert_eq!(state.canvas_set.active_frame().redo_stack_len(), 2);

    state.redo_all_immediate();
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 2);
    assert_eq!(state.canvas_set.active_frame().undo_stack_len(), 2);
}

#[test]
fn undo_all_with_delay_respects_history() {
    let mut state = create_test_input_state();
    let frame = state.canvas_set.active_frame_mut();

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
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 0);
    assert_eq!(state.canvas_set.active_frame().redo_stack_len(), 1);
}

#[test]
fn redo_all_with_delay_replays_history() {
    let mut state = create_test_input_state();
    let frame = state.canvas_set.active_frame_mut();

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
    assert_eq!(state.canvas_set.active_frame().redo_stack_len(), 1);

    state.start_redo_all_delayed(0);
    state.tick_delayed_history(std::time::Instant::now());
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 1);
    assert_eq!(state.canvas_set.active_frame().undo_stack_len(), 1);
}

#[test]
fn select_this_shape_command_focuses_single_shape() {
    let mut state = create_test_input_state();
    let first = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 40,
        y: 40,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![first, second]);
    state.open_context_menu(
        (10, 10),
        vec![first, second],
        ContextMenuKind::Shape,
        Some(second),
    );

    state.execute_menu_command(MenuCommand::SelectHoveredShape);
    assert_eq!(state.selected_shape_ids(), &[second]);

    assert!(
        matches!(state.context_menu_state, ContextMenuState::Hidden),
        "Context menu should close after selecting hovered shape"
    );
}

#[test]
fn properties_command_opens_panel() {
    let mut state = create_test_input_state();
    let shape_id = {
        let frame = state.canvas_set.active_frame_mut();
        frame.add_shape(Shape::Rect {
            x: 10,
            y: 10,
            w: 40,
            h: 30,
            fill: false,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        })
    };

    state.set_selection(vec![shape_id]);
    state.execute_menu_command(MenuCommand::Properties);
    assert!(state.properties_panel().is_some());
    assert!(!state.is_context_menu_open());
}

#[test]
fn shape_menu_disables_edit_for_locked_text() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Text {
        x: 15,
        y: 25,
        text: "Locked".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });

    if let Some(index) = state.canvas_set.active_frame().find_index(shape_id) {
        state.canvas_set.active_frame_mut().shapes[index].locked = true;
    }

    state.set_selection(vec![shape_id]);
    state.open_context_menu(
        (0, 0),
        vec![shape_id],
        ContextMenuKind::Shape,
        Some(shape_id),
    );

    let entries = state.context_menu_entries();
    let edit_entry = entries
        .iter()
        .find(|entry| entry.command == Some(MenuCommand::EditText))
        .expect("expected edit entry");
    assert!(edit_entry.disabled);
}

#[test]
fn shape_menu_disables_delete_when_all_locked() {
    let mut state = create_test_input_state();
    let first = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 5,
        y: 5,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 25,
        y: 25,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    if let Some(index) = state.canvas_set.active_frame().find_index(first) {
        state.canvas_set.active_frame_mut().shapes[index].locked = true;
    }
    if let Some(index) = state.canvas_set.active_frame().find_index(second) {
        state.canvas_set.active_frame_mut().shapes[index].locked = true;
    }

    state.set_selection(vec![first, second]);
    state.open_context_menu(
        (0, 0),
        vec![first, second],
        ContextMenuKind::Shape,
        Some(first),
    );

    let entries = state.context_menu_entries();
    let delete_entry = entries
        .iter()
        .find(|entry| entry.command == Some(MenuCommand::Delete))
        .expect("expected delete entry");
    assert!(delete_entry.disabled);
}

#[test]
fn shape_menu_allows_delete_when_mixed_lock_state() {
    let mut state = create_test_input_state();
    let locked_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let unlocked_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 30,
        y: 30,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    if let Some(index) = state.canvas_set.active_frame().find_index(locked_id) {
        state.canvas_set.active_frame_mut().shapes[index].locked = true;
    }

    state.set_selection(vec![locked_id, unlocked_id]);
    state.open_context_menu(
        (0, 0),
        vec![locked_id, unlocked_id],
        ContextMenuKind::Shape,
        Some(locked_id),
    );

    let entries = state.context_menu_entries();
    let delete_entry = entries
        .iter()
        .find(|entry| entry.command == Some(MenuCommand::Delete))
        .expect("expected delete entry");
    assert!(!delete_entry.disabled);
}

#[test]
fn keyboard_context_menu_sets_initial_focus() {
    let mut state = create_test_input_state();
    state.toggle_context_menu_via_keyboard();
    match &state.context_menu_state {
        ContextMenuState::Open { keyboard_focus, .. } => {
            assert!(keyboard_focus.is_some());
        }
        ContextMenuState::Hidden => panic!("Context menu should be open"),
    }
}

#[test]
fn keyboard_context_menu_focuses_edit_for_selected_text() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 60,
        text: "Hello".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });

    state.set_selection(vec![shape_id]);
    state.toggle_context_menu_via_keyboard();

    let focus_index = match &state.context_menu_state {
        ContextMenuState::Open {
            keyboard_focus: Some(index),
            ..
        } => index,
        _ => panic!("Context menu should be open with focus"),
    };
    let entries = state.context_menu_entries();
    assert_eq!(entries[*focus_index].command, Some(MenuCommand::EditText));
}
