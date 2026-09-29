use super::*;

fn add_rect(state: &mut InputState, x: i32, locked: bool) -> crate::draw::ShapeId {
    let id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x,
        y: 10,
        w: 10,
        h: 10,
        fill: false,
        fill_color: None,
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });
    let index = state.boards.active_frame().find_index(id).expect("index");
    state.boards.active_frame_mut().shapes[index].locked = locked;
    id
}

fn clear_from_the_canvas_menu(state: &mut InputState) {
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Canvas, None);
    state.execute_menu_command(MenuCommand::ClearAll);
}

#[test]
fn menu_clear_all_clears_laser_ink_and_offers_undo_like_the_toolbar() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Laser)));
    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_motion(120, 60);
    state.on_mouse_release(MouseButton::Left, 120, 60);
    assert!(state.laser.has_ink());
    add_rect(&mut state, 10, false);

    clear_from_the_canvas_menu(&mut state);

    assert!(!state.is_context_menu_open());
    assert!(state.boards.active_frame().shapes.is_empty());
    assert!(!state.laser.has_ink(), "Clear All left the laser ink");
    let toast = state.active_toast().expect("clear toast");
    assert_eq!(toast.message, "Cleared");
    assert_eq!(
        toast
            .action
            .as_ref()
            .and_then(|action| action.dispatch_action()),
        Some(Action::Undo)
    );
}

#[test]
fn menu_clear_all_says_that_locked_shapes_remain() {
    let mut state = create_test_input_state();
    let locked = add_rect(&mut state, 10, true);
    add_rect(&mut state, 40, false);

    clear_from_the_canvas_menu(&mut state);

    let shapes = &state.boards.active_frame().shapes;
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].id, locked);
    assert_eq!(
        state.active_toast().map(|toast| toast.message.as_str()),
        Some("Cleared unlocked shapes (locked shapes remain).")
    );
}
