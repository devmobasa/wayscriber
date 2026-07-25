//! The spotlight tool: drag geometry, region collection, and damage behavior.

use super::*;

fn only_shape(state: &InputState) -> &Shape {
    &state.boards.active_frame().shapes[0].shape
}

#[test]
fn dragging_the_spotlight_tool_commits_an_elliptical_region() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));

    state.on_mouse_press(MouseButton::Left, 100, 100);
    state.on_mouse_motion(200, 160);
    state.on_mouse_release(MouseButton::Left, 200, 160);

    match only_shape(&state) {
        Shape::Spotlight { cx, cy, rx, ry } => {
            assert_eq!((*cx, *cy), (150, 130), "centre is the drag box centre");
            assert_eq!((*rx, *ry), (50, 30));
        }
        other => panic!("expected a spotlight, got {other:?}"),
    }
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

    let regions = state.spotlight_regions((0, 0));
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

    let regions = state.spotlight_regions((140, 120));
    assert_eq!(regions.len(), 1, "the live drag should already dim");
    assert!((regions[0].cx - 90.0).abs() < 0.5);
    assert!((regions[0].rx - 50.0).abs() < 0.5);
}

#[test]
fn a_live_drag_of_another_tool_contributes_no_region() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Rect));

    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_motion(140, 120);

    assert!(state.spotlight_regions((140, 120)).is_empty());
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
    assert!(state.spotlight_regions((0, 0)).is_empty());
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
    };
    let bounds = shape.bounding_box().expect("spotlight has area");
    assert!(bounds.x <= 60 && bounds.y <= 80);
    assert!(bounds.x + bounds.width >= 140);
    assert!(bounds.y + bounds.height >= 120);
}
