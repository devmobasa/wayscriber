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
    let first = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.boards.active_frame_mut().add_shape(Shape::Rect {
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
fn select_this_shape_command_focuses_single_shape() {
    let mut state = create_test_input_state();
    let first = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.boards.active_frame_mut().add_shape(Shape::Rect {
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
        let frame = state.boards.active_frame_mut();
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
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
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
