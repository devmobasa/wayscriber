use super::*;
use crate::input::BOARD_ID_WHITEBOARD;
use crate::util::Rect;

fn add_rect(state: &mut InputState, x: i32, y: i32, w: i32, h: i32) -> crate::draw::ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x,
        y,
        w,
        h,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    })
}

fn entry_index(state: &InputState, label: &str) -> usize {
    state
        .properties_panel()
        .expect("properties panel")
        .entries
        .iter()
        .position(|entry| entry.label == label)
        .expect(label)
}

#[test]
fn show_properties_panel_for_single_shape_reports_type_layer_and_lock_state() {
    let mut state = create_test_input_state();
    let shape_id = add_rect(&mut state, 10, 20, 30, 40);
    state.set_selection(vec![shape_id]);

    assert!(state.show_properties_panel());

    let panel = state.properties_panel().expect("properties panel");
    assert_eq!(panel.title, "Shape Properties");
    assert!(!panel.multiple_selection);
    assert!(
        panel
            .lines
            .iter()
            .any(|line| line == &format!("Shape ID: {shape_id}"))
    );
    assert!(panel.lines.iter().any(|line| line == "Type: Rectangle"));
    assert!(panel.lines.iter().any(|line| line == "Layer: 1 of 1"));
    assert!(panel.lines.iter().any(|line| line == "Locked: No"));
    assert!(panel.lines.iter().any(|line| line.starts_with("Bounds: ")));
}

#[test]
fn show_properties_panel_for_multi_selection_includes_locked_count_and_summary() {
    let mut state = create_test_input_state();
    let first = add_rect(&mut state, 10, 10, 20, 20);
    let second = add_rect(&mut state, 50, 15, 10, 15);
    let second_index = state
        .boards
        .active_frame()
        .find_index(second)
        .expect("second index");
    state.boards.active_frame_mut().shapes[second_index].locked = true;
    state.set_selection(vec![first, second]);

    assert!(state.show_properties_panel());

    let panel = state.properties_panel().expect("properties panel");
    assert_eq!(panel.title, "Selection Properties");
    assert!(panel.multiple_selection);
    assert!(panel.lines.iter().any(|line| line == "Shapes selected: 2"));
    assert!(panel.lines.iter().any(|line| line == "Locked: 1/2"));
    assert!(panel.lines.iter().any(|line| line.starts_with("Bounds: ")));
}

#[test]
fn close_properties_panel_clears_panel_and_requests_redraw() {
    let mut state = create_test_input_state();
    let shape_id = add_rect(&mut state, 5, 5, 10, 10);
    state.set_selection(vec![shape_id]);
    assert!(state.show_properties_panel());
    state.needs_redraw = false;

    state.close_properties_panel();

    assert!(state.properties_panel().is_none());
    assert!(state.properties_panel_layout().is_none());
    assert!(state.needs_redraw);
}

#[test]
fn show_properties_panel_anchors_to_screen_space_on_panned_boards() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    assert!(state.boards.active_frame_mut().set_view_offset(100, 50));
    state.update_pointer_position(400, 300);
    let shape_id = add_rect(&mut state, 140, 90, 20, 20);
    state.set_selection(vec![shape_id]);

    assert!(state.show_properties_panel());

    let panel = state.properties_panel().expect("properties panel");
    assert_eq!(panel.anchor_rect, Rect::new(38, 38, 24, 24));
}

#[test]
fn activate_fill_entry_toggles_rectangle_fill_and_refreshes_panel_value() {
    let mut state = create_test_input_state();
    let shape_id = add_rect(&mut state, 5, 5, 20, 20);
    state.set_selection(vec![shape_id]);
    assert!(state.show_properties_panel());
    let fill_index = entry_index(&state, "Fill");
    state.set_properties_panel_focus(Some(fill_index));

    assert!(state.activate_properties_panel_entry());

    match &state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("shape")
        .shape
    {
        Shape::Rect { fill, .. } => assert!(*fill),
        other => panic!("expected rect, got {other:?}"),
    }
    assert_eq!(
        state.properties_panel().expect("panel").entries[fill_index].value,
        "On"
    );
}

#[test]
fn adjust_font_size_entry_increases_text_size_and_refreshes_panel_value() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 10,
        y: 20,
        text: "Note".to_string(),
        color: state.current_color,
        size: 18.0,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: false,
        wrap_width: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(state.show_properties_panel());
    let font_index = entry_index(&state, "Font size");
    state.set_properties_panel_focus(Some(font_index));

    assert!(state.adjust_properties_panel_entry(1));

    match &state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("shape")
        .shape
    {
        Shape::Text { size, .. } => assert_eq!(*size, 20.0),
        other => panic!("expected text, got {other:?}"),
    }
    assert_eq!(
        state.properties_panel().expect("panel").entries[font_index].value,
        "20pt"
    );
}

#[test]
fn activate_text_background_entry_on_mixed_selection_turns_all_backgrounds_on() {
    let mut state = create_test_input_state();
    let first = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 10,
        y: 20,
        text: "One".to_string(),
        color: state.current_color,
        size: 18.0,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: false,
        wrap_width: None,
    });
    let second = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 50,
        text: "Two".to_string(),
        color: state.current_color,
        size: 18.0,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: true,
        wrap_width: None,
    });
    state.set_selection(vec![first, second]);
    assert!(state.show_properties_panel());
    let bg_index = entry_index(&state, "Text background");
    state.set_properties_panel_focus(Some(bg_index));

    assert!(state.activate_properties_panel_entry());

    for id in [first, second] {
        match &state
            .boards
            .active_frame()
            .shape(id)
            .expect("text shape")
            .shape
        {
            Shape::Text {
                background_enabled, ..
            } => assert!(*background_enabled),
            other => panic!("expected text, got {other:?}"),
        }
    }
    assert_eq!(
        state.properties_panel().expect("panel").entries[bg_index].value,
        "On"
    );
}

#[test]
fn adjust_arrow_length_entry_clamps_to_max_and_refreshes_panel_value() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Arrow {
        x1: 0,
        y1: 0,
        x2: 20,
        y2: 10,
        color: state.current_color,
        thick: 3.0,
        arrow_length: 49.0,
        arrow_angle: 30.0,
        head_at_end: true,
        label: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(state.show_properties_panel());
    let length_index = entry_index(&state, "Arrow length");
    state.set_properties_panel_focus(Some(length_index));

    assert!(state.adjust_properties_panel_entry(1));
    assert!(!state.adjust_properties_panel_entry(1));

    match &state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("arrow")
        .shape
    {
        Shape::Arrow { arrow_length, .. } => assert_eq!(*arrow_length, 50.0),
        other => panic!("expected arrow, got {other:?}"),
    }
    assert_eq!(
        state.properties_panel().expect("panel").entries[length_index].value,
        "50px"
    );
}
