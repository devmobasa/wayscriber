//! The spotlight tool: drag geometry, region collection, and damage behavior.

use super::*;

fn only_shape(state: &InputState) -> &Shape {
    &state.boards.active_frame().shapes[0].shape
}

#[test]
fn dragging_the_spotlight_tool_commits_an_elliptical_region() {
    let mut state = create_test_input_state();
    state.spotlight_magnification = 2.25;
    state.set_tool_override(Some(Tool::Spotlight));
    assert!(
        !state.take_pending_frozen_toggle(),
        "selecting Spotlight must not capture the screen automatically"
    );

    state.on_mouse_press(MouseButton::Left, 100, 100);
    state.on_mouse_motion(200, 160);
    state.on_mouse_release(MouseButton::Left, 200, 160);
    assert!(state.take_pending_spotlight_magnifier_feedback());
    assert!(
        !state.take_pending_frozen_toggle(),
        "committing a magnified Spotlight must not capture automatically"
    );

    match only_shape(&state) {
        Shape::Spotlight {
            cx,
            cy,
            rx,
            ry,
            magnification,
        } => {
            assert_eq!((*cx, *cy), (150, 130), "centre is the drag box centre");
            assert_eq!((*rx, *ry), (50, 30));
            assert_eq!(*magnification, 2.25);
        }
        other => panic!("expected a spotlight, got {other:?}"),
    }
    assert_eq!(
        state.spotlight_frame_regions(Some((0, 0))).regions[0].magnification,
        2.25
    );
}

#[test]
fn committed_spotlights_are_collected_for_the_render_pass() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 100, 80);
    state.clear_selection();
    state.on_mouse_press(MouseButton::Left, 300, 300);
    state.on_mouse_release(MouseButton::Left, 400, 380);

    let regions = state.spotlight_frame_regions(Some((0, 0))).regions;
    assert_eq!(regions.len(), 2, "both spotlights must reach the pass");
    assert!(regions.iter().any(|r| (r.cx - 50.0).abs() < 0.5));
    assert!(regions.iter().any(|r| (r.cx - 350.0).abs() < 0.5));
}

#[test]
fn the_in_progress_drag_is_included_so_dimming_follows_the_cursor() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));

    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_motion(140, 120);

    let regions = state.spotlight_frame_regions(Some((140, 120))).regions;
    assert_eq!(regions.len(), 1, "the live drag should already dim");
    assert!((regions[0].cx - 90.0).abs() < 0.5);
    assert!((regions[0].rx - 50.0).abs() < 0.5);
}

#[test]
fn an_in_progress_drag_dims_but_does_not_count_as_something_the_page_holds() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));
    assert!(state.set_spotlight_magnification(2.5));

    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_motion(140, 120);

    // The drag dims immediately, but nothing is committed yet: a warning that
    // describes the page must not fire for an ellipse that cancelling erases.
    let collected = state.spotlight_frame_regions(Some((140, 120)));
    assert_eq!(
        collected.regions.len(),
        1,
        "the live drag should already dim"
    );
    assert!(
        !collected.committed_magnified,
        "a drag under the pointer is not yet something the page holds"
    );

    state.on_mouse_release(MouseButton::Left, 140, 120);
    let collected = state.spotlight_frame_regions(None);
    assert_eq!(collected.regions.len(), 1);
    assert!(
        collected.committed_magnified,
        "once committed, the page does hold a magnified Spotlight"
    );
}

#[test]
fn suppressing_transients_collects_committed_regions_only() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));

    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_motion(140, 120);

    assert!(
        state.spotlight_frame_regions(None).regions.is_empty(),
        "a frame that shows no transients draws no in-progress drag"
    );
}

#[test]
fn a_live_drag_of_another_tool_contributes_no_region() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Rect));

    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_motion(140, 120);

    assert!(
        state
            .spotlight_frame_regions(Some((140, 120)))
            .regions
            .is_empty()
    );
    assert!(!state.has_spotlight());
}

#[test]
fn has_spotlight_reports_both_committed_and_in_progress_regions() {
    let mut state = create_test_input_state();
    assert!(!state.has_spotlight(), "empty page dims nothing");

    state.set_tool_override(Some(Tool::Spotlight));
    state.on_mouse_press(MouseButton::Left, 10, 10);
    assert!(
        state.has_spotlight(),
        "a drag in flight already dims the screen"
    );

    state.on_mouse_release(MouseButton::Left, 90, 70);
    assert!(
        state.has_spotlight(),
        "the committed spotlight keeps dimming"
    );
}

#[test]
fn deleting_the_spotlight_stops_the_dimming() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));
    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_release(MouseButton::Left, 90, 70);
    assert!(state.has_spotlight());

    state.boards.active_frame_mut().shapes.clear();
    assert!(!state.has_spotlight());
    assert!(
        state
            .spotlight_frame_regions(Some((0, 0)))
            .regions
            .is_empty()
    );
}

#[test]
fn a_spotlight_is_selectable_anywhere_inside_its_opening() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));
    state.on_mouse_press(MouseButton::Left, 100, 100);
    state.on_mouse_release(MouseButton::Left, 200, 200);
    state.clear_selection();

    // The shape paints nothing, so the whole opening has to be clickable.
    let id = state.boards.active_frame().shapes[0].id;
    assert_eq!(
        state.hit_test_at(150, 150),
        Some(id),
        "the centre of the opening should select the spotlight"
    );
    assert_eq!(
        state.hit_test_at(400, 400),
        None,
        "a point outside the opening should not select it"
    );
}

#[test]
fn spotlight_exposes_a_readable_kind_name() {
    let shape = Shape::Spotlight {
        cx: 0,
        cy: 0,
        rx: 10,
        ry: 10,
        magnification: crate::draw::DEFAULT_SPOTLIGHT_MAGNIFICATION,
    };
    assert_eq!(shape.kind_name(), "Spotlight");
}

#[test]
fn spotlight_bounds_cover_its_opening() {
    let shape = Shape::Spotlight {
        cx: 100,
        cy: 100,
        rx: 40,
        ry: 20,
        magnification: crate::draw::DEFAULT_SPOTLIGHT_MAGNIFICATION,
    };
    let bounds = shape.bounding_box().expect("spotlight has area");
    assert!(bounds.x <= 60 && bounds.y <= 80);
    assert!(bounds.x + bounds.width >= 140);
    assert!(bounds.y + bounds.height >= 120);
}
