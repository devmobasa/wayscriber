use super::*;
use crate::draw::TextMeasurer;
use crate::util::Rect;

/// Selection handles are 8px squares centred on the bounds, so the painted
/// chrome reaches this far past the selected shape.
const HANDLE_REACH: i32 = 5;

fn damage_state() -> InputState {
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
        color: state.style.current_color,
        thick: state.style.current_thickness,
    })
}

fn chrome_of(state: &InputState, id: crate::draw::ShapeId) -> Rect {
    let measurer = TextMeasurer::default();
    state
        .boards
        .active_frame()
        .shape(id)
        .and_then(|shape| shape.bounding_box_with(&measurer))
        .and_then(|bounds| bounds.inflated(HANDLE_REACH))
        .expect("shape has bounds")
}

fn assert_damage_covers(damage: &[Rect], expected: Rect, what: &str) {
    let covered = damage.iter().any(|region| {
        region.x <= expected.x
            && region.y <= expected.y
            && region.x + region.width >= expected.x + expected.width
            && region.y + region.height >= expected.y + expected.height
    });
    assert!(
        covered,
        "damage {damage:?} must cover the {what} chrome {expected:?}"
    );
}

#[test]
fn click_select_repaints_the_previous_and_the_new_selection_chrome() {
    let mut state = damage_state();
    let first = add_filled_rect(&mut state, 20, 20);
    let second = add_filled_rect(&mut state, 300, 200);
    state.set_selection(vec![first]);
    let _ = state.take_dirty_regions();

    state.on_mouse_press(MouseButton::Left, 320, 220);
    let press_damage = state.take_dirty_regions();

    assert_eq!(state.selected_shape_ids(), [second]);
    assert_damage_covers(&press_damage, chrome_of(&state, first), "deselected");
    assert_damage_covers(&press_damage, chrome_of(&state, second), "selected");

    state.on_mouse_release(MouseButton::Left, 320, 220);
    let release_damage = state.take_dirty_regions();

    assert_damage_covers(&release_damage, chrome_of(&state, second), "revealed");
}

#[test]
fn extending_the_selection_repaints_the_added_member() {
    let mut state = damage_state();
    let first = add_filled_rect(&mut state, 20, 20);
    let second = add_filled_rect(&mut state, 300, 200);
    state.set_selection(vec![first]);
    let _ = state.take_dirty_regions();

    state.extend_selection([second]);
    let damage = state.take_dirty_regions();

    assert_eq!(state.selected_shape_ids(), [first, second]);
    assert_damage_covers(&damage, chrome_of(&state, second), "added");
}

#[test]
fn rubber_band_select_repaints_the_previous_and_the_new_selection_chrome() {
    let mut state = damage_state();
    let first = add_filled_rect(&mut state, 20, 20);
    let second = add_filled_rect(&mut state, 300, 200);
    state.set_selection(vec![first]);
    let _ = state.take_dirty_regions();

    state.on_mouse_press(MouseButton::Left, 280, 180);
    state.on_mouse_motion(360, 260);
    state.on_mouse_release(MouseButton::Left, 360, 260);
    let damage = state.take_dirty_regions();

    assert_eq!(state.selected_shape_ids(), [second]);
    assert_damage_covers(&damage, chrome_of(&state, first), "deselected");
    assert_damage_covers(&damage, chrome_of(&state, second), "selected");
}

#[test]
fn clicking_empty_canvas_repaints_the_cleared_selection_chrome() {
    let mut state = damage_state();
    let first = add_filled_rect(&mut state, 20, 20);
    state.set_selection(vec![first]);
    let _ = state.take_dirty_regions();
    state.needs_redraw = false;

    state.on_mouse_press(MouseButton::Left, 500, 400);
    state.on_mouse_release(MouseButton::Left, 500, 400);
    let damage = state.take_dirty_regions();

    assert!(!state.has_selection());
    assert!(state.needs_redraw);
    assert_damage_covers(&damage, chrome_of(&state, first), "cleared");
}

#[test]
fn reselecting_the_same_shapes_adds_no_damage() {
    let mut state = damage_state();
    let first = add_filled_rect(&mut state, 20, 20);
    state.set_selection(vec![first]);
    let _ = state.take_dirty_regions();
    state.needs_redraw = false;

    state.set_selection(vec![first]);
    state.extend_selection([first]);

    assert!(state.take_dirty_regions().is_empty());
    assert!(!state.needs_redraw);
}

#[test]
fn undo_repaints_the_chrome_where_it_was_drawn_before_the_step() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = crate::ui_text::UiTextEngine::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };
    let mut state = damage_state();
    let first = add_filled_rect(&mut state, 20, 20);
    state.set_selection(vec![first]);
    state.handle_action_with_resources(resources, Action::NudgeSelectionRight);
    let moved_chrome = chrome_of(&state, first);
    let _ = state.take_dirty_regions();

    state.handle_action_with_resources(resources, Action::Undo);
    let damage = state.take_dirty_regions();

    assert!(!state.has_selection());
    assert_damage_covers(&damage, moved_chrome, "nudged");
}

#[test]
fn deselecting_text_repaints_its_resize_handle() {
    let measurer = TextMeasurer::default();
    let mut state = damage_state();
    let text = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 100,
        y: 100,
        text: "Handle".to_string(),
        color: state.style.current_color,
        size: state.style.current_font_size,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: false,
        wrap_width: None,
    });
    state.set_selection(vec![text]);
    let (_, handle) = state
        .selected_text_resize_handle_with(&measurer)
        .expect("a lone text selection has a resize handle");
    let _ = state.take_dirty_regions();

    state.on_key_press(Key::Escape);
    let damage = state.take_dirty_regions();

    assert!(!state.has_selection());
    assert_damage_covers(&damage, handle, "text resize handle");
}
