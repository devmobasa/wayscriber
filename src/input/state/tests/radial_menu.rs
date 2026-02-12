use super::*;
use std::f64::consts::PI;

fn point_at(cx: f64, cy: f64, radius: f64, degrees: f64) -> (f64, f64) {
    let angle = degrees.to_radians();
    (cx + radius * angle.cos(), cy + radius * angle.sin())
}

fn point_in_tool_segment(layout: &RadialMenuLayout, segment_idx: u8) -> (f64, f64) {
    let seg_angle = 2.0 * PI / RADIAL_TOOL_SEGMENT_COUNT as f64;
    let angle = -PI / 2.0 + segment_idx as f64 * seg_angle;
    let radius = (layout.tool_inner + layout.tool_outer) / 2.0;
    (
        layout.center_x + radius * angle.cos(),
        layout.center_y + radius * angle.sin(),
    )
}

#[test]
fn radial_layout_small_surface_centers_menu_without_panic() {
    let mut state = create_test_input_state();
    state.open_radial_menu(8.0, 6.0);

    state.update_radial_menu_layout(120, 90);

    let layout = state
        .radial_menu_layout
        .expect("layout should be computed even on tiny surfaces");
    assert!((layout.center_x - 60.0).abs() < f64::EPSILON);
    assert!((layout.center_y - 45.0).abs() < f64::EPSILON);
}

#[test]
fn radial_hover_collapse_without_hover_change_requests_redraw() {
    let mut state = create_test_input_state();
    state.open_radial_menu(400.0, 300.0);
    state.update_radial_menu_layout(800, 600);

    let layout = state
        .radial_menu_layout
        .expect("layout should exist for open radial menu");

    // Hover Shapes parent (segment 4) to expand the sub-ring.
    state.needs_redraw = false;
    let (tool4_x, tool4_y) = point_in_tool_segment(&layout, 4);
    state.update_radial_menu_hover(tool4_x, tool4_y);
    assert!(state.needs_redraw);

    // Move to sub-ring radius outside parent angle. Hover becomes None, sub-ring stays expanded.
    state.needs_redraw = false;
    let (sub_none_x, sub_none_y) = point_at(layout.center_x, layout.center_y, 110.0, 0.0);
    state.update_radial_menu_hover(sub_none_x, sub_none_y);
    assert!(state.needs_redraw);

    // Move into center/tool gap: hover remains None, but expanded sub-ring should collapse.
    // This transition must still request redraw.
    state.needs_redraw = false;
    let (collapse_x, collapse_y) = point_at(layout.center_x, layout.center_y, 35.0, 0.0);
    state.update_radial_menu_hover(collapse_x, collapse_y);
    assert!(state.needs_redraw);
}

#[test]
fn size_for_active_tool_uses_eraser_size_for_eraser() {
    let mut state = create_test_input_state();
    state.current_thickness = 3.0;
    state.eraser_size = 17.0;

    assert!(state.set_tool_override(Some(Tool::Eraser)));
    assert!((state.size_for_active_tool() - 17.0).abs() < f64::EPSILON);

    assert!(state.set_tool_override(Some(Tool::Pen)));
    assert!((state.size_for_active_tool() - 3.0).abs() < f64::EPSILON);
}

#[test]
fn opening_radial_menu_closes_help_overlay() {
    let mut state = create_test_input_state();
    state.toggle_help_overlay();
    assert!(state.show_help);

    state.open_radial_menu(120.0, 90.0);

    assert!(!state.show_help);
    assert!(state.is_radial_menu_open());
}

#[test]
fn right_click_when_radial_open_opens_context_menu() {
    let mut state = create_test_input_state();
    state.open_radial_menu(200.0, 150.0);
    assert!(state.is_radial_menu_open());

    state.on_mouse_press(MouseButton::Right, 200, 150);

    assert!(!state.is_radial_menu_open());
    assert!(state.is_context_menu_open());
}

#[test]
fn selecting_clear_tool_segment_clears_canvas() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 24,
        y: 24,
        w: 80,
        h: 64,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    assert!(state.boards.active_frame().shape(shape_id).is_some());

    state.open_radial_menu(220.0, 160.0);
    state.update_radial_menu_layout(800, 600);
    let layout = state
        .radial_menu_layout
        .expect("layout should exist for open radial menu");

    // Segment 8 is the Clear Canvas action.
    let (clear_x, clear_y) = point_in_tool_segment(&layout, 8);
    state.update_radial_menu_hover(clear_x, clear_y);
    state.radial_menu_select_hovered();

    assert!(!state.is_radial_menu_open());
    assert!(state.boards.active_frame().shape(shape_id).is_none());
}

#[test]
fn color_hit_test_uses_color_ring_alignment_when_tool_count_differs() {
    let mut state = create_test_input_state();
    state.open_radial_menu(300.0, 220.0);
    state.update_radial_menu_layout(900, 700);
    let layout = state
        .radial_menu_layout
        .expect("layout should exist for open radial menu");

    // Just inside color segment 1 (Green), close to the segment boundary.
    let seg = 2.0 * PI / RADIAL_COLOR_SEGMENT_COUNT as f64;
    let probe_angle = -PI / 2.0 + seg + 0.04;
    let probe_radius = (layout.color_inner + layout.color_outer) / 2.0;
    let probe_x = layout.center_x + probe_radius * probe_angle.cos();
    let probe_y = layout.center_y + probe_radius * probe_angle.sin();

    state.update_radial_menu_hover(probe_x, probe_y);
    state.radial_menu_select_hovered();

    let expected = radial_color_for_index(1);
    assert!(colors_approx_eq(&state.current_color, &expected));
}

fn colors_approx_eq(a: &Color, b: &Color) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
}
