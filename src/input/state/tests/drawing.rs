use super::*;
use crate::input::{DragBinding, DragButtonBindings, DragToolBindings};
use crate::ui::toolbar::ToolbarEvent;

fn left_drag_bindings(
    drag: Tool,
    shift_drag: Tool,
    ctrl_drag: Tool,
    ctrl_shift_drag: Tool,
    tab_drag: Tool,
) -> DragToolBindings {
    DragToolBindings {
        left: DragButtonBindings {
            drag: DragBinding::from_tool(drag),
            shift_drag: DragBinding::from_tool(shift_drag),
            ctrl_drag: DragBinding::from_tool(ctrl_drag),
            ctrl_shift_drag: DragBinding::from_tool(ctrl_shift_drag),
            tab_drag: DragBinding::from_tool(tab_drag),
        },
        right: DragButtonBindings::button_default(),
        middle: DragButtonBindings::button_default(),
    }
}

#[test]
fn mouse_drag_creates_shapes_for_each_tool() {
    let mut state = create_test_input_state();

    // Pen
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_motion(10, 10);
    state.on_mouse_release(MouseButton::Left, 10, 10);
    assert_eq!(state.boards.active_frame().shapes.len(), 1);
    state.clear_selection();

    // Line (Shift)
    state.modifiers.shift = true;
    state.on_mouse_press(MouseButton::Left, 20, 20);
    state.on_mouse_release(MouseButton::Left, 25, 25);
    assert_eq!(state.boards.active_frame().shapes.len(), 2);
    state.clear_selection();

    // Rectangle (Ctrl)
    state.modifiers.shift = false;
    state.modifiers.ctrl = true;
    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_release(MouseButton::Left, 45, 45);
    assert_eq!(state.boards.active_frame().shapes.len(), 3);
    state.clear_selection();

    // Ellipse (Tab)
    state.modifiers.ctrl = false;
    state.modifiers.tab = true;
    state.on_mouse_press(MouseButton::Left, 60, 60);
    state.on_mouse_release(MouseButton::Left, 64, 64);
    assert_eq!(state.boards.active_frame().shapes.len(), 4);
    state.clear_selection();

    // Arrow (Ctrl+Shift)
    state.modifiers.tab = false;
    state.modifiers.ctrl = true;
    state.modifiers.shift = true;
    state.on_mouse_press(MouseButton::Left, 80, 80);
    state.on_mouse_release(MouseButton::Left, 86, 86);
    assert_eq!(state.boards.active_frame().shapes.len(), 5);
}

#[test]
fn regular_polygon_drag_stores_concrete_points_and_side_metadata() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::RegularPolygon)));
    assert!(state.set_polygon_sides(7));

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 100, 100);

    let shape = &state.boards.active_frame().shapes[0].shape;
    match shape {
        Shape::Polygon {
            kind: crate::draw::PolygonKind::Regular { sides },
            points,
            ..
        } => {
            assert_eq!(*sides, 7);
            assert_eq!(points.len(), 7);
        }
        other => panic!("expected regular polygon, got {other:?}"),
    }

    assert!(state.set_polygon_sides(3));
    match &state.boards.active_frame().shapes[0].shape {
        Shape::Polygon {
            kind: crate::draw::PolygonKind::Regular { sides },
            points,
            ..
        } => {
            assert_eq!(*sides, 7);
            assert_eq!(points.len(), 7);
        }
        other => panic!("expected regular polygon, got {other:?}"),
    }
}

#[test]
fn invalid_drag_polygon_tools_do_not_commit_ghost_shapes() {
    for (tool, start, end) in [
        (Tool::Triangle, (10, 10), (10, 10)),
        (Tool::RegularPolygon, (10, 10), (10, 10)),
        (Tool::Parallelogram, (0, 0), (1, 10)),
    ] {
        let mut state = create_test_input_state();
        assert!(state.set_tool_override(Some(tool)));

        state.on_mouse_press(MouseButton::Left, start.0, start.1);
        state.on_mouse_release(MouseButton::Left, end.0, end.1);

        assert!(
            state.boards.active_frame().shapes.is_empty(),
            "{tool:?} should not commit an invalid invisible polygon"
        );
    }
}

#[test]
fn alt_click_selects_filled_polygon_interior() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Polygon {
        kind: crate::draw::PolygonKind::Triangle,
        points: vec![(10, 10), (40, 10), (25, 40)],
        fill: true,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.modifiers.alt = true;
    state.on_mouse_press(MouseButton::Left, 25, 22);

    assert_eq!(state.selected_shape_ids(), &[shape_id]);
}

#[test]
fn freeform_polygon_double_click_finishes_without_duplicate_vertex() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::FreeformPolygon)));

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_press(MouseButton::Left, 20, 0);
    state.on_mouse_press(MouseButton::Left, 20, 20);
    state.on_mouse_press(MouseButton::Left, 20, 20);

    assert!(matches!(state.state, DrawingState::Idle));
    match &state.boards.active_frame().shapes[0].shape {
        Shape::Polygon {
            kind: crate::draw::PolygonKind::Freeform,
            points,
            ..
        } => assert_eq!(points, &vec![(0, 0), (20, 0), (20, 20)]),
        other => panic!("expected freeform polygon, got {other:?}"),
    }
}

#[test]
fn freeform_polygon_backspace_does_not_prime_double_click_commit() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::FreeformPolygon)));

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_press(MouseButton::Left, 20, 0);
    state.on_mouse_press(MouseButton::Left, 20, 20);
    state.on_mouse_press(MouseButton::Left, 0, 20);
    state.pop_building_polygon_point();
    state.on_mouse_press(MouseButton::Left, 0, 20);

    assert!(state.boards.active_frame().shapes.is_empty());
    match &state.state {
        DrawingState::BuildingPolygon { points, .. } => {
            assert_eq!(points, &vec![(0, 0), (20, 0), (20, 20), (0, 20)]);
        }
        other => panic!("expected polygon still building, got {other:?}"),
    }
}

#[test]
fn freeform_polygon_commit_records_first_stroke_onboarding() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::FreeformPolygon)));

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_press(MouseButton::Left, 20, 0);
    state.on_mouse_press(MouseButton::Left, 20, 20);
    state.finish_building_polygon();

    assert!(state.pending_onboarding_usage.first_stroke_done);
}

#[test]
fn freeform_polygon_preview_dirty_has_antialias_padding() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::FreeformPolygon)));

    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_motion(20, 20);

    let DrawingState::BuildingPolygon { thick, .. } = state.state else {
        panic!("expected polygon building state");
    };
    let base = crate::draw::shape::bounding_box_for_points(&[(10, 10), (20, 20)], thick)
        .expect("preview should have bounds");
    assert_eq!(
        state.last_provisional_bounds,
        base.inflated(2),
        "building polygon damage should be padded to clear antialias leftovers"
    );
}

#[test]
fn append_path_motion_dirties_only_new_tail_segment() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Pen)));
    let _ = state.take_dirty_regions();

    state.on_mouse_press(MouseButton::Left, 10, 10);
    let _ = state.take_dirty_regions();
    state.on_mouse_motion(20, 10);
    let _ = state.take_dirty_regions();

    state.on_mouse_motion(30, 10);
    let dirty = state.take_dirty_regions();
    let thick = state.thickness_for_tool(Tool::Pen);
    let tail_bounds = crate::draw::shape::bounding_box_for_points(&[(20, 10), (30, 10)], thick)
        .expect("tail segment should have bounds");
    let full_bounds = crate::draw::shape::bounding_box_for_points(&[(10, 10), (30, 10)], thick)
        .expect("full provisional stroke should have bounds");
    let head_probe = crate::util::Rect::new(10, 10, 1, 1).unwrap();

    assert!(
        dirty
            .iter()
            .any(|rect| test_rects_intersect(*rect, tail_bounds)),
        "dirty regions should include the new tail segment; dirty={dirty:?}, tail={tail_bounds:?}"
    );
    assert!(
        !dirty
            .iter()
            .any(|rect| test_rects_intersect(*rect, head_probe)),
        "append motion should not redraw the start of the accumulated stroke; dirty={dirty:?}"
    );
    assert_eq!(
        state.last_provisional_bounds,
        Some(full_bounds),
        "cleanup bounds should still cover the whole active stroke"
    );
}

#[test]
fn pressure_sample_shrink_dirties_previous_full_provisional_bounds() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Pen)));
    assert!(state.set_thickness(32.0));
    let _ = state.take_dirty_regions();

    state.on_mouse_press(MouseButton::Left, 10, 10);
    let old_full_bounds = state
        .last_provisional_bounds
        .expect("initial pressure preview should have bounds");
    let old_only_probe = crate::util::Rect::new(10, old_full_bounds.y, 1, 1).unwrap();
    let _ = state.take_dirty_regions();

    state.set_pressure_thickness_for_active_tool(2.0);
    state.on_mouse_motion(30, 10);
    let dirty = state.take_dirty_regions();

    assert!(
        dirty
            .iter()
            .any(|rect| test_rects_intersect(*rect, old_only_probe)),
        "shrinking the first pressure sample should dirty old wide preview pixels; dirty={dirty:?}, old_full={old_full_bounds:?}, probe={old_only_probe:?}"
    );
}

#[test]
fn marker_size_increase_updates_accumulated_cleanup_bounds() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Marker)));
    assert!(state.set_thickness(2.0));
    let _ = state.take_dirty_regions();

    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_motion(20, 10);
    let _ = state.take_dirty_regions();

    assert!(state.set_thickness(32.0));
    let _ = state.take_dirty_regions();

    state.cancel_active_interaction();
    let dirty = state.take_dirty_regions();
    let marker_width = (32.0f64 * 1.35).max(32.0 + 1.0);
    let expanded_bounds =
        crate::draw::shape::bounding_box_for_points(&[(10, 10), (20, 10)], marker_width)
            .expect("expanded marker preview should have bounds");
    let expanded_only_probe = crate::util::Rect::new(10, expanded_bounds.y, 1, 1).unwrap();

    assert!(
        dirty
            .iter()
            .any(|rect| test_rects_intersect(*rect, expanded_only_probe)),
        "cancel should dirty marker pixels exposed by a mid-stroke size increase; dirty={dirty:?}, expanded={expanded_bounds:?}, probe={expanded_only_probe:?}"
    );
    assert_eq!(state.last_provisional_bounds, None);
}

#[test]
fn eraser_size_increase_updates_accumulated_cleanup_bounds() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Eraser)));
    assert!(state.set_eraser_size(2.0));
    let _ = state.take_dirty_regions();

    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_motion(20, 10);
    let _ = state.take_dirty_regions();

    assert!(state.set_eraser_size(32.0));
    let _ = state.take_dirty_regions();

    state.cancel_active_interaction();
    let dirty = state.take_dirty_regions();
    let expanded_bounds = crate::draw::shape::bounding_box_for_eraser(&[(10, 10), (20, 10)], 32.0)
        .expect("expanded eraser preview should have bounds");
    let expanded_only_probe = crate::util::Rect::new(10, expanded_bounds.y, 1, 1).unwrap();

    assert!(
        dirty
            .iter()
            .any(|rect| test_rects_intersect(*rect, expanded_only_probe)),
        "cancel should dirty eraser pixels exposed by a mid-stroke size increase; dirty={dirty:?}, expanded={expanded_bounds:?}, probe={expanded_only_probe:?}"
    );
    assert_eq!(state.last_provisional_bounds, None);
}

#[test]
fn cancel_active_path_dirties_full_accumulated_provisional_bounds() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Pen)));
    let _ = state.take_dirty_regions();

    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_motion(20, 10);
    state.on_mouse_motion(30, 10);
    let _ = state.take_dirty_regions();

    state.cancel_active_interaction();
    let dirty = state.take_dirty_regions();
    let thick = state.thickness_for_tool(Tool::Pen);
    let full_bounds = crate::draw::shape::bounding_box_for_points(&[(10, 10), (30, 10)], thick)
        .expect("full provisional stroke should have bounds");

    assert!(
        dirty
            .iter()
            .any(|rect| test_rects_intersect(*rect, full_bounds)),
        "cancel should dirty the full active stroke bounds; dirty={dirty:?}, full={full_bounds:?}"
    );
    assert_eq!(state.last_provisional_bounds, None);
}

#[test]
fn freeform_polygon_freezes_style_on_first_click() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::FreeformPolygon)));
    let original = state.current_color;
    let changed = crate::draw::Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    state.on_mouse_press(MouseButton::Left, 0, 0);
    assert!(state.set_color(changed));
    state.on_mouse_press(MouseButton::Left, 20, 0);
    state.on_mouse_press(MouseButton::Left, 20, 20);
    state.finish_building_polygon();

    match &state.boards.active_frame().shapes[0].shape {
        Shape::Polygon { color, .. } => assert_eq!(*color, original),
        other => panic!("expected freeform polygon, got {other:?}"),
    }
}

#[test]
fn custom_drag_bindings_remap_default_and_modifier_tools() {
    let mut state = create_test_input_state();
    assert!(state.set_drag_tool_bindings(left_drag_bindings(
        Tool::Arrow,
        Tool::Eraser,
        Tool::Pen,
        Tool::Rect,
        Tool::Ellipse,
    )));

    assert_eq!(state.active_tool(), Tool::Arrow);
    assert!(state.set_tool_override(Some(Tool::Arrow)));
    assert_eq!(state.active_tool(), Tool::Arrow);

    state.modifiers.ctrl = true;
    assert_eq!(state.active_tool(), Tool::Pen);

    state.modifiers.ctrl = false;
    state.modifiers.shift = true;
    assert_eq!(state.active_tool(), Tool::Eraser);

    state.modifiers.ctrl = true;
    assert_eq!(state.active_tool(), Tool::Rect);
}

#[test]
fn blur_drag_requests_frozen_capture_on_press() {
    let mut state = create_test_input_state();
    assert!(state.set_drag_tool_bindings(left_drag_bindings(
        Tool::Blur,
        Tool::Line,
        Tool::Rect,
        Tool::Arrow,
        Tool::Ellipse,
    )));

    state.on_mouse_press(MouseButton::Left, 12, 14);

    assert!(state.take_pending_frozen_toggle());
    assert!(matches!(
        state.state,
        DrawingState::Drawing {
            tool: Tool::Blur,
            ..
        }
    ));
}

#[test]
fn drag_mapped_highlight_reports_highlight_active() {
    let mut state = create_test_input_state();
    assert!(state.set_drag_tool_bindings(left_drag_bindings(
        Tool::Highlight,
        Tool::Line,
        Tool::Rect,
        Tool::Arrow,
        Tool::Ellipse,
    )));

    assert_eq!(state.active_tool(), Tool::Highlight);
    assert!(state.highlight_tool_active());

    state.modifiers.shift = true;
    assert_eq!(state.active_tool(), Tool::Line);
    assert!(!state.highlight_tool_active());
}

#[test]
fn right_button_drag_uses_configured_tool() {
    let mut state = create_test_input_state();
    let mut bindings = DragToolBindings::default();
    bindings.right.drag = DragBinding::from_tool(Tool::Line);
    assert!(state.set_drag_tool_bindings(bindings));

    state.on_mouse_press(MouseButton::Right, 10, 20);
    assert!(matches!(
        state.state,
        DrawingState::Drawing {
            tool: Tool::Line,
            ..
        }
    ));
    state.on_mouse_release(MouseButton::Right, 30, 40);

    assert_eq!(state.boards.active_frame().shapes.len(), 1);
    assert!(matches!(
        state.boards.active_frame().shapes[0].shape,
        Shape::Line { .. }
    ));
}

#[test]
fn configured_non_left_drag_closes_context_menu_before_drawing() {
    let mut state = create_test_input_state();
    let mut bindings = DragToolBindings::default();
    bindings.right.drag = DragBinding::from_tool(Tool::Pen);
    assert!(state.set_drag_tool_bindings(bindings));
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Canvas, None);

    state.on_mouse_press(MouseButton::Right, 300, 300);
    state.on_mouse_motion(320, 320);
    state.on_mouse_release(MouseButton::Right, 320, 320);

    assert!(!state.is_context_menu_open());
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.boards.active_frame().shapes.is_empty());
}

#[test]
fn drag_binding_color_overrides_stroke_without_changing_current_color() {
    let mut state = create_test_input_state();
    let original_color = state.current_color;
    let blue = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    let mut bindings = DragToolBindings::default();
    bindings.right.drag = DragBinding::new(crate::input::DragTool::Pen, Some(blue));
    assert!(state.set_drag_tool_bindings(bindings));

    state.on_mouse_press(MouseButton::Right, 0, 0);
    state.on_mouse_motion(10, 10);
    state.on_mouse_release(MouseButton::Right, 10, 10);

    assert_eq!(state.current_color, original_color);
    match &state.boards.active_frame().shapes[0].shape {
        Shape::Freehand { color, .. } => assert_eq!(*color, blue),
        other => panic!("expected freehand shape, got {other:?}"),
    }
}

#[test]
fn toggle_click_highlight_action_changes_state() {
    let mut state = create_test_input_state();
    assert!(!state.click_highlight_enabled());

    state.handle_action(Action::ToggleClickHighlight);
    assert!(state.click_highlight_enabled());
    assert!(state.needs_redraw);

    state.needs_redraw = false;
    state.handle_action(Action::ToggleClickHighlight);
    assert!(!state.click_highlight_enabled());
    assert!(state.needs_redraw);
}

#[test]
fn toolbar_select_highlight_syncs_click_highlight_state() {
    let mut state = create_test_input_state();
    assert!(!state.highlight_tool_active());
    assert!(!state.click_highlight_enabled());

    assert!(state.apply_toolbar_event(ToolbarEvent::SelectTool(Tool::Highlight)));

    assert_eq!(state.tool_override(), Some(Tool::Highlight));
    assert!(state.highlight_tool_active());
    assert!(state.click_highlight_enabled());
}

#[test]
fn toolbar_select_highlight_sticks_when_highlight_is_active_via_modifier() {
    let mut state = create_test_input_state();
    let mut bindings = DragToolBindings::default();
    bindings.left.shift_drag = DragBinding::from_tool(Tool::Highlight);
    assert!(state.set_drag_tool_bindings(bindings));
    assert!(state.set_tool_override(Some(Tool::Pen)));

    state.on_key_press(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Highlight);
    assert_eq!(state.tool_override(), Some(Tool::Pen));

    assert!(state.apply_toolbar_event(ToolbarEvent::SelectTool(Tool::Highlight)));
    state.on_key_release(Key::Shift);

    assert_eq!(state.tool_override(), Some(Tool::Highlight));
    assert_eq!(state.active_tool(), Tool::Highlight);
    assert!(state.click_highlight_enabled());
}

#[test]
fn highlight_tool_prevents_drawing() {
    let mut state = create_test_input_state();
    assert_eq!(state.active_tool(), Tool::Pen);
    assert!(!state.highlight_tool_active());

    state.handle_action(Action::ToggleHighlightTool);
    assert!(state.highlight_tool_active());
    assert_eq!(state.active_tool(), Tool::Highlight);

    // Enable highlight effect to ensure no shapes are added while clicks happen
    state.handle_action(Action::ToggleClickHighlight);

    let initial_shapes = state.boards.active_frame().shapes.len();
    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_release(MouseButton::Left, 20, 20);
    assert_eq!(state.boards.active_frame().shapes.len(), initial_shapes);
    assert!(matches!(state.state, DrawingState::Idle));

    // Toggle highlight tool off and ensure pen drawing resumes
    state.handle_action(Action::ToggleHighlightTool);
    assert!(!state.highlight_tool_active());
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 5, 5);
    assert_eq!(state.boards.active_frame().shapes.len(), initial_shapes + 1);
}

#[test]
fn sync_highlight_color_marks_dirty_when_pen_color_changes() {
    let mut state = create_test_input_state();
    state.needs_redraw = false;
    state.tool_settings.pen.color = Color {
        r: 0.25,
        g: 0.5,
        b: 0.75,
        a: 1.0,
    };
    state.sync_highlight_color();
    assert!(state.needs_redraw);
}

fn test_rects_intersect(a: crate::util::Rect, b: crate::util::Rect) -> bool {
    let a_right = a.x.saturating_add(a.width);
    let a_bottom = a.y.saturating_add(a.height);
    let b_right = b.x.saturating_add(b.width);
    let b_bottom = b.y.saturating_add(b.height);

    !(a.x >= b_right || a_right <= b.x || a.y >= b_bottom || a_bottom <= b.y)
}
