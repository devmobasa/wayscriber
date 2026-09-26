use super::*;

fn add_rect(state: &mut InputState) -> crate::draw::ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
        fill: false,
        color: state.style.current_color,
        thick: state.style.current_thickness,
    })
}

fn paste_entry(state: &InputState) -> ContextMenuEntry {
    state
        .context_menu_entries()
        .into_iter()
        .find(|entry| entry.command == Some(MenuCommand::Paste))
        .expect("paste entry")
}

#[test]
fn canvas_menu_paste_waits_for_something_to_paste() {
    let mut state = create_test_input_state();
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Canvas, None);

    let paste = paste_entry(&state);
    assert_eq!(paste.label, "Paste");
    assert_eq!(paste.shortcut.as_deref(), Some("Ctrl+Alt+V"));
    assert!(paste.disabled, "nothing copied yet");

    let shape_id = add_rect(&mut state);
    state.set_selection(vec![shape_id]);
    assert_eq!(state.copy_selection(), 1);
    assert!(!paste_entry(&state).disabled, "copied shapes enable it");
}

#[test]
fn a_capture_copied_to_the_clipboard_enables_paste() {
    let mut state = create_test_input_state();
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Canvas, None);
    assert!(paste_entry(&state).disabled);

    state.note_capture_image_on_clipboard();

    assert!(!paste_entry(&state).disabled);
}

#[test]
fn shape_menu_includes_copy_and_paste_entries_for_selection() {
    let mut state = create_test_input_state();
    let shape_id = add_rect(&mut state);
    state.set_selection(vec![shape_id]);
    state.open_context_menu(
        (12, 34),
        vec![shape_id],
        ContextMenuKind::Shape,
        Some(shape_id),
    );

    let entries = state.context_menu_entries();
    let copy_entry = entries
        .iter()
        .find(|entry| entry.command == Some(MenuCommand::Copy))
        .expect("copy entry");
    let paste_entry = entries
        .iter()
        .find(|entry| entry.command == Some(MenuCommand::Paste))
        .expect("paste entry");

    assert_eq!(copy_entry.label, "Copy");
    assert_eq!(copy_entry.shortcut.as_deref(), Some("Ctrl+Alt+C"));
    assert!(!copy_entry.disabled);
    assert_eq!(paste_entry.label, "Paste");
    assert_eq!(paste_entry.shortcut.as_deref(), Some("Ctrl+Alt+V"));
    assert!(paste_entry.disabled, "nothing has been copied yet");
}

#[test]
fn shape_menu_disables_copy_when_all_selected_shapes_are_locked() {
    let mut state = create_test_input_state();
    let shape_id = add_rect(&mut state);
    let shape_index = state
        .boards
        .active_frame()
        .find_index(shape_id)
        .expect("shape index");
    state.boards.active_frame_mut().shapes[shape_index].locked = true;

    state.set_selection(vec![shape_id]);
    state.open_context_menu(
        (12, 34),
        vec![shape_id],
        ContextMenuKind::Shape,
        Some(shape_id),
    );

    let copy_entry = state
        .context_menu_entries()
        .into_iter()
        .find(|entry| entry.command == Some(MenuCommand::Copy))
        .expect("copy entry");

    assert!(copy_entry.disabled);
}

#[test]
fn copy_menu_command_publishes_selection_and_closes_menu() {
    let mut state = create_test_input_state();
    let shape_id = add_rect(&mut state);
    state.set_selection(vec![shape_id]);
    state.open_context_menu(
        (12, 34),
        vec![shape_id],
        ContextMenuKind::Shape,
        Some(shape_id),
    );

    state.execute_menu_command(MenuCommand::Copy);

    let publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending clipboard publish");
    assert_eq!(publish.generation, 1);
    assert!(publish.payload_json.contains("\"schema_version\":1"));
    assert!(matches!(state.context_menu.state, ContextMenuState::Hidden));
}

#[test]
fn paste_menu_command_uses_context_anchor_after_pointer_moves_to_menu_item() {
    let mut state = create_test_input_state();
    state.open_context_menu((44, 55), Vec::new(), ContextMenuKind::Canvas, None);
    state.update_pointer_positions(120, 140, 220, 240);

    state.execute_menu_command(MenuCommand::Paste);

    let request = state
        .take_pending_clipboard_paste_request()
        .expect("pending clipboard paste");
    assert_eq!(request.anchor.point(), (44, 55));
    assert!(matches!(state.context_menu.state, ContextMenuState::Hidden));
}
