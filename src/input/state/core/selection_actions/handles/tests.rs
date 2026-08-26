use super::*;
use crate::draw::{ArrowStyle, Color, Shape};
use crate::input::state::DrawingState;
use crate::input::state::test_support::make_test_input_state;

/// A curved arrow whose arc is shallow enough that its bend grip lands inside
/// the selection box's top-edge handle.
fn add_shallow_curved_arrow(state: &mut crate::input::InputState) -> crate::draw::ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Arrow {
        x1: 0,
        y1: 100,
        x2: 400,
        y2: 100,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 4.0,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        head_at_end: true,
        style: ArrowStyle::Curved,
        bend: 0.05,
        label: None,
    })
}

#[test]
fn a_shallow_bend_grip_outranks_the_selection_edge_handle_it_overlaps() {
    // This is the collision the ordering exists for. At a shallow bend the grip
    // sits a couple of pixels off the chord, well inside the top edge handle's
    // tolerance, and whichever probe runs first wins the pixel.
    let mut state = make_test_input_state();
    let id = add_shallow_curved_arrow(&mut state);
    state.set_selection(vec![id]);

    let grip = state
        .selected_arrow_bend_handle()
        .expect("a selected curved arrow has a bend handle");
    let center = (
        grip.rect.x + grip.rect.width / 2,
        grip.rect.y + grip.rect.height / 2,
    );

    assert!(
        state.hit_selection_handle(center.0, center.1).is_some(),
        "test setup should have put the grip inside an edge handle"
    );
    assert_eq!(
        state.hit_idle_handle(center.0, center.1),
        Some(IdleHandle::ArrowBend(id)),
        "the edge handle swallowed the bend grip"
    );
}

#[test]
fn what_the_routing_reports_is_what_a_press_starts() {
    // The cursor is drawn from `hit_idle_handle` and the press is dispatched
    // from it, so the two agree by construction — but only while the press arms
    // keep matching the variants. This is what notices if one drifts, and it is
    // the bug the shared routing replaced: the pointer showed a resize arrow
    // over a grip that a click would bend.
    let mut state = make_test_input_state();
    let id = add_shallow_curved_arrow(&mut state);
    state.set_selection(vec![id]);

    let grip = state.selected_arrow_bend_handle().expect("bend handle");
    let center = (
        grip.rect.x + grip.rect.width / 2,
        grip.rect.y + grip.rect.height / 2,
    );
    assert_eq!(
        state.hit_idle_handle(center.0, center.1),
        Some(IdleHandle::ArrowBend(id))
    );

    state.on_mouse_press(crate::input::events::MouseButton::Left, center.0, center.1);
    assert!(
        matches!(state.state, DrawingState::BendingArrow { .. }),
        "routing promised a bend but the press started {:?}",
        state.state
    );
}

#[test]
fn empty_canvas_routes_to_no_handle() {
    let state = make_test_input_state();
    assert_eq!(state.hit_idle_handle(50, 50), None);
}
