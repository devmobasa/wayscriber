use super::*;

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
