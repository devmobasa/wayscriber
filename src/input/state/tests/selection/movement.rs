use super::*;
use crate::config::BoardsConfig;
use crate::draw::{ShapeId, TextMeasurer};
use crate::input::state::InputTextResources;
use crate::input::state::test_support::TestInputStateBuilder;
use crate::input::{BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};
use crate::ui_text::UiTextEngine;
use crate::util::Rect;

fn add_rect(state: &mut InputState, x: i32, y: i32, w: i32, h: i32) -> ShapeId {
    let color = state.style.current_color;
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x,
        y,
        w,
        h,
        fill: true,
        fill_color: None,
        color,
        thick: 4.0,
    })
}

fn add_line(state: &mut InputState, (x1, y1): (i32, i32), (x2, y2): (i32, i32)) -> ShapeId {
    let color = state.style.current_color;
    state.boards.active_frame_mut().add_shape(Shape::Line {
        x1,
        y1,
        x2,
        y2,
        color,
        thick: 4.0,
    })
}

fn shape(state: &InputState, id: ShapeId) -> Shape {
    state
        .boards
        .active_frame()
        .shape(id)
        .expect("the shape stays in the frame")
        .shape
        .clone()
}

fn rect_origin(state: &InputState, id: ShapeId) -> (i32, i32) {
    match shape(state, id) {
        Shape::Rect { x, y, .. } => (x, y),
        other => panic!("expected a rectangle, got {other:?}"),
    }
}

fn line_ends(state: &InputState, id: ShapeId) -> ((i32, i32), (i32, i32)) {
    match shape(state, id) {
        Shape::Line { x1, y1, x2, y2, .. } => ((x1, y1), (x2, y2)),
        other => panic!("expected a line, got {other:?}"),
    }
}

fn painted(state: &InputState) -> Rect {
    state
        .selection_bounds()
        .expect("the selection has painted bounds")
}

fn painted_right(bounds: Rect) -> i64 {
    i64::from(bounds.x) + i64::from(bounds.width)
}

fn whiteboard() -> InputState {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(800, 600);
    state.switch_board(BOARD_ID_WHITEBOARD);
    state
}

/// The two fixed 800x600 canvases: the transparent overlay and a solid board
/// with panning disabled.
fn fixed_canvases() -> Vec<(&'static str, InputState)> {
    let mut overlay = create_test_input_state();
    overlay.update_screen_dimensions(800, 600);
    overlay.switch_board(BOARD_ID_TRANSPARENT);

    let mut solid = TestInputStateBuilder::default()
        .boards_config(BoardsConfig {
            pan_enabled: false,
            ..BoardsConfig::default()
        })
        .build();
    solid.update_screen_dimensions(800, 600);
    solid.switch_board(BOARD_ID_WHITEBOARD);
    assert!(
        !solid.boards.pan_enabled() && !solid.board_is_transparent(),
        "the second fixture must be a solid board with panning disabled"
    );

    vec![
        ("transparent overlay", overlay),
        ("unpannable board", solid),
    ]
}

#[test]
fn crossing_the_click_radius_catches_up_with_the_press() {
    for board in [BOARD_ID_WHITEBOARD, BOARD_ID_TRANSPARENT] {
        let mut state = create_test_input_state();
        state.update_screen_dimensions(800, 600);
        state.switch_board(board);
        state.set_tool_override(Some(Tool::Select));
        let id = add_rect(&mut state, 100, 100, 40, 40);

        state.on_mouse_press(MouseButton::Left, 120, 120);
        state.on_mouse_motion(121, 120);
        state.on_mouse_motion(122, 120);
        state.on_mouse_motion(126, 120);
        state.on_mouse_release(MouseButton::Left, 126, 120);

        assert_eq!(
            rect_origin(&state, id),
            (106, 100),
            "the drag must move by the whole pointer travel since the press on {board}"
        );
        assert_eq!(
            state.boards.active_frame().undo_stack_len(),
            1,
            "one drag must record one undo entry on {board}"
        );
    }
}

#[test]
fn wobble_inside_the_click_radius_does_not_move_the_selection() {
    let mut state = whiteboard();
    state.set_tool_override(Some(Tool::Select));
    let id = add_rect(&mut state, 100, 100, 40, 40);

    state.on_mouse_press(MouseButton::Left, 120, 120);
    state.on_mouse_motion(121, 121);
    state.on_mouse_motion(122, 119);
    state.on_mouse_release(MouseButton::Left, 122, 119);

    assert_eq!(
        rect_origin(&state, id),
        (100, 100),
        "a click must not nudge"
    );
    assert_eq!(
        state.boards.active_frame().undo_stack_len(),
        0,
        "a click must not record a move"
    );
}

#[test]
fn a_blocked_drag_moves_again_as_soon_as_it_reverses() {
    for (canvas, mut state) in fixed_canvases() {
        state.set_tool_override(Some(Tool::Select));
        add_rect(&mut state, 700, 100, 40, 40);

        state.on_mouse_press(MouseButton::Left, 720, 120);
        state.on_mouse_motion(790, 120);
        let at_edge = painted(&state);
        assert_eq!(
            painted_right(at_edge),
            800,
            "the drag must stop at the right edge of the {canvas}"
        );

        state.on_mouse_motion(795, 120);
        assert_eq!(
            painted(&state),
            at_edge,
            "further outward motion must be dropped on the {canvas}"
        );

        state.on_mouse_motion(785, 120);
        assert_eq!(
            painted(&state).x,
            at_edge.x - 10,
            "reversing must move the selection by the reverse travel at once on the {canvas}"
        );

        state.on_mouse_motion(785, 150);
        let after = painted(&state);
        assert_eq!(
            (after.x, after.y),
            (at_edge.x - 10, at_edge.y + 30),
            "motion on the other axis must not jump on the {canvas}"
        );

        state.on_mouse_release(MouseButton::Left, 785, 150);
        assert_eq!(
            state.boards.active_frame().undo_stack_len(),
            1,
            "the whole drag must record one undo entry on the {canvas}"
        );
    }
}

#[test]
fn dragging_on_a_pannable_board_follows_the_pointer_in_any_view() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for (board_offset, zoom, origin) in [
        ((0, 0), None, (0, 0)),
        ((1600, 1200), None, (1600, 1200)),
        ((-1600, -1200), None, (-1600, -1200)),
        ((0, 0), Some((2.0, (100.0, 50.0))), (100, 50)),
        ((1600, 1200), Some((2.0, (100.0, 50.0))), (1700, 1250)),
    ] {
        let mut state = whiteboard();
        state
            .boards
            .active_frame_mut()
            .set_view_offset(board_offset.0, board_offset.1);
        if let Some((scale, offset)) = zoom {
            state.set_zoom_status(true, false, scale, offset);
        }
        state.set_tool_override(Some(Tool::Select));
        let (x, y) = origin;
        let original = Shape::Freehand {
            points: vec![
                (f64::from(x + 100), f64::from(y + 100)),
                (f64::from(x + 120), f64::from(y + 130)),
                (f64::from(x + 140), f64::from(y + 160)),
            ],
            color: state.style.current_color,
            thick: 4.0,
        };
        let stroke = state.boards.active_frame_mut().add_shape(original.clone());

        state.on_mouse_press(MouseButton::Left, x + 80, y + 80);
        state.on_mouse_motion(x + 160, y + 180);
        state.on_mouse_release(MouseButton::Left, x + 160, y + 180);
        assert_eq!(state.selected_shape_ids(), &[stroke], "view {origin:?}");

        state.on_mouse_press(MouseButton::Left, x + 120, y + 130);
        state.on_mouse_motion(x + 160, y + 160);
        state.on_mouse_motion(x + 170, y + 170);
        state.on_mouse_release(MouseButton::Left, x + 170, y + 170);

        let expected = Shape::Freehand {
            points: vec![
                (f64::from(x + 150), f64::from(y + 140)),
                (f64::from(x + 170), f64::from(y + 170)),
                (f64::from(x + 190), f64::from(y + 200)),
            ],
            color: state.style.current_color,
            thick: 4.0,
        };
        assert_eq!(
            shape(&state, stroke),
            expected,
            "the stroke must follow the pointer in view {origin:?}"
        );
        assert_eq!(
            state.boards.active_frame().undo_stack_len(),
            1,
            "one drag must create one undo entry in view {origin:?}"
        );

        state.handle_action_with_resources(resources, Action::Undo);
        assert_eq!(
            shape(&state, stroke),
            original,
            "undo must restore the original stroke in view {origin:?}"
        );
        state.handle_action_with_resources(resources, Action::Redo);
        assert_eq!(
            shape(&state, stroke),
            expected,
            "redo must restore the moved stroke in view {origin:?}"
        );
    }
}

#[test]
fn a_fixed_canvas_never_pulls_back_a_selection_that_crosses_an_edge() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for (canvas, mut state) in fixed_canvases() {
        let id = add_rect(&mut state, 780, 100, 40, 40);
        state.set_selection(vec![id]);

        state.handle_action_with_resources(resources, Action::NudgeSelectionRight);
        assert_eq!(
            rect_origin(&state, id),
            (780, 100),
            "an outward nudge must neither move nor snap a clipped selection on the {canvas}"
        );

        state.handle_action_with_resources(resources, Action::NudgeSelectionLeft);
        assert_eq!(
            rect_origin(&state, id),
            (772, 100),
            "an inward nudge must move by exactly one step on the {canvas}"
        );
    }
}

#[test]
fn a_fixed_canvas_blocks_a_selection_outside_it_only_outward() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for (canvas, mut state) in fixed_canvases() {
        for (origin, outward, inward, moved) in [
            (
                (3000, 100),
                Action::NudgeSelectionRight,
                Action::NudgeSelectionLeft,
                (2992, 100),
            ),
            (
                (-3000, 100),
                Action::NudgeSelectionLeft,
                Action::NudgeSelectionRight,
                (-2992, 100),
            ),
            (
                (100, 3000),
                Action::NudgeSelectionDown,
                Action::NudgeSelectionUp,
                (100, 2992),
            ),
            (
                (100, -3000),
                Action::NudgeSelectionUp,
                Action::NudgeSelectionDown,
                (100, -2992),
            ),
        ] {
            let id = add_rect(&mut state, origin.0, origin.1, 40, 40);
            state.set_selection(vec![id]);
            let undo_depth = state.boards.active_frame().undo_stack_len();

            state.handle_action_with_resources(resources, outward);
            assert_eq!(
                rect_origin(&state, id),
                origin,
                "{outward:?} must not move a selection outside the {canvas}"
            );
            assert_eq!(
                state.boards.active_frame().undo_stack_len(),
                undo_depth,
                "a blocked {outward:?} must not record history on the {canvas}"
            );

            state.handle_action_with_resources(resources, inward);
            assert_eq!(
                rect_origin(&state, id),
                moved,
                "{inward:?} must move one step without jumping toward the {canvas}"
            );
        }
    }
}

#[test]
fn an_oversized_selection_slides_only_while_it_covers_a_fixed_canvas() {
    let measurer = TextMeasurer::default();

    for (canvas, mut state) in fixed_canvases() {
        let id = add_rect(&mut state, -100, 100, 1000, 40);
        state.set_selection(vec![id]);

        assert!(state.translate_selection_with_undo_with(&measurer, 500, 0));
        assert_eq!(
            painted(&state).x,
            0,
            "the left edge must stop at the left edge of the {canvas}"
        );

        assert!(state.translate_selection_with_undo_with(&measurer, -500, 0));
        assert_eq!(
            painted_right(painted(&state)),
            800,
            "the right edge must stop at the right edge of the {canvas}"
        );
    }
}

#[test]
fn zoom_does_not_shrink_a_fixed_canvas() {
    let measurer = TextMeasurer::default();

    for (canvas, mut state) in fixed_canvases() {
        state.set_zoom_status(true, false, 2.0, (0.0, 0.0));
        let id = add_rect(&mut state, 300, 100, 40, 40);
        state.set_selection(vec![id]);

        assert!(state.translate_selection_with_undo_with(&measurer, 1000, 0));
        assert_eq!(
            painted_right(painted(&state)),
            800,
            "at 2x zoom the {canvas} must still end at the screen width, not the visible area"
        );
    }
}

#[test]
fn an_unknown_screen_dimension_leaves_that_axis_of_a_fixed_canvas_free() {
    let measurer = TextMeasurer::default();

    for (width, height, delta, expected) in [
        (0, 600, (5000, 0), (5100, 100)),
        (800, 0, (0, 5000), (100, 5100)),
    ] {
        for (canvas, mut state) in fixed_canvases() {
            state.update_screen_dimensions(width, height);
            let id = add_rect(&mut state, 100, 100, 40, 40);
            state.set_selection(vec![id]);

            assert!(state.translate_selection_with_undo_with(&measurer, delta.0, delta.1));
            assert_eq!(
                rect_origin(&state, id),
                expected,
                "a {width}x{height} {canvas} must not limit its unknown axis"
            );
        }
    }
}

#[test]
fn edge_actions_align_with_the_visible_canvas() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for (board, offset, zoom, viewport) in [
        (
            BOARD_ID_WHITEBOARD,
            (1600, 1200),
            false,
            (1600, 1200, 800, 600),
        ),
        (
            BOARD_ID_WHITEBOARD,
            (-1600, -1200),
            false,
            (-1600, -1200, 800, 600),
        ),
        (
            BOARD_ID_WHITEBOARD,
            (1600, 1200),
            true,
            (1700, 1250, 400, 300),
        ),
        (BOARD_ID_TRANSPARENT, (0, 0), true, (100, 50, 400, 300)),
    ] {
        let mut state = create_test_input_state();
        state.update_screen_dimensions(800, 600);
        state.switch_board(board);
        state
            .boards
            .active_frame_mut()
            .set_view_offset(offset.0, offset.1);
        if zoom {
            state.set_zoom_status(true, false, 2.0, (100.0, 50.0));
        }
        let (left, top, width, height) = viewport;
        let id = add_rect(&mut state, left + 100, top + 100, 40, 40);
        state.set_selection(vec![id]);

        for (action, expected_x, expected_y) in [
            (Action::MoveSelectionToStart, left, top + 98),
            (Action::MoveSelectionToTop, left, top),
            (Action::MoveSelectionToEnd, left + width - 44, top),
            (
                Action::MoveSelectionToBottom,
                left + width - 44,
                top + height - 44,
            ),
        ] {
            state.handle_action_with_resources(resources, action);

            let bounds = painted(&state);
            assert_eq!(
                (bounds.x, bounds.y, bounds.width, bounds.height),
                (expected_x, expected_y, 44, 44),
                "{action:?} must align with viewport {viewport:?} on {board}"
            );
        }
    }
}

#[test]
fn edge_actions_do_not_wall_off_a_pannable_board() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = whiteboard();
    state.boards.active_frame_mut().set_view_offset(1600, 1200);
    let id = add_rect(&mut state, 1700, 1300, 40, 40);
    state.set_selection(vec![id]);

    state.handle_action_with_resources(resources, Action::MoveSelectionToStart);
    state.handle_action_with_resources(resources, Action::NudgeSelectionLeft);

    assert_eq!(
        painted(&state).x,
        1592,
        "a nudge after an edge action must keep moving past the visible edge"
    );
}

#[test]
fn edge_actions_are_unavailable_without_the_screen_dimension() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for (width, height, action) in [
        (0, 600, Action::MoveSelectionToEnd),
        (800, 0, Action::MoveSelectionToBottom),
    ] {
        let mut state = whiteboard();
        state.update_screen_dimensions(width, height);
        let id = add_rect(&mut state, 100, 100, 40, 40);
        state.set_selection(vec![id]);

        state.handle_action_with_resources(resources, action);

        assert_eq!(
            rect_origin(&state, id),
            (100, 100),
            "{action:?} needs the {width}x{height} screen dimension it aligns with"
        );
    }
}

#[test]
fn nudges_on_a_pannable_board_stop_at_the_coordinate_limits() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for (view_x, ends, action, expected) in [
        (
            i32::MAX - 800,
            ((i32::MAX - 50, 100), (i32::MAX - 4, 150)),
            Action::NudgeSelectionRight,
            ((i32::MAX - 48, 100), (i32::MAX - 2, 150)),
        ),
        (
            i32::MIN,
            ((i32::MIN + 4, 100), (i32::MIN + 50, 150)),
            Action::NudgeSelectionLeft,
            ((i32::MIN + 2, 100), (i32::MIN + 48, 150)),
        ),
    ] {
        let mut state = whiteboard();
        state.boards.active_frame_mut().set_view_offset(view_x, 0);
        let id = add_line(&mut state, ends.0, ends.1);
        state.set_selection(vec![id]);

        state.handle_action_with_resources(resources, action);

        // Release builds wrap on overflow, so check the stored coordinates
        // rather than only the absence of a panic.
        assert_eq!(
            line_ends(&state, id),
            expected,
            "{action:?} must shorten the move so the line and its stroke padding stay representable"
        );
    }
}

#[test]
fn nudges_at_an_extreme_view_offset_move_normally() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = whiteboard();
    state
        .boards
        .active_frame_mut()
        .set_view_offset(i32::MIN, i32::MIN);
    let id = add_rect(&mut state, 100, 100, 1000, 1000);
    state.set_selection(vec![id]);

    state.handle_action_with_resources(resources, Action::NudgeSelectionRight);
    state.handle_action_with_resources(resources, Action::NudgeSelectionDown);

    assert_eq!(
        rect_origin(&state, id),
        (108, 108),
        "the camera position must not limit or redirect a valid nudge"
    );
}

#[test]
fn edge_actions_keep_oversized_lines_within_coordinate_limits() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for offset in [i32::MAX, i32::MAX - 800] {
        let mut state = whiteboard();
        state
            .boards
            .active_frame_mut()
            .set_view_offset(offset, offset);
        let line = add_line(&mut state, (0, 0), (1000, 1000));
        state.set_selection(vec![line]);

        state.handle_action_with_resources(resources, Action::MoveSelectionToStart);
        state.handle_action_with_resources(resources, Action::MoveSelectionToTop);

        assert_eq!(
            line_ends(&state, line),
            (
                (i32::MAX - 1002, i32::MAX - 1002),
                (i32::MAX - 2, i32::MAX - 2)
            ),
            "edge moves must keep the line's length and leave room for its stroke padding"
        );
    }
}

#[test]
fn edge_actions_keep_text_and_note_anchors_within_coordinate_limits() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for (offset, text, action, anchor, expected, is_note) in [
        (
            i32::MAX,
            "'",
            Action::MoveSelectionToTop,
            (1000, 1000),
            (1000, i32::MAX),
            false,
        ),
        (
            i32::MIN,
            "  x",
            Action::MoveSelectionToStart,
            (-1000, 1000),
            (i32::MIN, 1000),
            false,
        ),
        (
            i32::MIN,
            "אבג",
            Action::MoveSelectionToStart,
            (-1000, 1000),
            (i32::MIN, 1000),
            true,
        ),
    ] {
        let mut state = whiteboard();
        state
            .boards
            .active_frame_mut()
            .set_view_offset(offset, offset);
        let shape = if is_note {
            Shape::StickyNote {
                x: anchor.0,
                y: anchor.1,
                text: text.to_string(),
                background: state.style.current_color,
                size: 32.0,
                font_descriptor: state.style.font_descriptor.clone(),
                wrap_width: Some(300),
            }
        } else {
            Shape::Text {
                x: anchor.0,
                y: anchor.1,
                text: text.to_string(),
                color: state.style.current_color,
                size: 32.0,
                font_descriptor: state.style.font_descriptor.clone(),
                background_enabled: false,
                wrap_width: None,
            }
        };
        let id = state.boards.active_frame_mut().add_shape(shape);
        state.set_selection(vec![id]);
        let bounds = painted(&state);
        match action {
            Action::MoveSelectionToStart => assert!(
                bounds.x > anchor.0,
                "the horizontal fixture must have its anchor left of the painted bounds"
            ),
            Action::MoveSelectionToTop => assert!(
                bounds.y + bounds.height < anchor.1,
                "the vertical fixture must have its baseline below the painted bounds"
            ),
            other => panic!("unexpected edge action {other:?}"),
        }

        state.handle_action_with_resources(resources, action);

        match state
            .boards
            .active_frame()
            .shape(id)
            .expect("text survives edge move")
            .shape
        {
            Shape::Text { x, y, .. } | Shape::StickyNote { x, y, .. } => assert_eq!(
                (x, y),
                expected,
                "the text anchor must stop at the coordinate boundary"
            ),
            ref other => panic!("expected the original text shape, got {other:?}"),
        }
    }
}

#[test]
fn painted_bounds_past_i32_max_still_allow_inward_moves() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = whiteboard();
    state.set_tool_override(Some(Tool::Select));
    // Screen (0, 0) is canvas (i32::MAX - 800, 0).
    state
        .boards
        .active_frame_mut()
        .set_view_offset(i32::MAX - 800, 0);
    let id = add_line(&mut state, (i32::MAX - 50, 100), (i32::MAX, 150));
    state.set_selection(vec![id]);
    let bounds = state
        .boards
        .active_frame()
        .shape(id)
        .and_then(|drawn| drawn.bounding_box())
        .expect("the line has painted bounds");
    assert!(
        painted_right(bounds) > i64::from(i32::MAX),
        "the fixture's painted bounds must end past i32::MAX, got {bounds:?}"
    );
    assert_eq!(
        state.selection_bounds(),
        None,
        "selection code computes x + width in i32, so these bounds have no Rect"
    );

    // Hovering and clicking the line reach handle hit-testing, and the
    // double-click opens Properties; none of them may overflow.
    let (screen_x, screen_y) = (790, 140);
    let (canvas_x, canvas_y) = (i32::MAX - 10, 140);
    state.on_mouse_motion_with_canvas(screen_x, screen_y, canvas_x, canvas_y);
    for _ in 0..2 {
        state.on_mouse_press_with_canvas(MouseButton::Left, screen_x, screen_y, canvas_x, canvas_y);
        state.on_mouse_release_with_canvas(
            MouseButton::Left,
            screen_x,
            screen_y,
            canvas_x,
            canvas_y,
        );
    }
    assert!(
        state.is_properties_panel_open(),
        "a double-click on the line must open Properties"
    );
    state.close_properties_panel();
    assert_eq!(
        state.selected_shape_ids(),
        &[id],
        "clicking the line must keep it selected"
    );

    state.handle_action_with_resources(resources, Action::NudgeSelectionRight);
    assert_eq!(
        line_ends(&state, id),
        ((i32::MAX - 50, 100), (i32::MAX, 150)),
        "an outward nudge must not move a line already at the limit"
    );

    state.handle_action_with_resources(resources, Action::NudgeSelectionLeft);
    assert_eq!(
        line_ends(&state, id),
        ((i32::MAX - 58, 100), (i32::MAX - 8, 150)),
        "an inward nudge must move by exactly one step"
    );
    assert!(
        state.selection_bounds().is_some(),
        "the selection bounds must return once they fit in i32"
    );
}

#[test]
fn a_selection_without_painted_bounds_keeps_its_coordinate_limits() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let empty_text = |state: &InputState, x: i32| Shape::Text {
        x,
        y: 100,
        text: String::new(),
        color: state.style.current_color,
        size: 32.0,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: false,
        wrap_width: None,
    };

    let mut state = whiteboard();
    let text = empty_text(&state, i32::MAX - 4);
    let id = state.boards.active_frame_mut().add_shape(text);
    state.set_selection(vec![id]);
    assert_eq!(
        state.selection_bounds(),
        None,
        "the fixture must have no painted bounds"
    );

    state.handle_action_with_resources(resources, Action::NudgeSelectionRight);
    match shape(&state, id) {
        Shape::Text { x, y, .. } => assert_eq!(
            (x, y),
            (i32::MAX, 100),
            "without painted bounds the anchor alone must limit a pannable-board nudge"
        ),
        other => panic!("expected text, got {other:?}"),
    }

    for (canvas, mut state) in fixed_canvases() {
        let text = empty_text(&state, 100);
        let id = state.boards.active_frame_mut().add_shape(text);
        state.set_selection(vec![id]);

        state.handle_action_with_resources(resources, Action::NudgeSelectionRight);

        match shape(&state, id) {
            Shape::Text { x, y, .. } => assert_eq!(
                (x, y),
                (100, 100),
                "the {canvas} cannot place a selection without painted bounds, so it must not move it"
            ),
            other => panic!("expected text, got {other:?}"),
        }
        assert_eq!(
            state.boards.active_frame().undo_stack_len(),
            0,
            "a blocked nudge must not record history on the {canvas}"
        );
    }
}

#[test]
fn a_selection_wider_than_a_rect_still_moves_on_a_pannable_board() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = whiteboard();
    let near_min = add_rect(&mut state, i32::MIN + 1000, 100, 40, 40);
    let near_max = add_rect(&mut state, i32::MAX - 1000, 100, 40, 40);
    state.set_selection(vec![near_min, near_max]);
    assert_eq!(
        state.selection_bounds(),
        None,
        "the fixture's union must be too wide for one Rect"
    );

    state.handle_action_with_resources(resources, Action::NudgeSelectionRight);

    assert_eq!(
        (rect_origin(&state, near_min), rect_origin(&state, near_max)),
        ((i32::MIN + 1008, 100), (i32::MAX - 992, 100)),
        "both shapes must move by the requested step"
    );
}

#[test]
fn a_group_moves_by_one_delta_that_locked_shapes_do_not_limit() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = whiteboard();
    let free = add_rect(&mut state, 100, 100, 40, 40);
    let near_limit = add_line(&mut state, (i32::MAX - 50, 100), (i32::MAX - 6, 150));
    let locked = add_line(&mut state, (i32::MAX - 50, 200), (i32::MAX - 1, 250));
    state
        .boards
        .active_frame_mut()
        .shape_mut(locked)
        .expect("locked line")
        .locked = true;
    state.set_selection(vec![free, near_limit, locked]);

    state.handle_action_with_resources(resources, Action::NudgeSelectionRight);

    assert_eq!(
        rect_origin(&state, free),
        (104, 100),
        "the group must share the delta that the line near the limit allows"
    );
    assert_eq!(
        line_ends(&state, near_limit),
        ((i32::MAX - 46, 100), (i32::MAX - 2, 150)),
        "the line near the limit must keep room for its stroke padding"
    );
    assert_eq!(
        line_ends(&state, locked),
        ((i32::MAX - 50, 200), (i32::MAX - 1, 250)),
        "a locked shape must neither move nor limit the group"
    );
}

#[test]
fn edge_actions_reach_a_view_more_than_i32_max_away_in_one_undo_step() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for (offset, origin, action, expected_edge) in [
        (
            (1600, 1200),
            (i32::MIN + 1000, 1300),
            Action::MoveSelectionToStart,
            1600,
        ),
        (
            (-1600, -1200),
            (i32::MAX - 1000, -1100),
            Action::MoveSelectionToEnd,
            -800,
        ),
    ] {
        let mut state = whiteboard();
        state
            .boards
            .active_frame_mut()
            .set_view_offset(offset.0, offset.1);
        let id = add_rect(&mut state, origin.0, origin.1, 40, 40);
        state.set_selection(vec![id]);

        state.handle_action_with_resources(resources, action);

        let bounds = painted(&state);
        let edge = if action == Action::MoveSelectionToStart {
            i64::from(bounds.x)
        } else {
            painted_right(bounds)
        };
        assert_eq!(
            edge,
            i64::from(expected_edge),
            "{action:?} must reach the visible edge from {origin:?}"
        );
        assert_eq!(
            state.boards.active_frame().undo_stack_len(),
            1,
            "{action:?} must record one undo entry"
        );

        state.handle_action_with_resources(resources, Action::Undo);
        assert_eq!(
            rect_origin(&state, id),
            origin,
            "undo must restore the rectangle in one step"
        );
    }
}

#[test]
fn text_moved_to_the_coordinate_limit_survives_deselect_and_undo() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = whiteboard();
    // The visible canvas ends exactly at i32::MAX.
    state
        .boards
        .active_frame_mut()
        .set_view_offset(i32::MAX - 800, 0);
    let text = Shape::Text {
        x: i32::MAX - 1000,
        y: 100,
        text: "Handle".to_string(),
        color: state.style.current_color,
        size: 32.0,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: false,
        wrap_width: None,
    };
    let id = state.boards.active_frame_mut().add_shape(text);
    state.set_selection(vec![id]);

    state.handle_action_with_resources(resources, Action::MoveSelectionToEnd);
    assert_eq!(
        painted_right(painted(&state)),
        i64::from(i32::MAX),
        "the text must reach the visible edge at the coordinate limit"
    );
    assert_eq!(
        state.selected_text_resize_handle_with(&measurer),
        None,
        "a resize handle past i32::MAX cannot be represented"
    );

    state.on_key_press(Key::Escape);
    assert!(!state.has_selection(), "Escape must clear the selection");
    state.handle_action_with_resources(resources, Action::Undo);

    match shape(&state, id) {
        Shape::Text { x, y, .. } => {
            assert_eq!((x, y), (i32::MAX - 1000, 100), "undo must restore the text");
        }
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn edge_actions_align_lines_whose_bounds_were_clipped_at_the_limit() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for (view_x, ends, thick, action, expected_edge) in [
        (
            0,
            ((i32::MAX - 50, 100), (i32::MAX, 150)),
            4.0,
            Action::MoveSelectionToEnd,
            800,
        ),
        (
            0,
            ((i32::MIN, 100), (i32::MIN + 50, 150)),
            4.0,
            Action::MoveSelectionToStart,
            0,
        ),
        // The stroke padding hidden past the limit is wider than one pixel.
        (
            i32::MAX - 800,
            ((i32::MAX - 50, 100), (i32::MAX, 150)),
            20.0,
            Action::MoveSelectionToEnd,
            i64::from(i32::MAX),
        ),
    ] {
        let mut state = whiteboard();
        state.boards.active_frame_mut().set_view_offset(view_x, 0);
        let color = state.style.current_color;
        let id = state.boards.active_frame_mut().add_shape(Shape::Line {
            x1: ends.0.0,
            y1: ends.0.1,
            x2: ends.1.0,
            y2: ends.1.1,
            color,
            thick,
        });
        state.set_selection(vec![id]);

        state.handle_action_with_resources(resources, action);

        let bounds = painted(&state);
        let edge = if action == Action::MoveSelectionToStart {
            i64::from(bounds.x)
        } else {
            painted_right(bounds)
        };
        assert_eq!(
            edge, expected_edge,
            "{action:?} must align the full {thick}px stroke padding with the view, got {bounds:?}"
        );
        assert_eq!(
            state.boards.active_frame().undo_stack_len(),
            1,
            "{action:?} must record one undo entry"
        );
    }
}

#[test]
fn handle_hit_testing_far_from_a_selection_near_i32_min_does_not_overflow() {
    let mut state = whiteboard();
    state.set_tool_override(Some(Tool::Select));
    let id = add_rect(&mut state, i32::MIN + 10, 100, 40, 40);
    state.set_selection(vec![id]);

    state.on_mouse_motion(100, 100);

    assert_eq!(
        state.hit_selection_handle(100, 100),
        None,
        "a pointer i32::MAX away from every handle must not hit one"
    );
}
