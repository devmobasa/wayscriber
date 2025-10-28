use super::*;
use crate::config::{Action, BoardConfig};
use crate::draw::{Color, FontDescriptor};
use crate::input::{BoardMode, Key, MouseButton};
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
        3.0,  // thickness
        32.0, // font_size
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

    // Line (Shift)
    state.modifiers.shift = true;
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 5, 5);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 2);

    // Rectangle (Ctrl)
    state.modifiers.shift = false;
    state.modifiers.ctrl = true;
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 5, 5);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 3);

    // Ellipse (Tab)
    state.modifiers.ctrl = false;
    state.modifiers.tab = true;
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 4, 4);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 4);

    // Arrow (Ctrl+Shift)
    state.modifiers.tab = false;
    state.modifiers.ctrl = true;
    state.modifiers.shift = true;
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 6, 6);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 5);
}
