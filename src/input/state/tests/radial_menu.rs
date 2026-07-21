use super::helpers::create_test_input_state_with_keybindings;
use super::*;
use crate::config::{QuickColorPalette, QuickColorPaletteEntry};
use crate::input::BOARD_ID_WHITEBOARD;
use std::f64::consts::PI;

fn point_at(cx: f64, cy: f64, radius: f64, degrees: f64) -> (f64, f64) {
    let angle = degrees.to_radians();
    (cx + radius * angle.cos(), cy + radius * angle.sin())
}

/// Raw screen angle (atan2 degrees, y-down) of a compass direction's wedge
/// center. The compass is fixed: wedge centers sit exactly on these angles.
fn compass_angle_degrees(dir: CompassDir) -> f64 {
    match dir {
        CompassDir::N => -90.0,
        CompassDir::NE => -45.0,
        CompassDir::E => 0.0,
        CompassDir::SE => 45.0,
        CompassDir::S => 90.0,
        CompassDir::SW => 135.0,
        CompassDir::W => 180.0,
        CompassDir::NW => -135.0,
    }
}

fn point_in_compass_wedge(layout: &RadialMenuLayout, dir: CompassDir) -> (f64, f64) {
    let radius = (layout.tool_inner + layout.tool_outer) / 2.0;
    point_at(
        layout.center_x,
        layout.center_y,
        radius,
        compass_angle_degrees(dir),
    )
}

fn point_in_sub_tool_segment(
    layout: &RadialMenuLayout,
    parent_idx: u8,
    child_idx: u8,
    child_count: usize,
) -> (f64, f64) {
    let seg_angle = 2.0 * PI / RADIAL_TOOL_SEGMENT_COUNT as f64;
    let half_seg = seg_angle / 2.0;
    let child_angle = seg_angle / child_count as f64;
    // Compute the desired tool_angle in hit-test space (0 at top, with half-seg offset).
    let tool_angle =
        parent_idx as f64 * seg_angle + child_idx as f64 * child_angle + child_angle / 2.0;
    // Convert back to raw atan2 angle (undo the +PI/2 and +half_seg transforms).
    let raw_angle = tool_angle - PI / 2.0 - half_seg;
    let radius = (layout.sub_inner + layout.sub_outer) / 2.0;
    (
        layout.center_x + radius * raw_angle.cos(),
        layout.center_y + radius * raw_angle.sin(),
    )
}

fn open_with_layout(state: &mut InputState) -> RadialMenuLayout {
    state.open_radial_menu(400.0, 300.0);
    state.update_radial_menu_layout(800, 600);
    state
        .radial_menu_layout
        .expect("layout should exist for open radial menu")
}

fn expanded_sub_ring_of(state: &InputState) -> Option<u8> {
    match &state.radial_menu_state {
        RadialMenuState::Open {
            expanded_sub_ring, ..
        } => *expanded_sub_ring,
        RadialMenuState::Hidden => panic!("radial menu should be open"),
    }
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
fn compass_slice_table_is_the_fixed_eight_way_layout() {
    assert_eq!(RADIAL_TOOL_SEGMENT_COUNT, 8);
    let expected = [
        (
            CompassDir::N,
            RadialSliceKind::Action(Action::SelectPenTool),
        ),
        (
            CompassDir::NE,
            RadialSliceKind::Action(Action::SelectMarkerTool),
        ),
        (CompassDir::E, RadialSliceKind::Parent(RadialParent::Shapes)),
        (
            CompassDir::SE,
            RadialSliceKind::Action(Action::SelectArrowTool),
        ),
        (
            CompassDir::S,
            RadialSliceKind::Action(Action::SelectSelectionTool),
        ),
        (CompassDir::SW, RadialSliceKind::Parent(RadialParent::Notes)),
        (
            CompassDir::W,
            RadialSliceKind::Action(Action::EnterTextMode),
        ),
        (
            CompassDir::NW,
            RadialSliceKind::Action(Action::SelectEraserTool),
        ),
    ];
    for (i, (dir, kind)) in expected.into_iter().enumerate() {
        assert_eq!(dir.index() as usize, i, "compass order drifted at {dir:?}");
        assert_eq!(
            CompassDir::ALL[i],
            dir,
            "CompassDir::ALL order drifted at {dir:?}"
        );
        assert_eq!(RADIAL_COMPASS_SLICES[i].dir, dir);
        assert_eq!(RADIAL_COMPASS_SLICES[i].kind, kind, "slice {dir:?} changed");
    }
}

#[test]
fn compass_wedges_dispatch_fixed_tools_via_handle_action() {
    let cases = [
        (CompassDir::N, Tool::Pen),
        (CompassDir::NE, Tool::Marker),
        (CompassDir::SE, Tool::Arrow),
        (CompassDir::S, Tool::Select),
        (CompassDir::NW, Tool::Eraser),
    ];
    for (dir, expected_tool) in cases {
        let mut state = create_test_input_state();
        let layout = open_with_layout(&mut state);

        let (x, y) = point_in_compass_wedge(&layout, dir);
        state.update_radial_menu_hover(x, y);
        state.radial_menu_select_hovered();

        assert!(
            !state.is_radial_menu_open(),
            "{dir:?} should close the menu"
        );
        assert_eq!(
            state.active_tool(),
            expected_tool,
            "{dir:?} wedge should activate {expected_tool:?}"
        );
    }
}

#[test]
fn north_wedge_spans_22_5_degrees_around_straight_up() {
    let mut state = create_test_input_state();
    let layout = open_with_layout(&mut state);
    let radius = (layout.tool_inner + layout.tool_outer) / 2.0;

    for (degrees, expected_dir) in [
        (-90.0 + 21.0, CompassDir::N),
        (-90.0 - 21.0, CompassDir::N),
        (-90.0 + 24.0, CompassDir::NE),
        (-90.0 - 24.0, CompassDir::NW),
    ] {
        let (x, y) = point_at(layout.center_x, layout.center_y, radius, degrees);
        state.update_radial_menu_hover(x, y);
        let hover = match &state.radial_menu_state {
            RadialMenuState::Open { hover, .. } => *hover,
            RadialMenuState::Hidden => panic!("menu should stay open while hovering"),
        };
        assert_eq!(
            hover,
            Some(RadialSegmentId::Tool(expected_dir.index())),
            "probe at {degrees} degrees should hit the {expected_dir:?} wedge"
        );
    }
}

#[test]
fn west_wedge_enters_text_mode_and_closes_menu() {
    let mut state = create_test_input_state();
    let layout = open_with_layout(&mut state);

    let (x, y) = point_in_compass_wedge(&layout, CompassDir::W);
    state.update_radial_menu_hover(x, y);
    state.radial_menu_select_hovered();

    assert!(!state.is_radial_menu_open());
    assert!(matches!(state.state, DrawingState::TextInput { .. }));
    assert!(matches!(state.text_input_mode, TextInputMode::Plain));
}

#[test]
fn east_wedge_expands_shapes_sub_ring_without_closing() {
    let mut state = create_test_input_state();
    let layout = open_with_layout(&mut state);

    let (x, y) = point_in_compass_wedge(&layout, CompassDir::E);
    state.update_radial_menu_hover(x, y);
    assert_eq!(expanded_sub_ring_of(&state), Some(CompassDir::E.index()));

    state.radial_menu_select_hovered();

    assert!(state.is_radial_menu_open(), "parent select keeps menu open");
    assert_eq!(expanded_sub_ring_of(&state), Some(CompassDir::E.index()));
}

#[test]
fn southwest_wedge_expands_notes_sub_ring_without_closing() {
    let mut state = create_test_input_state();
    let layout = open_with_layout(&mut state);

    let (x, y) = point_in_compass_wedge(&layout, CompassDir::SW);
    state.update_radial_menu_hover(x, y);
    state.radial_menu_select_hovered();

    assert!(state.is_radial_menu_open());
    assert_eq!(expanded_sub_ring_of(&state), Some(CompassDir::SW.index()));
}

#[test]
fn shapes_sub_ring_matches_shape_tools_catalog() {
    let children = sub_ring_children(CompassDir::E.index());
    assert_eq!(
        children,
        &[
            Action::SelectLineTool,
            Action::SelectRectTool,
            Action::SelectEllipseTool,
            Action::SelectBlurTool,
            Action::SelectRegularPolygonTool,
        ]
    );

    // Membership and order both come from the toolbar's shape_tools()
    // catalog, the shapes source of truth.
    let catalog = crate::ui::toolbar::model::shape_tools();
    let mut previous_position = None;
    for action in children {
        let tool = Tool::from_select_action(*action).expect("shape child must be a tool action");
        let position = catalog
            .iter()
            .position(|entry| *entry == tool)
            .unwrap_or_else(|| panic!("{tool:?} missing from shape_tools()"));
        if let Some(previous) = previous_position {
            assert!(
                position > previous,
                "sub-ring order must follow shape_tools() order"
            );
        }
        previous_position = Some(position);
    }
}

#[test]
fn notes_sub_ring_is_step_marker_then_sticky_note() {
    assert_eq!(
        sub_ring_children(CompassDir::SW.index()),
        &[Action::SelectStepMarkerTool, Action::EnterStickyNoteMode]
    );
}

#[test]
fn non_parent_wedges_have_no_sub_ring_children() {
    for slice in RADIAL_COMPASS_SLICES.iter() {
        let expected = matches!(slice.kind, RadialSliceKind::Parent(_));
        assert_eq!(
            sub_ring_child_count(slice.dir.index()) > 0,
            expected,
            "sub-ring children mismatch for {:?}",
            slice.dir
        );
    }
}

#[test]
fn shapes_sub_ring_child_dispatches_polygon_tool() {
    let mut state = create_test_input_state();
    let layout = open_with_layout(&mut state);

    let (parent_x, parent_y) = point_in_compass_wedge(&layout, CompassDir::E);
    state.update_radial_menu_hover(parent_x, parent_y);

    let children = sub_ring_children(CompassDir::E.index());
    let polygon_idx = children
        .iter()
        .position(|action| *action == Action::SelectRegularPolygonTool)
        .expect("polygon child") as u8;
    let (x, y) =
        point_in_sub_tool_segment(&layout, CompassDir::E.index(), polygon_idx, children.len());
    state.update_radial_menu_hover(x, y);
    state.radial_menu_select_hovered();

    assert!(!state.is_radial_menu_open());
    assert_eq!(state.active_tool(), Tool::RegularPolygon);
}

#[test]
fn notes_sub_ring_child_dispatches_sticky_note_mode() {
    let mut state = create_test_input_state();
    let layout = open_with_layout(&mut state);

    let (parent_x, parent_y) = point_in_compass_wedge(&layout, CompassDir::SW);
    state.update_radial_menu_hover(parent_x, parent_y);

    let children = sub_ring_children(CompassDir::SW.index());
    let sticky_idx = children
        .iter()
        .position(|action| *action == Action::EnterStickyNoteMode)
        .expect("sticky child") as u8;
    let (x, y) =
        point_in_sub_tool_segment(&layout, CompassDir::SW.index(), sticky_idx, children.len());
    state.update_radial_menu_hover(x, y);
    state.radial_menu_select_hovered();

    assert!(!state.is_radial_menu_open());
    assert!(matches!(state.state, DrawingState::TextInput { .. }));
    assert!(matches!(state.text_input_mode, TextInputMode::StickyNote));
}

#[test]
fn compass_ring_offers_no_history_or_clear_actions() {
    let mut ring_actions: Vec<Action> = Vec::new();
    for slice in RADIAL_COMPASS_SLICES.iter() {
        match slice.kind {
            RadialSliceKind::Action(action) => ring_actions.push(action),
            RadialSliceKind::Parent(parent) => ring_actions.extend(parent.children()),
        }
    }
    for action in [Action::Undo, Action::Redo, Action::ClearCanvas] {
        assert!(
            !ring_actions.contains(&action),
            "{action:?} must not live on the compass ring"
        );
    }
}

#[test]
fn radial_hover_collapse_without_hover_change_requests_redraw() {
    let mut state = create_test_input_state();
    let layout = open_with_layout(&mut state);

    // Hover the Shapes parent (compass E) to expand the sub-ring.
    state.needs_redraw = false;
    let (shapes_x, shapes_y) = point_in_compass_wedge(&layout, CompassDir::E);
    state.update_radial_menu_hover(shapes_x, shapes_y);
    assert!(state.needs_redraw);
    assert_eq!(expanded_sub_ring_of(&state), Some(CompassDir::E.index()));

    // Move to sub-ring radius outside parent angle (west side). Hover becomes
    // None, sub-ring stays expanded.
    state.needs_redraw = false;
    let (sub_none_x, sub_none_y) = point_at(layout.center_x, layout.center_y, 110.0, 180.0);
    state.update_radial_menu_hover(sub_none_x, sub_none_y);
    assert!(state.needs_redraw);
    assert_eq!(expanded_sub_ring_of(&state), Some(CompassDir::E.index()));

    // Move into center/tool gap: hover remains None, but expanded sub-ring
    // should collapse. This transition must still request redraw.
    state.needs_redraw = false;
    let (collapse_x, collapse_y) = point_at(layout.center_x, layout.center_y, 35.0, 0.0);
    state.update_radial_menu_hover(collapse_x, collapse_y);
    assert!(state.needs_redraw);
    assert_eq!(expanded_sub_ring_of(&state), None);
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
fn right_click_toggles_radial_when_configured() {
    let mut state = create_test_input_state();
    state.radial_menu_mouse_binding = crate::config::RadialMenuMouseBinding::Right;

    state.on_mouse_press(MouseButton::Right, 200, 150);
    assert!(state.is_radial_menu_open());
    assert!(!state.is_context_menu_open());

    state.on_mouse_press(MouseButton::Right, 200, 150);
    assert!(!state.is_radial_menu_open());
    assert!(!state.is_context_menu_open());
}

#[test]
fn toggle_radial_menu_action_opens_and_closes_menu() {
    let mut state = create_test_input_state();
    state.update_pointer_position(320, 240);

    state.handle_action(Action::ToggleRadialMenu);
    assert!(state.is_radial_menu_open());

    state.handle_action(Action::ToggleRadialMenu);
    assert!(!state.is_radial_menu_open());

    state.state = DrawingState::Selecting {
        start_x: 10,
        start_y: 20,
        additive: false,
    };
    state.handle_action(Action::ToggleRadialMenu);
    assert!(!state.is_radial_menu_open());
}

#[test]
fn toggle_radial_menu_with_modifier_keybinding_closes_when_open() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.ui.toggle_radial_menu = vec!["Ctrl+R".to_string()];
    let mut state = create_test_input_state_with_keybindings(keybindings);
    state.update_pointer_position(320, 240);

    state.on_key_press(Key::Ctrl);
    state.on_key_press(Key::Char('r'));
    assert!(state.is_radial_menu_open());

    state.on_key_release(Key::Ctrl);
    state.on_key_press(Key::Ctrl);
    state.on_key_press(Key::Char('r'));
    assert!(!state.is_radial_menu_open());
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
fn right_click_when_radial_open_on_panned_board_uses_canvas_hit_testing() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    assert!(state.boards.active_frame_mut().set_view_offset(100, 50));
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Line {
        x1: 140,
        y1: 90,
        x2: 180,
        y2: 90,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.open_radial_menu(50.0, 40.0);
    assert!(state.is_radial_menu_open());

    state.on_mouse_press_with_canvas(MouseButton::Right, 50, 40, 150, 90);

    assert!(!state.is_radial_menu_open());
    match &state.context_menu_state {
        ContextMenuState::Open {
            kind,
            shape_ids,
            hovered_shape_id,
            ..
        } => {
            assert_eq!(*kind, ContextMenuKind::Shape);
            assert_eq!(shape_ids.as_slice(), &[shape_id]);
            assert_eq!(*hovered_shape_id, Some(shape_id));
        }
        ContextMenuState::Hidden => panic!("context menu should be open"),
    }
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
    let seg = 2.0 * PI / state.quick_colors.radial_rendered_len() as f64;
    let probe_angle = -PI / 2.0 + seg + 0.04;
    let probe_radius = (layout.color_inner + layout.color_outer) / 2.0;
    let probe_x = layout.center_x + probe_radius * probe_angle.cos();
    let probe_y = layout.center_y + probe_radius * probe_angle.sin();

    state.update_radial_menu_hover(probe_x, probe_y);
    state.radial_menu_select_hovered();

    let expected = state.quick_colors.radial_color_for_index(1).unwrap();
    assert!(colors_approx_eq(&state.current_color, &expected));
}

#[test]
fn color_ring_selection_uses_configured_quick_palette() {
    let mut state = create_test_input_state();
    let configured_green = Color {
        r: 0.12,
        g: 0.34,
        b: 0.56,
        a: 1.0,
    };
    state.set_quick_colors(QuickColorPalette::from_entries(vec![
        QuickColorPaletteEntry {
            label: "First".to_string(),
            color: crate::draw::color::RED,
        },
        QuickColorPaletteEntry {
            label: "Configured".to_string(),
            color: configured_green,
        },
    ]));
    state.open_radial_menu(300.0, 220.0);
    state.update_radial_menu_layout(900, 700);
    let layout = state
        .radial_menu_layout
        .expect("layout should exist for open radial menu");

    let seg = 2.0 * PI / state.quick_colors.radial_rendered_len() as f64;
    let probe_angle = -PI / 2.0 + seg + 0.04;
    let probe_radius = (layout.color_inner + layout.color_outer) / 2.0;
    let probe_x = layout.center_x + probe_radius * probe_angle.cos();
    let probe_y = layout.center_y + probe_radius * probe_angle.sin();

    state.update_radial_menu_hover(probe_x, probe_y);
    state.radial_menu_select_hovered();

    let expected = configured_green;
    assert!(colors_approx_eq(&state.current_color, &expected));
}

fn colors_approx_eq(a: &Color, b: &Color) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
}

// ── Phase B: paint delay, flick commit, size ring, recent colors ──

use std::time::{Duration, Instant};

/// Rewind the open menu's `opened_at` so the paint deadline lies in the past.
fn rewind_radial_open(state: &mut InputState, by: Duration) {
    if let RadialMenuState::Open {
        ref mut opened_at, ..
    } = state.radial_menu_state
    {
        *opened_at = opened_at
            .checked_sub(by)
            .expect("monotonic clock should allow rewinding a test instant");
    }
}

fn hover_of(state: &InputState) -> Option<RadialSegmentId> {
    match &state.radial_menu_state {
        RadialMenuState::Open { hover, .. } => *hover,
        RadialMenuState::Hidden => panic!("radial menu should be open"),
    }
}

fn open_via_right_press(state: &mut InputState) -> RadialMenuLayout {
    state.radial_menu_mouse_binding = crate::config::RadialMenuMouseBinding::Right;
    state.on_mouse_press(MouseButton::Right, 400, 300);
    assert!(state.is_radial_menu_open());
    state.update_radial_menu_layout(800, 600);
    state
        .radial_menu_layout
        .expect("layout should exist for open radial menu")
}

#[test]
fn radial_paint_gate_defers_paint_but_keeps_hit_testing_live() {
    let mut state = create_test_input_state();
    let layout = open_with_layout(&mut state);

    let now = Instant::now();
    assert!(
        !state.radial_menu_mark_painted_if_due(now),
        "menu must not paint before the delay"
    );
    assert!(!state.radial_menu_has_painted());
    let timeout = state
        .radial_menu_paint_timeout(now)
        .expect("an unpainted open menu must schedule a wakeup");
    assert!(timeout <= RADIAL_PAINT_DELAY);

    // Hit-testing is live before the first paint.
    let (x, y) = point_in_compass_wedge(&layout, CompassDir::N);
    state.update_radial_menu_hover(x, y);
    assert_eq!(
        hover_of(&state),
        Some(RadialSegmentId::Tool(CompassDir::N.index()))
    );

    rewind_radial_open(&mut state, RADIAL_PAINT_DELAY + Duration::from_millis(50));
    let later = Instant::now();
    assert!(state.radial_menu_mark_painted_if_due(later));
    assert!(state.radial_menu_has_painted());
    assert_eq!(
        state.radial_menu_paint_timeout(later),
        None,
        "a painted menu needs no more paint wakeups"
    );
}

#[test]
fn tick_radial_menu_paint_requests_redraw_only_at_deadline() {
    let mut state = create_test_input_state();
    let _ = open_with_layout(&mut state);

    state.needs_redraw = false;
    assert!(!state.tick_radial_menu_paint(Instant::now()));
    assert!(!state.needs_redraw, "no redraw before the paint deadline");

    rewind_radial_open(&mut state, RADIAL_PAINT_DELAY + Duration::from_millis(50));
    assert!(state.tick_radial_menu_paint(Instant::now()));
    assert!(
        state.needs_redraw,
        "deadline must request the painting redraw"
    );

    // Once painted, the tick goes quiet again.
    assert!(state.radial_menu_mark_painted_if_due(Instant::now()));
    state.needs_redraw = false;
    assert!(!state.tick_radial_menu_paint(Instant::now()));
    assert!(!state.needs_redraw);
}

#[test]
fn blind_flick_release_commits_wedge_by_direction() {
    let mut state = create_test_input_state();
    let layout = open_via_right_press(&mut state);

    let (x, y) = point_in_compass_wedge(&layout, CompassDir::NE);
    state.on_mouse_motion(x as i32, y as i32);
    assert!(
        !state.radial_menu_has_painted(),
        "the flick happens before the paint deadline"
    );
    state.on_mouse_release(MouseButton::Right, x as i32, y as i32);

    assert!(!state.is_radial_menu_open(), "flick commit closes the menu");
    assert_eq!(state.active_tool(), Tool::Marker);
}

#[test]
fn flick_release_beyond_the_rings_still_commits_by_direction() {
    let mut state = create_test_input_state();
    let layout = open_via_right_press(&mut state);

    // Far past the size ring, straight up: direction alone picks N/Pen.
    let (x, y) = point_at(
        layout.center_x,
        layout.center_y,
        layout.size_outer + 40.0,
        -90.0,
    );
    state.on_mouse_motion(x as i32, y as i32);
    state.on_mouse_release(MouseButton::Right, x as i32, y as i32);

    assert!(!state.is_radial_menu_open());
    assert_eq!(state.active_tool(), Tool::Pen);
}

#[test]
fn flick_release_through_parent_slice_expands_sub_ring_and_stays_open() {
    let mut state = create_test_input_state();
    let layout = open_via_right_press(&mut state);
    let before = state.active_tool();

    let (x, y) = point_in_compass_wedge(&layout, CompassDir::E);
    state.on_mouse_motion(x as i32, y as i32);
    state.on_mouse_release(MouseButton::Right, x as i32, y as i32);

    assert!(
        state.is_radial_menu_open(),
        "parent flick keeps the menu open"
    );
    assert_eq!(expanded_sub_ring_of(&state), Some(CompassDir::E.index()));
    assert_eq!(state.active_tool(), before, "no blind sub-ring commit");
}

#[test]
fn blind_flick_into_sub_band_never_commits_a_sub_child() {
    let mut state = create_test_input_state();
    let layout = open_via_right_press(&mut state);
    let before = state.active_tool();

    // Through the Shapes parent (expands the sub-ring) and onward into the
    // sub-ring band, all before the menu ever painted.
    let (px, py) = point_in_compass_wedge(&layout, CompassDir::E);
    state.on_mouse_motion(px as i32, py as i32);
    let sub_r = (layout.sub_inner + layout.sub_outer) / 2.0;
    let (sx, sy) = point_at(layout.center_x, layout.center_y, sub_r, 0.0);
    state.on_mouse_motion(sx as i32, sy as i32);
    state.on_mouse_release(MouseButton::Right, sx as i32, sy as i32);

    assert!(state.is_radial_menu_open());
    assert_eq!(expanded_sub_ring_of(&state), Some(CompassDir::E.index()));
    assert_eq!(state.active_tool(), before);
}

#[test]
fn sighted_release_over_sub_child_commits_it() {
    let mut state = create_test_input_state();
    let layout = open_via_right_press(&mut state);

    rewind_radial_open(&mut state, RADIAL_PAINT_DELAY + Duration::from_millis(50));
    assert!(state.radial_menu_mark_painted_if_due(Instant::now()));

    let (px, py) = point_in_compass_wedge(&layout, CompassDir::E);
    state.on_mouse_motion(px as i32, py as i32);
    let children = sub_ring_children(CompassDir::E.index());
    let polygon_idx = children
        .iter()
        .position(|action| *action == Action::SelectRegularPolygonTool)
        .expect("polygon child") as u8;
    let (sx, sy) =
        point_in_sub_tool_segment(&layout, CompassDir::E.index(), polygon_idx, children.len());
    state.on_mouse_motion(sx as i32, sy as i32);
    state.on_mouse_release(MouseButton::Right, sx as i32, sy as i32);

    assert!(!state.is_radial_menu_open());
    assert_eq!(state.active_tool(), Tool::RegularPolygon);
}

#[test]
fn flick_release_inside_deadzone_cancels() {
    let mut state = create_test_input_state();
    let layout = open_via_right_press(&mut state);
    let before = state.active_tool();

    let (x, y) = point_in_compass_wedge(&layout, CompassDir::NE);
    state.on_mouse_motion(x as i32, y as i32);
    // Pull back into the center deadzone before releasing.
    state.on_mouse_motion(400, 300);
    state.on_mouse_release(MouseButton::Right, 400, 300);

    assert!(!state.is_radial_menu_open(), "deadzone release cancels");
    assert_eq!(state.active_tool(), before, "cancel must not commit");
}

#[test]
fn toggle_release_without_leaving_deadzone_keeps_menu_open() {
    let mut state = create_test_input_state();
    let _ = open_via_right_press(&mut state);

    // Tiny jitter inside the deadzone must not arm the flick.
    state.on_mouse_motion(405, 302);
    state.on_mouse_release(MouseButton::Right, 405, 302);

    assert!(
        state.is_radial_menu_open(),
        "click-to-open browsing must survive the toggle release"
    );
}

/// Open the menu with a right press near the left screen edge, where the
/// layout clamps the rendered center far away from the press point.
fn open_via_right_press_at_edge(state: &mut InputState, x: i32, y: i32) -> RadialMenuLayout {
    state.radial_menu_mouse_binding = crate::config::RadialMenuMouseBinding::Right;
    state.on_mouse_press(MouseButton::Right, x, y);
    assert!(state.is_radial_menu_open());
    state.update_radial_menu_layout(800, 600);
    let layout = state
        .radial_menu_layout
        .expect("layout should exist for open radial menu");
    assert!(
        layout.center_x - x as f64 > layout.center_radius,
        "edge press must clamp the layout center beyond a deadzone from the press point"
    );
    layout
}

#[test]
fn edge_clamped_jittered_toggle_click_keeps_menu_open() {
    let mut state = create_test_input_state();
    let before = state.active_tool();
    let _ = open_via_right_press_at_edge(&mut state, 10, 300);

    // Physical-click jitter while the toggle button is held: the pointer
    // barely travels, but it sits far from the clamped layout center.
    state.on_mouse_motion(11, 301);
    state.on_mouse_release(MouseButton::Right, 11, 301);

    assert!(
        state.is_radial_menu_open(),
        "a jittered toggle click near the edge must not arm a flick"
    );
    assert_eq!(state.active_tool(), before, "no blind commit");
}

#[test]
fn edge_clamped_slow_click_after_paint_keeps_menu_open() {
    let mut state = create_test_input_state();
    let before = state.active_tool();
    let _ = open_via_right_press_at_edge(&mut state, 100, 300);
    rewind_radial_open(&mut state, RADIAL_PAINT_DELAY + Duration::from_millis(50));
    assert!(state.radial_menu_mark_painted_if_due(Instant::now()));

    state.on_mouse_motion(101, 301);
    state.on_mouse_release(MouseButton::Right, 101, 301);

    assert!(
        state.is_radial_menu_open(),
        "a stationary sighted toggle release must keep browsing open"
    );
    assert_eq!(state.active_tool(), before);
}

#[test]
fn edge_clamped_blind_flick_commits_direction_from_press_point() {
    let mut state = create_test_input_state();
    let _ = open_via_right_press_at_edge(&mut state, 10, 300);

    // Flick NE relative to the press point. Relative to the clamped layout
    // center this motion points west, so anchoring the blind commit there
    // would select the wrong wedge.
    state.on_mouse_motion(60, 250);
    state.on_mouse_release(MouseButton::Right, 60, 250);

    assert!(!state.is_radial_menu_open());
    assert_eq!(state.active_tool(), Tool::Marker, "NE flick selects Marker");
}

#[test]
fn edge_clamped_blind_flick_back_to_press_point_cancels() {
    let mut state = create_test_input_state();
    let before = state.active_tool();
    let _ = open_via_right_press_at_edge(&mut state, 10, 300);

    state.on_mouse_motion(80, 300);
    state.on_mouse_motion(12, 302);
    state.on_mouse_release(MouseButton::Right, 12, 302);

    assert!(
        !state.is_radial_menu_open(),
        "release back at the press point cancels"
    );
    assert_eq!(state.active_tool(), before, "cancel must not commit");
}

#[test]
fn paint_timeout_goes_quiet_once_the_deadline_redraw_is_requested() {
    let mut state = create_test_input_state();
    let _ = open_with_layout(&mut state);
    rewind_radial_open(&mut state, RADIAL_PAINT_DELAY + Duration::from_millis(50));

    state.needs_redraw = false;
    let now = Instant::now();
    assert_eq!(
        state.radial_menu_paint_timeout(now),
        Some(Duration::ZERO),
        "past the deadline with no redraw requested yet, wake immediately"
    );

    assert!(state.tick_radial_menu_paint(now));
    assert!(state.needs_redraw);
    assert_eq!(
        state.radial_menu_paint_timeout(now),
        None,
        "once the painting redraw is requested the deadline stops scheduling zero-timeouts"
    );
}

#[test]
fn size_ring_angle_value_mapping_roundtrips_and_snaps_the_gap() {
    for value in [MIN_STROKE_THICKNESS, 10.0, 25.5, 40.0, MAX_STROKE_THICKNESS] {
        let roundtrip = size_ring_value_for_angle(size_ring_angle_for_value(value));
        assert!(
            (roundtrip - value).abs() < 1e-9,
            "roundtrip drifted for {value}: {roundtrip}"
        );
    }

    // Angles in the bottom gap snap to the nearest end.
    let past_end = SIZE_RING_ARC_START + SIZE_RING_ARC_SPAN + 0.1;
    assert!((size_ring_value_for_angle(past_end) - MAX_STROKE_THICKNESS).abs() < 1e-9);
    let before_start = SIZE_RING_ARC_START - 0.1;
    assert!((size_ring_value_for_angle(before_start) - MIN_STROKE_THICKNESS).abs() < 1e-9);
}

#[test]
fn size_ring_hit_test_covers_arc_but_not_gap() {
    let mut state = create_test_input_state();
    let layout = open_with_layout(&mut state);
    let band_r = (layout.size_inner + layout.size_outer) / 2.0;

    // West is inside the gauge arc.
    let (wx, wy) = point_at(layout.center_x, layout.center_y, band_r, 180.0);
    state.update_radial_menu_hover(wx, wy);
    assert_eq!(hover_of(&state), Some(RadialSegmentId::SizeRing));

    // Straight down is the gauge's bottom gap: inert.
    let (gx, gy) = point_at(layout.center_x, layout.center_y, band_r, 90.0);
    state.update_radial_menu_hover(gx, gy);
    assert_eq!(hover_of(&state), None);
}

#[test]
fn size_ring_drag_adjusts_thickness_live_and_keeps_menu_open() {
    let mut state = create_test_input_state();
    state.radial_menu_mouse_binding = crate::config::RadialMenuMouseBinding::Right;
    state.on_mouse_press(MouseButton::Right, 400, 300);
    state.update_radial_menu_layout(800, 600);
    let layout = state.radial_menu_layout.expect("layout");
    let band_r = (layout.size_inner + layout.size_outer) / 2.0;

    // Press on the gauge at west starts a drag and applies the value.
    let (px, py) = point_at(layout.center_x, layout.center_y, band_r, 180.0);
    let (pxi, pyi) = (px.round() as i32, py.round() as i32);
    state.on_mouse_motion(pxi, pyi);
    state.on_mouse_press(MouseButton::Left, pxi, pyi);
    assert!(state.radial_menu_is_size_dragging());
    let expected = size_ring_value_for_angle(
        (pyi as f64 - layout.center_y).atan2(pxi as f64 - layout.center_x),
    );
    assert!((state.size_for_active_tool() - expected).abs() < 1e-9);

    // Dragging along the arc updates the value live (top = midpoint = 25.5).
    let (tx, ty) = point_at(layout.center_x, layout.center_y, band_r, -90.0);
    let (txi, tyi) = (tx.round() as i32, ty.round() as i32);
    state.on_mouse_motion(txi, tyi);
    let expected_top = size_ring_value_for_angle(
        (tyi as f64 - layout.center_y).atan2(txi as f64 - layout.center_x),
    );
    assert!((state.size_for_active_tool() - expected_top).abs() < 1e-9);

    // Dragging past the max end into the gap clamps to the maximum, even
    // outside the band radius (drag capture).
    let (mx, my) = point_at(layout.center_x, layout.center_y, band_r + 60.0, 80.0);
    state.on_mouse_motion(mx.round() as i32, my.round() as i32);
    assert!((state.size_for_active_tool() - MAX_STROKE_THICKNESS).abs() < 1e-9);

    // Releasing ends the drag but keeps the menu open.
    state.on_mouse_release(MouseButton::Left, mx.round() as i32, my.round() as i32);
    assert!(!state.radial_menu_is_size_dragging());
    assert!(state.is_radial_menu_open());
}

#[test]
fn recent_colors_are_deduped_most_recent_first_and_capped() {
    let mut state = create_test_input_state();
    let color = |r: f64| Color {
        r,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    };
    // Steps of 0.125 are exact binary fractions, so equality is exact.
    for i in 0..8 {
        state.apply_color_from_ui(color(i as f64 * 0.125));
    }
    assert_eq!(state.recent_colors.len(), 6, "recents are capped");
    assert_eq!(state.recent_colors[0], color(0.875), "most recent first");

    // Re-applying an existing color moves it to the front without growing.
    state.apply_color_from_ui(color(0.5));
    assert_eq!(state.recent_colors.len(), 6);
    assert_eq!(state.recent_colors[0], color(0.5));
    assert_eq!(
        state
            .recent_colors
            .iter()
            .filter(|c| **c == color(0.5))
            .count(),
        1
    );
}

#[test]
fn radial_ring_appends_recents_after_quick_palette_without_duplicates() {
    let mut state = create_test_input_state();
    let quick_len = state.quick_colors.radial_rendered_len();
    let unique = Color {
        r: 0.11,
        g: 0.22,
        b: 0.33,
        a: 1.0,
    };
    state.apply_color_from_ui(unique);
    // A recent identical to a quick swatch is filtered from the ring.
    let quick0 = state
        .quick_colors
        .radial_color_for_index(0)
        .expect("quick color 0");
    state.apply_color_from_ui(quick0);

    let swatches = state.radial_ring_swatches();
    assert_eq!(swatches.len(), quick_len + 1);
    assert!(swatches[..quick_len].iter().all(|s| !s.recent));
    assert!(swatches[quick_len].recent);
    assert_eq!(swatches[quick_len].color, unique);
}

#[test]
fn recent_color_segment_applies_through_the_color_path() {
    let mut state = create_test_input_state();
    let unique = Color {
        r: 0.11,
        g: 0.22,
        b: 0.33,
        a: 1.0,
    };
    state.apply_color_from_ui(unique);
    // Move the current color away so applying the recent is observable.
    let quick0 = state
        .quick_colors
        .radial_color_for_index(0)
        .expect("quick color 0");
    state.apply_color_from_ui(quick0);
    let quick_len = state.quick_colors.radial_rendered_len();

    let layout = open_with_layout(&mut state);
    let total = state.radial_ring_swatch_count();
    assert_eq!(total, quick_len + 1);
    let seg = 2.0 * PI / total as f64;
    let probe_angle = -PI / 2.0 + quick_len as f64 * seg;
    let probe_radius = (layout.color_inner + layout.color_outer) / 2.0;
    let probe_x = layout.center_x + probe_radius * probe_angle.cos();
    let probe_y = layout.center_y + probe_radius * probe_angle.sin();

    state.update_radial_menu_hover(probe_x, probe_y);
    assert_eq!(
        hover_of(&state),
        Some(RadialSegmentId::Color(quick_len as u8))
    );
    state.radial_menu_select_hovered();

    assert!(!state.is_radial_menu_open());
    assert!(colors_approx_eq(&state.current_color, &unique));
}
