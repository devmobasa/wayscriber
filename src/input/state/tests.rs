use super::core::{ContextMenuKind, ContextMenuState, MenuCommand};
use super::*;
use crate::config::{Action, BoardConfig};
use crate::draw::{Color, FontDescriptor, Shape, frame::UndoAction};
use crate::input::{BoardMode, ClickHighlightSettings, Key, MouseButton, Tool};
use crate::util;

fn create_test_input_state() -> InputState {
    use crate::config::KeybindingsConfig;

    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings.build_action_map().unwrap();

    InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }, // Red
        3.0,   // thickness
        false, // fill_enabled
        32.0,  // font_size
        FontDescriptor {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        },
        false,                  // text_background_enabled
        20.0,                   // arrow_length
        30.0,                   // arrow_angle
        true,                   // show_status_bar
        BoardConfig::default(), // board_config
        action_map,             // action_map
        usize::MAX,
        ClickHighlightSettings::disabled(),
        0,
        0,
    )
}

#[test]
fn test_adjust_font_size_increase() {
    let mut state = create_test_input_state();
    assert_eq!(state.current_font_size, 32.0);

    state.adjust_font_size(2.0);
    assert_eq!(state.current_font_size, 34.0);
    assert!(state.needs_redraw);
}

#[test]
fn test_adjust_font_size_decrease() {
    let mut state = create_test_input_state();
    assert_eq!(state.current_font_size, 32.0);

    state.adjust_font_size(-2.0);
    assert_eq!(state.current_font_size, 30.0);
    assert!(state.needs_redraw);
}

#[test]
fn test_adjust_font_size_clamp_min() {
    let mut state = create_test_input_state();
    state.current_font_size = 10.0;

    // Try to go below minimum (8.0)
    state.adjust_font_size(-5.0);
    assert_eq!(state.current_font_size, 8.0);
}

#[test]
fn test_adjust_font_size_clamp_max() {
    let mut state = create_test_input_state();
    state.current_font_size = 70.0;

    // Try to go above maximum (72.0)
    state.adjust_font_size(5.0);
    assert_eq!(state.current_font_size, 72.0);
}

#[test]
fn test_adjust_font_size_at_boundaries() {
    let mut state = create_test_input_state();

    // Test at minimum boundary
    state.current_font_size = 8.0;
    state.adjust_font_size(0.0);
    assert_eq!(state.current_font_size, 8.0);

    // Test at maximum boundary
    state.current_font_size = 72.0;
    state.adjust_font_size(0.0);
    assert_eq!(state.current_font_size, 72.0);
}

#[test]
fn test_adjust_font_size_multiple_adjustments() {
    let mut state = create_test_input_state();
    assert_eq!(state.current_font_size, 32.0);

    // Simulate multiple Ctrl+Shift++ presses
    state.adjust_font_size(2.0);
    state.adjust_font_size(2.0);
    state.adjust_font_size(2.0);
    assert_eq!(state.current_font_size, 38.0);

    // Then decrease
    state.adjust_font_size(-2.0);
    state.adjust_font_size(-2.0);
    assert_eq!(state.current_font_size, 34.0);
}

#[test]
fn duplicate_selection_via_action_creates_offset_shape() {
    let mut state = create_test_input_state();
    let original_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![original_id]);
    state.handle_action(Action::DuplicateSelection);

    let frame = state.canvas_set.active_frame();
    assert_eq!(frame.shapes.len(), 2);

    let new_id = frame
        .shapes
        .iter()
        .map(|shape| shape.id)
        .find(|id| *id != original_id)
        .expect("duplicate shape id");
    let original = frame.shape(original_id).unwrap();
    let duplicate = frame.shape(new_id).unwrap();

    match (&original.shape, &duplicate.shape) {
        (Shape::Rect { x: ox, y: oy, .. }, Shape::Rect { x: dx, y: dy, .. }) => {
            assert_eq!(*dx, ox + 12);
            assert_eq!(*dy, oy + 12);
        }
        _ => panic!("Expected rectangles"),
    }
}

#[test]
fn clear_all_removes_shapes_even_when_marked_frozen() {
    let mut state = create_test_input_state();
    state.canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: state.current_color,
        thick: state.current_thickness,
    });

    // Simulate frozen flag being on
    state.set_frozen_active(true);
    assert!(state.frozen_active());

    assert!(state.clear_all());
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 0);
    assert!(state.needs_redraw);
}

#[test]
fn translate_selection_with_undo_moves_shape() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 50,
        y2: 50,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    assert!(state.translate_selection_with_undo(10, -5));

    {
        let frame = state.canvas_set.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        match &shape.shape {
            Shape::Line { x1, y1, x2, y2, .. } => {
                assert_eq!((*x1, *y1, *x2, *y2), (10, -5, 60, 45));
            }
            _ => panic!("Expected line shape"),
        }
    }

    // Undo and ensure shape returns to original coordinates
    if let Some(action) = state.canvas_set.active_frame_mut().undo_last() {
        state.apply_action_side_effects(&action);
    }

    {
        let frame = state.canvas_set.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        match &shape.shape {
            Shape::Line { x1, y1, x2, y2, .. } => {
                assert_eq!((*x1, *y1, *x2, *y2), (0, 0, 50, 50));
            }
            _ => panic!("Expected line shape"),
        }
    }
}

#[test]
fn restore_selection_snapshots_reverts_translation() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Text {
        x: 100,
        y: 100,
        text: "Hello".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
    });

    state.set_selection(vec![shape_id]);
    let snapshots = state.capture_movable_selection_snapshots();
    assert_eq!(snapshots.len(), 1);

    assert!(state.apply_translation_to_selection(20, 30));
    state.restore_selection_from_snapshots(snapshots);

    let frame = state.canvas_set.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { x, y, .. } => {
            assert_eq!((*x, *y), (100, 100));
        }
        _ => panic!("Expected text shape"),
    }
}

#[test]
fn test_text_mode_plain_letters_not_triggering_actions() {
    let mut state = create_test_input_state();

    // Enter text mode
    state.state = DrawingState::TextInput {
        x: 100,
        y: 100,
        buffer: String::new(),
    };

    // Type 'r' - should add to buffer, not change color
    let original_color = state.current_color;
    state.on_key_press(Key::Char('r'));

    // Check that 'r' was added to buffer
    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert_eq!(buffer, "r");
    } else {
        panic!("Should still be in text input mode");
    }

    // Color should NOT have changed
    assert_eq!(state.current_color, original_color);

    // Type more color keys
    state.on_key_press(Key::Char('g'));
    state.on_key_press(Key::Char('b'));
    state.on_key_press(Key::Char('t'));

    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert_eq!(buffer, "rgbt");
    } else {
        panic!("Should still be in text input mode");
    }

    // Color should still not have changed
    assert_eq!(state.current_color, original_color);
}

#[test]
fn test_text_mode_allows_symbol_keys_without_modifiers() {
    let mut state = create_test_input_state();

    state.state = DrawingState::TextInput {
        x: 0,
        y: 0,
        buffer: String::new(),
    };

    for key in ['-', '+', '=', '_', '!', '@', '#', '$'] {
        state.on_key_press(Key::Char(key));
    }

    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert_eq!(buffer, "-+=_!@#$");
    } else {
        panic!("Expected to remain in text input mode");
    }
}

#[test]
fn test_text_mode_ctrl_keys_trigger_actions() {
    let mut state = create_test_input_state();

    // Enter text mode
    state.state = DrawingState::TextInput {
        x: 100,
        y: 100,
        buffer: String::from("test"),
    };

    // Press Ctrl (modifier)
    state.on_key_press(Key::Ctrl);

    // Verify Ctrl is held
    assert!(state.modifiers.ctrl);

    // Press 'Z' while Ctrl is held (Ctrl+Z should undo - a non-Exit action)
    state.on_key_press(Key::Char('Z'));

    // Should still be in text mode (undo works but doesn't exit text mode)
    assert!(matches!(state.state, DrawingState::TextInput { .. }));

    // Now test Ctrl+Q for exit
    state.on_key_press(Key::Char('Q'));

    // Exit action from text mode goes to Idle (cancels text mode)
    assert!(matches!(state.state, DrawingState::Idle));

    // Now that we're in Idle, pressing Ctrl+Q again should exit the app
    state.on_key_press(Key::Char('Q'));
    assert!(state.should_exit);
}

#[test]
fn test_redo_restores_shape_after_undo() {
    let mut state = create_test_input_state();

    {
        let frame = state.canvas_set.active_frame_mut();
        let shape_id = frame.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: state.current_color,
            thick: state.current_thickness,
        });

        let index = frame.find_index(shape_id).unwrap();
        let snapshot = frame.shape(shape_id).unwrap().clone();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, snapshot)],
            },
            state.undo_stack_limit,
        );
    }

    assert_eq!(state.canvas_set.active_frame().shapes.len(), 1);

    state.handle_action(Action::Undo);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 0);

    state.handle_action(Action::Redo);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 1);
}

#[test]
fn test_text_mode_respects_length_cap() {
    let mut state = create_test_input_state();

    state.state = DrawingState::TextInput {
        x: 0,
        y: 0,
        buffer: "a".repeat(10_000),
    };

    state.on_key_press(Key::Char('b'));

    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert_eq!(buffer.len(), 10_000);
        assert!(buffer.ends_with('a'));
    } else {
        panic!("Expected to remain in text input mode");
    }

    // After trimming, adding should work again
    if let DrawingState::TextInput { buffer, .. } = &mut state.state {
        buffer.truncate(9_999);
    }

    state.on_key_press(Key::Char('c'));

    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert!(buffer.ends_with('c'));
        assert_eq!(buffer.len(), 10_000);
    }
}

#[test]
fn test_escape_cancels_active_drawing_only() {
    let mut state = create_test_input_state();
    state.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 0,
        start_y: 0,
        points: vec![(0, 0), (5, 5)],
    };

    state.on_key_press(Key::Escape);

    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.should_exit == false);
}

#[test]
fn test_escape_from_idle_requests_exit() {
    let mut state = create_test_input_state();
    assert!(matches!(state.state, DrawingState::Idle));

    state.on_key_press(Key::Escape);

    assert!(state.should_exit);
}

#[test]
fn test_text_mode_escape_exits() {
    let mut state = create_test_input_state();

    // Enter text mode
    state.state = DrawingState::TextInput {
        x: 100,
        y: 100,
        buffer: String::from("test"),
    };

    // Press Escape (should cancel text input)
    state.on_key_press(Key::Escape);

    // Should have exited text mode without adding text
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(!state.should_exit); // Just cancel, don't exit app
}

#[test]
fn test_text_mode_f10_shows_help() {
    let mut state = create_test_input_state();

    // Enter text mode
    state.state = DrawingState::TextInput {
        x: 100,
        y: 100,
        buffer: String::new(),
    };

    assert!(!state.show_help);

    // Press F10 (should toggle help even in text mode)
    state.on_key_press(Key::F10);

    // Help should be visible
    assert!(state.show_help);

    // Should still be in text mode
    assert!(matches!(state.state, DrawingState::TextInput { .. }));
}

#[test]
fn test_idle_mode_plain_letters_trigger_color_actions() {
    let mut state = create_test_input_state();

    // Should be in Idle mode
    assert!(matches!(state.state, DrawingState::Idle));

    let original_color = state.current_color;

    // Press 'g' for green
    state.on_key_press(Key::Char('g'));

    // Color should have changed
    assert_ne!(state.current_color, original_color);
    assert_eq!(state.current_color, util::key_to_color('g').unwrap());
}

#[test]
fn capture_action_sets_pending_and_clears_modifiers() {
    let mut state = create_test_input_state();
    state.modifiers.ctrl = true;
    state.modifiers.shift = true;
    state.modifiers.alt = true;

    state.handle_action(Action::CaptureClipboardFull);

    assert!(!state.modifiers.ctrl);
    assert!(!state.modifiers.shift);
    assert!(!state.modifiers.alt);

    assert_eq!(
        state.take_pending_capture_action(),
        Some(Action::CaptureClipboardFull)
    );
    assert!(state.take_pending_capture_action().is_none());
}

#[test]
fn board_mode_toggle_restores_previous_color() {
    let mut state = create_test_input_state();
    let initial_color = state.current_color;
    assert_eq!(state.board_mode(), BoardMode::Transparent);

    state.switch_board_mode(BoardMode::Whiteboard);
    assert_eq!(state.board_mode(), BoardMode::Whiteboard);
    assert_eq!(state.board_previous_color, Some(initial_color));
    let expected_pen = BoardMode::Whiteboard
        .default_pen_color(&state.board_config)
        .expect("whiteboard should have default pen");
    assert_eq!(state.current_color, expected_pen);

    state.switch_board_mode(BoardMode::Whiteboard);
    assert_eq!(state.board_mode(), BoardMode::Transparent);
    assert_eq!(state.current_color, initial_color);
    assert!(state.board_previous_color.is_none());
}

#[test]
fn mouse_drag_creates_shapes_for_each_tool() {
    let mut state = create_test_input_state();

    // Pen
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_motion(10, 10);
    state.on_mouse_release(MouseButton::Left, 10, 10);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 1);
    state.clear_selection();

    // Line (Shift)
    state.modifiers.shift = true;
    state.on_mouse_press(MouseButton::Left, 20, 20);
    state.on_mouse_release(MouseButton::Left, 25, 25);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 2);
    state.clear_selection();

    // Rectangle (Ctrl)
    state.modifiers.shift = false;
    state.modifiers.ctrl = true;
    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_release(MouseButton::Left, 45, 45);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 3);
    state.clear_selection();

    // Ellipse (Tab)
    state.modifiers.ctrl = false;
    state.modifiers.tab = true;
    state.on_mouse_press(MouseButton::Left, 60, 60);
    state.on_mouse_release(MouseButton::Left, 64, 64);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 4);
    state.clear_selection();

    // Arrow (Ctrl+Shift)
    state.modifiers.tab = false;
    state.modifiers.ctrl = true;
    state.modifiers.shift = true;
    state.on_mouse_press(MouseButton::Left, 80, 80);
    state.on_mouse_release(MouseButton::Left, 86, 86);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 5);
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
fn highlight_tool_prevents_drawing() {
    let mut state = create_test_input_state();
    assert_eq!(state.active_tool(), Tool::Pen);
    assert!(!state.highlight_tool_active());

    state.handle_action(Action::ToggleHighlightTool);
    assert!(state.highlight_tool_active());
    assert_eq!(state.active_tool(), Tool::Highlight);

    // Enable highlight effect to ensure no shapes are added while clicks happen
    state.handle_action(Action::ToggleClickHighlight);

    let initial_shapes = state.canvas_set.active_frame().shapes.len();
    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_release(MouseButton::Left, 20, 20);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), initial_shapes);
    assert!(matches!(state.state, DrawingState::Idle));

    // Toggle highlight tool off and ensure pen drawing resumes
    state.handle_action(Action::ToggleHighlightTool);
    assert!(!state.highlight_tool_active());
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 5, 5);
    assert_eq!(
        state.canvas_set.active_frame().shapes.len(),
        initial_shapes + 1
    );
}

#[test]
fn sync_highlight_color_marks_dirty_when_pen_color_changes() {
    let mut state = create_test_input_state();
    state.needs_redraw = false;
    state.current_color = Color {
        r: 0.25,
        g: 0.5,
        b: 0.75,
        a: 1.0,
    };
    state.sync_highlight_color();
    assert!(state.needs_redraw);
}

#[test]
fn context_menu_respects_enable_flag() {
    let mut state = create_test_input_state();
    state.set_context_menu_enabled(false);
    state.toggle_context_menu_via_keyboard();
    assert!(!state.is_context_menu_open());

    state.set_context_menu_enabled(true);
    state.toggle_context_menu_via_keyboard();
    assert!(state.is_context_menu_open());
}

#[test]
fn shape_menu_includes_select_this_entry_whenever_hovered() {
    let mut state = create_test_input_state();
    let first = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 40,
        y: 40,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![first, second]);
    state.open_context_menu(
        (0, 0),
        vec![first, second],
        ContextMenuKind::Shape,
        Some(first),
    );

    let entries = state.context_menu_entries();
    assert!(
        entries
            .iter()
            .any(|entry| entry.label == "Select This Shape"),
        "Expected Select This Shape entry to be present for multi-selection"
    );

    state.set_selection(vec![first]);
    state.open_context_menu((0, 0), vec![first], ContextMenuKind::Shape, Some(first));

    let entries_single = state.context_menu_entries();
    assert!(
        entries_single
            .iter()
            .any(|entry| entry.label == "Select This Shape"),
        "Expected Select This Shape entry even for single selection"
    );
}

#[test]
fn undo_all_and_redo_all_process_entire_stack() {
    let mut state = create_test_input_state();
    let frame = state.canvas_set.active_frame_mut();

    // Seed history with two creates
    let first = frame.add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let first_index = frame.find_index(first).unwrap();
    let first_snapshot = frame.shape(first).unwrap().clone();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(first_index, first_snapshot)],
        },
        state.undo_stack_limit,
    );

    let second = frame.add_shape(Shape::Rect {
        x: 20,
        y: 20,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second_index = frame.find_index(second).unwrap();
    let second_snapshot = frame.shape(second).unwrap().clone();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(second_index, second_snapshot)],
        },
        state.undo_stack_limit,
    );

    assert_eq!(state.canvas_set.active_frame().undo_stack_len(), 2);

    state.undo_all_immediate();
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 0);
    assert_eq!(state.canvas_set.active_frame().redo_stack_len(), 2);

    state.redo_all_immediate();
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 2);
    assert_eq!(state.canvas_set.active_frame().undo_stack_len(), 2);
}

#[test]
fn undo_all_with_delay_respects_history() {
    let mut state = create_test_input_state();
    let frame = state.canvas_set.active_frame_mut();

    let id = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let idx = frame.find_index(id).unwrap();
    let snap = frame.shape(id).unwrap().clone();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(idx, snap)],
        },
        state.undo_stack_limit,
    );

    state.undo_all_with_delay(0);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 0);
    assert_eq!(state.canvas_set.active_frame().redo_stack_len(), 1);
}

#[test]
fn redo_all_with_delay_replays_history() {
    let mut state = create_test_input_state();
    let frame = state.canvas_set.active_frame_mut();

    let id = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let idx = frame.find_index(id).unwrap();
    let snap = frame.shape(id).unwrap().clone();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(idx, snap)],
        },
        state.undo_stack_limit,
    );

    state.undo_all_immediate();
    assert_eq!(state.canvas_set.active_frame().redo_stack_len(), 1);

    state.redo_all_with_delay(0);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 1);
    assert_eq!(state.canvas_set.active_frame().undo_stack_len(), 1);
}

#[test]
fn select_this_shape_command_focuses_single_shape() {
    let mut state = create_test_input_state();
    let first = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 40,
        y: 40,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![first, second]);
    state.open_context_menu(
        (10, 10),
        vec![first, second],
        ContextMenuKind::Shape,
        Some(second),
    );

    state.execute_menu_command(MenuCommand::SelectHoveredShape);
    assert_eq!(state.selected_shape_ids(), &[second]);

    assert!(
        matches!(state.context_menu_state, ContextMenuState::Hidden),
        "Context menu should close after selecting hovered shape"
    );
}

#[test]
fn properties_command_opens_panel() {
    let mut state = create_test_input_state();
    let shape_id = {
        let frame = state.canvas_set.active_frame_mut();
        frame.add_shape(Shape::Rect {
            x: 10,
            y: 10,
            w: 40,
            h: 30,
            fill: false,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        })
    };

    state.set_selection(vec![shape_id]);
    state.execute_menu_command(MenuCommand::Properties);
    assert!(state.properties_panel().is_some());
    assert!(!state.is_context_menu_open());
}

#[test]
fn keyboard_context_menu_sets_initial_focus() {
    let mut state = create_test_input_state();
    state.toggle_context_menu_via_keyboard();
    match &state.context_menu_state {
        ContextMenuState::Open { keyboard_focus, .. } => {
            assert!(keyboard_focus.is_some());
        }
        ContextMenuState::Hidden => panic!("Context menu should be open"),
    }
}
