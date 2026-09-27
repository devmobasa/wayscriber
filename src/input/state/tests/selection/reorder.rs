use super::*;
use crate::draw::{ShapeId, TextMeasurer};

fn add(state: &mut InputState, x: i32) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: Color::new(1.0, 0.0, 0.0, 1.0),
        thick: 2.0,
    })
}

fn order(state: &InputState) -> Vec<ShapeId> {
    state
        .boards
        .active_frame()
        .shapes
        .iter()
        .map(|shape| shape.id)
        .collect()
}

fn stack(count: i32) -> (InputState, Vec<ShapeId>) {
    let mut state = create_test_input_state();
    let ids = (0..count)
        .map(|index| add(&mut state, index * 20))
        .collect();
    (state, ids)
}

#[test]
fn move_to_front_and_back_reach_the_ends_of_the_stack() {
    let measurer = TextMeasurer::default();
    let (mut state, ids) = stack(3);

    state.set_selection(vec![ids[0]]);
    assert!(state.move_selection_to_front_with(&measurer));
    assert_eq!(order(&state), vec![ids[1], ids[2], ids[0]]);

    state.set_selection(vec![ids[2]]);
    assert!(state.move_selection_to_back_with(&measurer));
    assert_eq!(order(&state), vec![ids[2], ids[1], ids[0]]);
    assert!(
        !state.move_selection_to_back_with(&measurer),
        "already at the back"
    );

    // From one below the top, the old reading could not move it at all.
    state.set_selection(vec![ids[1]]);
    assert!(state.move_selection_to_front_with(&measurer));
    assert_eq!(order(&state), vec![ids[2], ids[0], ids[1]]);
}

#[test]
fn forward_and_backward_step_one_shape_past_its_neighbour() {
    let measurer = TextMeasurer::default();
    let (mut state, ids) = stack(3);
    state.set_selection(vec![ids[0]]);

    assert!(state.selection_can_step(true));
    assert!(!state.selection_can_step(false));
    assert!(state.move_selection_forward_with(&measurer));
    assert_eq!(order(&state), vec![ids[1], ids[0], ids[2]]);
    assert!(state.move_selection_forward_with(&measurer));
    assert_eq!(order(&state), vec![ids[1], ids[2], ids[0]]);
    assert!(!state.selection_can_step(true));
    assert!(
        !state.move_selection_forward_with(&measurer),
        "already on top"
    );

    assert!(state.move_selection_backward_with(&measurer));
    assert_eq!(order(&state), vec![ids[1], ids[0], ids[2]]);
}

#[test]
fn a_selected_block_steps_as_one_and_keeps_its_own_order() {
    let measurer = TextMeasurer::default();
    let (mut state, ids) = stack(4);
    state.set_selection(vec![ids[0], ids[1]]);

    assert!(state.move_selection_forward_with(&measurer));

    assert_eq!(order(&state), vec![ids[2], ids[0], ids[1], ids[3]]);
}

#[test]
fn a_step_is_one_undo_entry_that_undoes_and_redoes() {
    let measurer = TextMeasurer::default();
    let resources_engine = crate::ui_text::UiTextEngine::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &resources_engine,
    };
    let (mut state, ids) = stack(4);
    let before = order(&state);
    state.set_selection(vec![ids[0], ids[2]]);

    assert!(state.move_selection_forward_with(&measurer));
    let after = order(&state);
    assert_eq!(after, vec![ids[1], ids[0], ids[3], ids[2]]);

    state.handle_action_with_resources(resources, Action::Undo);
    assert_eq!(order(&state), before);
    state.handle_action_with_resources(resources, Action::Redo);
    assert_eq!(order(&state), after);
}
