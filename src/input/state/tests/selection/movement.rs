use super::*;
use crate::draw::TextMeasurer;
use crate::input::BOARD_ID_WHITEBOARD;
use crate::input::state::InputTextResources;
use crate::ui_text::UiTextEngine;

#[test]
fn dragging_strokes_follows_the_pointer_in_a_panned_or_zoomed_view() {
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
        let mut state = create_test_input_state();
        state.update_screen_dimensions(800, 600);
        state.switch_board(BOARD_ID_WHITEBOARD);
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
            state
                .boards
                .active_frame()
                .shape(stroke)
                .expect("stroke survives drag")
                .shape,
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
            state
                .boards
                .active_frame()
                .shape(stroke)
                .expect("undo retains stroke")
                .shape,
            original,
            "undo must restore the original stroke in view {origin:?}"
        );
        state.handle_action_with_resources(resources, Action::Redo);
        assert_eq!(
            state
                .boards
                .active_frame()
                .shape(stroke)
                .expect("redo retains stroke")
                .shape,
            expected,
            "redo must restore the moved stroke in view {origin:?}"
        );
    }
}

#[test]
fn selection_edge_actions_and_nudges_use_the_visible_canvas() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for (offset, zoom, viewport) in [
        ((1600, 1200), false, (1600, 1200, 800, 600)),
        ((-1600, -1200), false, (-1600, -1200, 800, 600)),
        ((1600, 1200), true, (1700, 1250, 400, 300)),
    ] {
        let mut state = create_test_input_state();
        state.update_screen_dimensions(800, 600);
        state.switch_board(BOARD_ID_WHITEBOARD);
        state
            .boards
            .active_frame_mut()
            .set_view_offset(offset.0, offset.1);
        if zoom {
            state.set_zoom_status(true, false, 2.0, (100.0, 50.0));
        }
        let (left, top, width, height) = viewport;
        let id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: left + 100,
            y: top + 100,
            w: 40,
            h: 40,
            fill: true,
            fill_color: None,
            color: state.style.current_color,
            thick: 4.0,
        });
        state.set_selection(vec![id]);

        for (edge, nudge, expected_x, expected_y) in [
            (
                Action::MoveSelectionToStart,
                Action::NudgeSelectionLeft,
                left,
                top + 98,
            ),
            (
                Action::MoveSelectionToTop,
                Action::NudgeSelectionUp,
                left,
                top,
            ),
            (
                Action::MoveSelectionToEnd,
                Action::NudgeSelectionRight,
                left + width - 44,
                top,
            ),
            (
                Action::MoveSelectionToBottom,
                Action::NudgeSelectionDown,
                left + width - 44,
                top + height - 44,
            ),
        ] {
            state.handle_action_with_resources(resources, edge);
            state.handle_action_with_resources(resources, nudge);

            let bounds = state
                .selection_bounds_with(&measurer)
                .expect("selection has bounds");
            assert_eq!(
                (bounds.x, bounds.y, bounds.width, bounds.height),
                (expected_x, expected_y, 44, 44),
                "{edge:?} and {nudge:?} must use viewport {viewport:?}"
            );
        }
    }
}

#[test]
fn dragging_a_clipped_selection_does_not_reverse_the_pointer_delta() {
    for (offset, zoom, x, width, press_x) in [
        ((0, 0), true, 250, 300, 300),
        ((0, 0), true, 50, 1000, 100),
        ((1600, 1200), false, 1650, 1000, 1700),
    ] {
        let mut state = create_test_input_state();
        state.update_screen_dimensions(800, 600);
        state.switch_board(BOARD_ID_WHITEBOARD);
        state
            .boards
            .active_frame_mut()
            .set_view_offset(offset.0, offset.1);
        if zoom {
            state.set_zoom_status(true, false, 2.0, (0.0, 0.0));
        }
        state.set_tool_override(Some(Tool::Select));
        let y = offset.1 + 100;
        let id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x,
            y,
            w: width,
            h: 100,
            fill: true,
            fill_color: None,
            color: state.style.current_color,
            thick: 4.0,
        });
        state.set_selection(vec![id]);
        let before = state.selection_bounds().expect("rectangle has bounds");

        state.on_mouse_press(MouseButton::Left, press_x, y + 50);
        state.on_mouse_motion(press_x + 5, y + 50);
        assert_eq!(
            state.selection_bounds(),
            Some(before),
            "an outward drag must stop without snapping a clipped selection"
        );

        state.on_mouse_motion(press_x - 20, y + 50);
        state.on_mouse_release(MouseButton::Left, press_x - 20, y + 50);
        let after = state.selection_bounds().expect("rectangle survives drag");
        assert_eq!(
            (after.x, after.y, after.width, after.height),
            (before.x - 20, before.y, before.width, before.height),
            "an inward drag must follow the pointer without an initial jump"
        );
    }
}

#[test]
fn nudging_away_from_an_extreme_view_offset_does_not_panic_or_jump() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = create_test_input_state();
    state.update_screen_dimensions(800, 600);
    state.switch_board(BOARD_ID_WHITEBOARD);
    state
        .boards
        .active_frame_mut()
        .set_view_offset(i32::MIN, i32::MIN);
    let original = Shape::Rect {
        x: 100,
        y: 100,
        w: 1000,
        h: 1000,
        fill: true,
        fill_color: None,
        color: state.style.current_color,
        thick: 4.0,
    };
    let id = state.boards.active_frame_mut().add_shape(original.clone());
    state.set_selection(vec![id]);

    state.handle_action_with_resources(resources, Action::NudgeSelectionRight);
    state.handle_action_with_resources(resources, Action::NudgeSelectionDown);

    assert_eq!(
        state
            .boards
            .active_frame()
            .shape(id)
            .expect("rectangle remains in frame")
            .shape,
        original,
        "nudging away from the viewport must stop without an overflow or snap"
    );
}

#[test]
fn translation_on_a_panned_board_preserves_unknown_screen_dimensions() {
    let measurer = TextMeasurer::default();

    for (width, height) in [(0, 0), (0, 600), (800, 0)] {
        let mut state = create_test_input_state();
        state.update_screen_dimensions(width, height);
        state.switch_board(BOARD_ID_WHITEBOARD);
        state.boards.active_frame_mut().set_view_offset(1600, 1200);
        let id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 1700,
            y: 1300,
            w: 40,
            h: 40,
            fill: true,
            fill_color: None,
            color: state.style.current_color,
            thick: 4.0,
        });
        state.set_selection(vec![id]);

        assert!(
            state.translate_selection_with_undo_with(&measurer, 20, 30),
            "translation must remain available with screen size {width}x{height}"
        );
        let bounds = state.selection_bounds().expect("rectangle has bounds");
        assert_eq!(
            (bounds.x, bounds.y),
            (1718, 1328),
            "an unknown screen dimension must not become a one-pixel movement limit"
        );
    }
}

#[test]
fn edge_shortcuts_keep_oversized_lines_within_canvas_coordinate_limits() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };

    for offset in [i32::MAX, i32::MAX - 800] {
        let mut state = create_test_input_state();
        state.update_screen_dimensions(800, 600);
        state.switch_board(BOARD_ID_WHITEBOARD);
        state
            .boards
            .active_frame_mut()
            .set_view_offset(offset, offset);
        let line = state.boards.active_frame_mut().add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 1000,
            y2: 1000,
            color: state.style.current_color,
            thick: 4.0,
        });
        state.set_selection(vec![line]);

        state.handle_action_with_resources(resources, Action::MoveSelectionToStart);
        state.handle_action_with_resources(resources, Action::MoveSelectionToTop);

        match state
            .boards
            .active_frame()
            .shape(line)
            .expect("line survives edge moves")
            .shape
        {
            Shape::Line { x1, y1, x2, y2, .. } => {
                assert_eq!(
                    (x1, y1),
                    (i32::MAX - 1002, i32::MAX - 1002),
                    "edge moves must preserve the line's starting point and length"
                );
                assert_eq!(
                    (x2, y2),
                    (i32::MAX - 2, i32::MAX - 2),
                    "edge moves must leave room for the line's stroke padding"
                );
            }
            ref other => panic!("expected the original line, got {other:?}"),
        }
    }
}

#[test]
fn edge_shortcuts_keep_text_and_note_anchors_within_coordinate_limits() {
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
        let mut state = create_test_input_state();
        state.update_screen_dimensions(800, 600);
        state.switch_board(BOARD_ID_WHITEBOARD);
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
        let bounds = state.selection_bounds().expect("text has ink bounds");
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
