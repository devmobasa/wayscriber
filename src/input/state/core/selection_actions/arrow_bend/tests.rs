use super::*;
use crate::draw::Color;
use crate::input::state::DrawingState;
use crate::input::state::test_support::make_test_input_state;

/// A curved arrow pointing right along y = 100, from (0, 100) to (400, 100).
///
/// `head_at_end` is true, so `(x2, y2)` is the tip and the tail-to-tip
/// direction runs left to right — which is what the bend's sign is measured
/// against.
fn add_curved_arrow(state: &mut crate::input::InputState, bend: f64) -> crate::draw::ShapeId {
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
        bend,
        label: None,
    })
}

#[test]
fn handle_rides_the_arc_not_the_chord() {
    let mut state = make_test_input_state();
    let id = add_curved_arrow(&mut state, 0.4);
    state.set_selection(vec![id]);

    let handle = state
        .selected_arrow_bend_handle()
        .expect("a selected curved arrow has a bend handle");
    let center_y = handle.rect.y + handle.rect.height / 2;
    // Bend 0.4 over a 400px chord puts the arc's midpoint 80px off the chord.
    // A handle parked on the chord midpoint would sit at y = 100 and be
    // nowhere near the curve it edits.
    assert_eq!(handle.rect.x + handle.rect.width / 2, 200);
    assert_eq!(center_y, 20);
}

#[test]
fn handle_is_offered_only_for_a_single_unlocked_curved_arrow() {
    let mut state = make_test_input_state();
    let curved = add_curved_arrow(&mut state, 0.3);
    let straight = state.boards.active_frame_mut().add_shape(Shape::Arrow {
        x1: 0,
        y1: 200,
        x2: 400,
        y2: 200,
        color: state.style.current_color,
        thick: 4.0,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        head_at_end: true,
        style: ArrowStyle::Standard,
        bend: 0.5,
        label: None,
    });

    state.set_selection(vec![straight]);
    assert!(
        state.selected_arrow_bend_handle().is_none(),
        "a straight arrow draws no arc, so there is nothing to bend"
    );

    state.set_selection(vec![curved, straight]);
    assert!(
        state.selected_arrow_bend_handle().is_none(),
        "a multi-selection has no honest handle position"
    );

    state.set_selection(vec![curved]);
    assert!(state.selected_arrow_bend_handle().is_some());

    let index = state
        .boards
        .active_frame()
        .shapes
        .iter()
        .position(|drawn| drawn.id == curved)
        .expect("curved arrow index");
    state.boards.active_frame_mut().shapes[index].locked = true;
    assert!(
        state.selected_arrow_bend_handle().is_none(),
        "a locked arrow must not offer an edit handle"
    );
}

#[test]
fn dragging_the_handle_bends_toward_the_pointer() {
    let test_text_measurer = crate::draw::TextMeasurer::default();

    let mut state = make_test_input_state();
    let id = add_curved_arrow(&mut state, 0.0);
    state.set_selection(vec![id]);
    state.state = DrawingState::BendingArrow {
        shape_id: id,
        snapshot: crate::draw::frame::ShapeSnapshot {
            shape: state
                .boards
                .active_frame()
                .shape(id)
                .expect("arrow")
                .shape
                .clone(),
            locked: false,
        },
    };

    // Pointer 80px above the chord midpoint: the arc's midpoint should follow
    // it, which needs bend = 2 * 80 / 400 = 0.4 on the left-of-travel side.
    assert!(state.drag_arrow_bend_to_with(&test_text_measurer, 200, 20, false));
    assert!((arrow_bend(&state, id) - 0.4).abs() < 1e-9);

    // And the other way. A sign flip here means the arrow curves away from
    // the drag.
    assert!(state.drag_arrow_bend_to_with(&test_text_measurer, 200, 180, false));
    assert!((arrow_bend(&state, id) + 0.4).abs() < 1e-9);
}

#[test]
fn dragging_along_the_chord_does_not_change_the_bend() {
    let test_text_measurer = crate::draw::TextMeasurer::default();

    let mut state = make_test_input_state();
    let id = add_curved_arrow(&mut state, 0.0);
    state.set_selection(vec![id]);
    state.state = DrawingState::BendingArrow {
        shape_id: id,
        snapshot: crate::draw::frame::ShapeSnapshot {
            shape: state
                .boards
                .active_frame()
                .shape(id)
                .expect("arrow")
                .shape
                .clone(),
            locked: false,
        },
    };

    // Only the perpendicular component counts, which is what keeps the arc
    // symmetric however far along it the user grabs.
    assert!(state.drag_arrow_bend_to_with(&test_text_measurer, 120, 40, false));
    let from_left = arrow_bend(&state, id);
    assert!(
        !state.drag_arrow_bend_to_with(&test_text_measurer, 330, 40, false),
        "sliding along the chord should be a no-op, not a new bend"
    );
    assert!((arrow_bend(&state, id) - from_left).abs() < 1e-9);
}

#[test]
fn shift_snaps_the_bend_to_tenths() {
    let test_text_measurer = crate::draw::TextMeasurer::default();

    let mut state = make_test_input_state();
    let id = add_curved_arrow(&mut state, 0.0);
    state.set_selection(vec![id]);
    state.state = DrawingState::BendingArrow {
        shape_id: id,
        snapshot: crate::draw::frame::ShapeSnapshot {
            shape: state
                .boards
                .active_frame()
                .shape(id)
                .expect("arrow")
                .shape
                .clone(),
            locked: false,
        },
    };

    // 43px off the chord is bend 0.215, which snaps to 0.2.
    assert!(state.drag_arrow_bend_to_with(&test_text_measurer, 200, 57, true));
    assert!(
        (arrow_bend(&state, id) - 0.2).abs() < 1e-9,
        "shift did not snap: got {}",
        arrow_bend(&state, id)
    );
}

#[test]
fn the_handle_follows_the_head_end() {
    // `head_at_end` decides which stored point is the tip, and the bend sign is
    // measured against the tail-to-tip direction. Flipping the head therefore
    // has to flip which side the same bend bulges toward, or the handle and the
    // rendered arc end up on opposite sides of the chord.
    let mut state = make_test_input_state();
    let id = add_curved_arrow(&mut state, 0.4);
    state.set_selection(vec![id]);
    let above = state.selected_arrow_bend_handle().expect("handle").rect.y;

    if let Some(drawn) = state.boards.active_frame_mut().shape_mut(id)
        && let Shape::Arrow { head_at_end, .. } = &mut drawn.shape
    {
        *head_at_end = false;
    }
    let below = state.selected_arrow_bend_handle().expect("handle").rect.y;

    assert!(
        above < 100 && below > 100,
        "flipping the head did not mirror the handle: {above} then {below}"
    );
}

fn arrow_bend(state: &crate::input::InputState, id: crate::draw::ShapeId) -> f64 {
    match &state.boards.active_frame().shape(id).expect("arrow").shape {
        Shape::Arrow { bend, .. } => *bend,
        other => panic!("expected arrow, got {other:?}"),
    }
}

#[test]
fn restyling_mid_gesture_ends_the_bend_instead_of_stacking_on_it() {
    let test_text_measurer = crate::draw::TextMeasurer::default();
    let test_ui_engine = crate::ui_text::UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    // `CycleArrowStyle` is bindable, so it can land while the bend handle is
    // still held. Restyling on top of a live gesture pushes an undo entry while
    // the gesture is still holding a pre-bend snapshot, and the eventual
    // release pushes a second entry measured from that same stale snapshot — so
    // undo walks back through an arrow that never existed on screen. Leaving
    // `Curved` also hides the very arc the drag is editing.
    let mut state = make_test_input_state();
    let id = add_curved_arrow(&mut state, 0.0);
    state.set_selection(vec![id]);
    let before = crate::draw::frame::ShapeSnapshot {
        shape: state
            .boards
            .active_frame()
            .shape(id)
            .expect("arrow")
            .shape
            .clone(),
        locked: false,
    };
    state.state = DrawingState::BendingArrow {
        shape_id: id,
        snapshot: before,
    };
    assert!(state.drag_arrow_bend_to_with(&test_text_measurer, 200, 20, false));
    let bent = arrow_bend(&state, id);
    assert!(bent.abs() > 0.1, "test setup should have bent the arrow");

    state.handle_action_with_resources(test_text_resources, crate::config::Action::CycleArrowStyle);

    assert!(
        matches!(state.state, DrawingState::Idle),
        "restyling left the bend gesture running"
    );
    // The bend is recorded first, then the restyle: two entries, each measured
    // from the state the one before it left behind.
    assert_eq!(state.boards.active_frame().undo_stack_len(), 2);

    // Undo the restyle, then the bend, and the arrow is back where it started
    // with nothing in between that was never drawn.
    state.handle_action_with_resources(test_text_resources, crate::config::Action::Undo);
    assert_eq!(arrow_style(&state, id), ArrowStyle::Curved);
    assert!((arrow_bend(&state, id) - bent).abs() < 1e-9);
    state.handle_action_with_resources(test_text_resources, crate::config::Action::Undo);
    assert_eq!(arrow_bend(&state, id), 0.0);
    assert_eq!(arrow_style(&state, id), ArrowStyle::Curved);
}

fn arrow_style(state: &crate::input::InputState, id: crate::draw::ShapeId) -> ArrowStyle {
    match &state.boards.active_frame().shape(id).expect("arrow").shape {
        Shape::Arrow { style, .. } => *style,
        other => panic!("expected arrow, got {other:?}"),
    }
}

#[test]
fn any_selection_property_change_ends_a_live_bend_first() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let test_ui_engine = crate::ui_text::UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &test_ui_engine,
    };

    // The guard sits on `dispatch_selection_property`, not on the arrow-style
    // action, because the toolbar and the shape properties panel reach the same
    // mutators by other routes — and because the hazard is not style-specific.
    // A thickness change swallowed into a live bend's snapshot pair would be
    // undone by undoing the bend, which is not what either edit asked for.
    let mut state = make_test_input_state();
    let id = add_curved_arrow(&mut state, 0.0);
    state.set_selection(vec![id]);
    state.state = DrawingState::BendingArrow {
        shape_id: id,
        snapshot: crate::draw::frame::ShapeSnapshot {
            shape: state
                .boards
                .active_frame()
                .shape(id)
                .expect("arrow")
                .shape
                .clone(),
            locked: false,
        },
    };
    assert!(state.drag_arrow_bend_to_with(&route_measurer, 200, 20, false));
    let bent = arrow_bend(&state, id);

    state.adjust_selection_property_kind_with(
        &route_measurer,
        crate::input::state::core::properties::SelectionPropertyKind::Thickness,
        1,
    );

    assert!(
        matches!(state.state, DrawingState::Idle),
        "a thickness change left the bend gesture running"
    );
    // Undoing the thickness change must leave the bend intact rather than
    // rolling the arrow back past a gesture that had already been committed.
    state.handle_action_with_resources(test_text_resources, crate::config::Action::Undo);
    assert!((arrow_bend(&state, id) - bent).abs() < 1e-9);
}

#[test]
fn a_nudge_key_mid_gesture_ends_the_bend_instead_of_stacking_on_it() {
    let test_text_measurer = crate::draw::TextMeasurer::default();
    let test_ui_engine = crate::ui_text::UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    // Pressed as a real key, not dispatched as an action: a bound key goes
    // from `keyboard.rs` straight into `route_action` and never touches
    // `handle_action`, so a preflight hung off the latter would leave the
    // ordinary key press — which is most of them — going around it.
    //
    // Recording a nudge from the live bent shape while the gesture still holds
    // a pre-bend snapshot means the release records a second entry from that
    // stale snapshot, so undoing twice puts the bend *back* instead of taking
    // it away.
    let mut state = make_test_input_state();
    let id = add_curved_arrow(&mut state, 0.0);
    state.set_selection(vec![id]);
    state.state = DrawingState::BendingArrow {
        shape_id: id,
        snapshot: crate::draw::frame::ShapeSnapshot {
            shape: state
                .boards
                .active_frame()
                .shape(id)
                .expect("arrow")
                .shape
                .clone(),
            locked: false,
        },
    };
    assert!(state.drag_arrow_bend_to_with(&test_text_measurer, 200, 20, false));
    let bent = arrow_bend(&state, id);
    assert!(bent.abs() > 0.1, "test setup should have bent the arrow");

    state.on_key_press(crate::input::Key::Right);

    assert!(
        matches!(state.state, DrawingState::Idle),
        "a nudge left the bend gesture running"
    );
    assert_eq!(state.boards.active_frame().undo_stack_len(), 2);

    // Undo the nudge, then the bend. Neither step may resurrect the other.
    state.handle_action_with_resources(test_text_resources, crate::config::Action::Undo);
    assert!((arrow_bend(&state, id) - bent).abs() < 1e-9);
    state.handle_action_with_resources(test_text_resources, crate::config::Action::Undo);
    assert_eq!(arrow_bend(&state, id), 0.0);
}

#[test]
fn escape_still_cancels_a_bend_rather_than_committing_it() {
    let test_text_measurer = crate::draw::TextMeasurer::default();

    // `route_action` ends a live bend for every action except Exit, whose whole
    // job is to back out of the gesture. Committing first would leave Escape
    // with nothing to cancel and quietly keep the arc. Driven through the key
    // so the exception is checked on the path that actually carries it.
    let mut state = make_test_input_state();
    let id = add_curved_arrow(&mut state, 0.0);
    state.set_selection(vec![id]);
    state.state = DrawingState::BendingArrow {
        shape_id: id,
        snapshot: crate::draw::frame::ShapeSnapshot {
            shape: state
                .boards
                .active_frame()
                .shape(id)
                .expect("arrow")
                .shape
                .clone(),
            locked: false,
        },
    };
    assert!(state.drag_arrow_bend_to_with(&test_text_measurer, 200, 20, false));

    state.on_key_press(crate::input::Key::Escape);

    assert!(matches!(state.state, DrawingState::Idle));
    assert_eq!(arrow_bend(&state, id), 0.0, "Escape kept the bend");
    assert_eq!(
        state.boards.active_frame().undo_stack_len(),
        0,
        "a cancelled bend must not be recorded"
    );
    assert!(
        !state.should_exit,
        "Escape cancelled the gesture, not the app"
    );
}

#[test]
fn an_action_with_no_bend_running_leaves_the_interaction_alone() {
    // `finish_active_arrow_bend` runs on every action now, so taking the state
    // apart before confirming a bend is running would cancel whatever else was.
    let mut state = make_test_input_state();
    state.state = DrawingState::Selecting {
        start_x: 10,
        start_y: 10,
        additive: false,
    };

    assert!(!state.finish_active_arrow_bend());
    assert!(
        matches!(state.state, DrawingState::Selecting { .. }),
        "an unrelated interaction was cancelled, got {:?}",
        state.state
    );
}

#[test]
fn a_toolbar_event_mid_gesture_ends_the_bend_before_it_can_lose_the_arrow() {
    let test_text_measurer = crate::draw::TextMeasurer::default();
    let test_ui_engine = crate::ui_text::UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    // Toolbar events never reach `route_action`, and touch, tablet, and the GTK
    // toolbar all deliver them while a pointer-held gesture is running. Undo All
    // is the sharp case: it can remove the arrow outright, after which the
    // release finds no shape and drops the bend without a trace.
    let mut state = make_test_input_state();
    let id = add_curved_arrow(&mut state, 0.0);
    state.set_selection(vec![id]);
    state.state = DrawingState::BendingArrow {
        shape_id: id,
        snapshot: crate::draw::frame::ShapeSnapshot {
            shape: state
                .boards
                .active_frame()
                .shape(id)
                .expect("arrow")
                .shape
                .clone(),
            locked: false,
        },
    };
    assert!(state.drag_arrow_bend_to_with(&test_text_measurer, 200, 20, false));
    let bent = arrow_bend(&state, id);

    state.apply_toolbar_event(crate::ui::toolbar::ToolbarEvent::UndoAll);

    assert!(
        matches!(state.state, DrawingState::Idle),
        "a toolbar event left the bend gesture running"
    );
    // The bend was recorded before Undo All ran, so it is on the stack to be
    // undone rather than lost with the shape.
    state.handle_action_with_resources(test_text_resources, crate::config::Action::RedoAll);
    assert!(
        (arrow_bend(&state, id) - bent).abs() < 1e-9,
        "the bend was dropped instead of committed before the toolbar event"
    );
}
