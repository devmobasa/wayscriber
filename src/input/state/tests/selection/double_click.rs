use super::*;

fn select_state() -> InputState {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(800, 600);
    state.set_tool_override(Some(Tool::Select));
    state
}

fn add_filled_rect(state: &mut InputState, x: i32, y: i32) -> crate::draw::ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x,
        y,
        w: 40,
        h: 40,
        fill: true,
        fill_color: None,
        color: state.style.current_color,
        thick: state.style.current_thickness,
    })
}

fn click(state: &mut InputState, x: i32, y: i32) {
    state.on_mouse_press(MouseButton::Left, x, y);
    state.on_mouse_release(MouseButton::Left, x, y);
}

fn drag(state: &mut InputState, from: (i32, i32), to: (i32, i32)) {
    state.on_mouse_press(MouseButton::Left, from.0, from.1);
    state.on_mouse_motion(to.0, to.1);
    state.on_mouse_release(MouseButton::Left, to.0, to.1);
}

fn bounds_of(state: &InputState, id: crate::draw::ShapeId) -> crate::util::Rect {
    state
        .boards
        .active_frame()
        .shape(id)
        .and_then(|shape| shape.shape.bounding_box())
        .expect("shape bounds")
}

#[test]
fn select_tool_double_click_opens_properties_for_a_shape() {
    let mut state = select_state();
    let rect = add_filled_rect(&mut state, 100, 100);

    click(&mut state, 120, 120);
    assert_eq!(state.selected_shape_ids(), &[rect]);
    assert!(
        !state.is_properties_panel_open(),
        "one click only selects the shape"
    );

    click(&mut state, 120, 120);
    assert!(state.is_properties_panel_open());
    assert!(matches!(state.state, DrawingState::Idle));
    assert_eq!(state.selected_shape_ids(), &[rect]);
}

#[test]
fn select_tool_double_click_edits_text_instead_of_opening_properties() {
    let mut state = select_state();
    let text = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 120,
        y: 120,
        text: "Hello".to_string(),
        color: state.style.current_color,
        size: state.style.current_font_size,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: state.style.text_background_enabled,
        wrap_width: None,
    });
    let bounds = state
        .boards
        .active_frame()
        .shape(text)
        .unwrap()
        .shape
        .bounding_box()
        .expect("text bounds");

    // The middle, clear of the corner handles the first click reveals.
    let (x, y) = (bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
    click(&mut state, x, y);
    click(&mut state, x, y);

    assert!(matches!(state.state, DrawingState::TextInput { .. }));
    assert!(!state.is_properties_panel_open());
}

#[test]
fn a_drag_between_clicks_does_not_make_a_double_click() {
    let mut state = select_state();
    let rect = add_filled_rect(&mut state, 100, 100);
    let before = bounds_of(&state, rect);

    click(&mut state, 120, 120);
    drag(&mut state, (120, 120), (160, 120));
    assert!(!state.is_properties_panel_open(), "a drag is not a click");
    assert_eq!(
        bounds_of(&state, rect).x,
        before.x + 40,
        "past the click radius the shape follows the pointer exactly"
    );

    click(&mut state, 160, 120);
    assert!(
        !state.is_properties_panel_open(),
        "the click before the drag must not pair with the one after it"
    );
    click(&mut state, 160, 120);
    assert!(state.is_properties_panel_open());
}

#[test]
fn alt_double_click_opens_properties_without_drawing() {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(800, 600);
    let rect = add_filled_rect(&mut state, 100, 100);
    assert!(!matches!(
        state.active_tool().press_behavior(),
        crate::input::tool::ToolPressBehavior::Selection
    ));
    state.modifiers.alt = true;

    click(&mut state, 120, 120);
    click(&mut state, 120, 120);

    assert!(state.is_properties_panel_open());
    assert_eq!(state.selected_shape_ids(), &[rect]);
    assert_eq!(state.boards.active_frame().shapes.len(), 1);
}

#[test]
fn double_click_narrows_a_selection_unless_shift_is_held() {
    let mut state = select_state();
    let first = add_filled_rect(&mut state, 100, 100);
    let second = add_filled_rect(&mut state, 300, 100);

    state.set_selection(vec![first, second]);
    click(&mut state, 120, 120);
    assert_eq!(
        state.selected_shape_ids(),
        &[first, second],
        "one click on a selected shape keeps the selection for dragging"
    );
    click(&mut state, 120, 120);
    assert!(state.is_properties_panel_open());
    assert_eq!(state.selected_shape_ids(), &[first]);

    // Shift alone picks the Shift drag tool; Alt+Shift is the selection
    // gesture that extends rather than replaces.
    state.close_properties_panel();
    state.set_selection(vec![first, second]);
    state.modifiers.alt = true;
    state.modifiers.shift = true;
    click(&mut state, 120, 120);
    click(&mut state, 120, 120);
    assert!(state.is_properties_panel_open());
    assert_eq!(state.selected_shape_ids(), &[first, second]);
}

#[test]
fn a_click_on_another_page_does_not_pair_with_this_one() {
    let mut state = select_state();
    add_filled_rect(&mut state, 100, 100);
    click(&mut state, 120, 120);

    state.page_new();
    add_filled_rect(&mut state, 100, 100);
    click(&mut state, 120, 120);

    assert!(
        !state.is_properties_panel_open(),
        "shape ids are page-local, so the first click belongs to the other page"
    );
}

#[test]
fn a_wobble_within_the_click_radius_is_still_a_click_and_moves_nothing() {
    let mut state = select_state();
    let rect = add_filled_rect(&mut state, 100, 100);
    let before = bounds_of(&state, rect);
    let history = state.boards.active_frame().undo_stack_len();

    drag(&mut state, (120, 120), (121, 121));
    drag(&mut state, (120, 120), (122, 119));

    assert!(state.is_properties_panel_open());
    assert_eq!(bounds_of(&state, rect), before);
    assert_eq!(state.boards.active_frame().undo_stack_len(), history);
}

#[test]
fn pointer_travel_against_an_edge_is_a_drag_even_when_nothing_moves() {
    let mut state = select_state();
    let rect = add_filled_rect(&mut state, 300, 100);
    // Park the rectangle flush against the right edge first.
    drag(&mut state, (320, 120), (2000, 120));
    let parked = bounds_of(&state, rect);
    assert_eq!(parked.x + parked.width, 800, "the rectangle is at the edge");
    let (x, y) = (parked.x + parked.width / 2, parked.y + parked.height / 2);

    click(&mut state, x, y);
    drag(&mut state, (x, y), (x + 60, y));
    assert_eq!(bounds_of(&state, rect), parked, "the edge stops the move");
    click(&mut state, x, y);

    assert!(
        !state.is_properties_panel_open(),
        "the drag in between breaks the pair even though nothing moved"
    );
}

#[test]
fn a_locked_shape_opens_in_properties_on_double_click() {
    let mut state = select_state();
    let rect = add_filled_rect(&mut state, 100, 100);
    state.boards.active_frame_mut().shapes[0].locked = true;
    let before = bounds_of(&state, rect);

    drag(&mut state, (120, 120), (200, 200));
    assert_eq!(bounds_of(&state, rect), before, "a locked shape stays put");

    click(&mut state, 120, 120);
    click(&mut state, 120, 120);
    assert!(state.is_properties_panel_open());
    assert_eq!(state.selected_shape_ids(), &[rect]);
}
