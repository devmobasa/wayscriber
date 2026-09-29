use super::super::*;

#[test]
fn test_text_mode_plain_letters_not_triggering_actions() {
    let mut state = create_test_input_state();

    // Enter text mode
    state.state = DrawingState::text_input(100, 100, String::new());

    // Type 'r' - should add to buffer, not change color
    let original_color = state.style.current_color;
    state.on_key_press(Key::Char('r'));

    // Check that 'r' was added to buffer
    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert_eq!(buffer, "r");
    } else {
        panic!("Should still be in text input mode");
    }

    // Color should NOT have changed
    assert_eq!(state.style.current_color, original_color);

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
    assert_eq!(state.style.current_color, original_color);
}

#[test]
fn test_text_mode_allows_symbol_keys_without_modifiers() {
    let mut state = create_test_input_state();

    state.state = DrawingState::text_input(0, 0, String::new());

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
    state.state = DrawingState::text_input(100, 100, String::from("test"));

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

    state.state = DrawingState::text_input(0, 0, "a".repeat(10_000));

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
    state.state = DrawingState::text_input(100, 100, String::from("test"));

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
    state.state = DrawingState::text_input(100, 100, String::new());

    assert!(!state.help_overlay.visible);

    // Press F10 (should toggle help even in text mode)
    state.on_key_press(Key::F10);

    // Help should be visible
    assert!(state.help_overlay.visible);

    // Should still be in text mode
    assert!(matches!(state.state, DrawingState::TextInput { .. }));
}

const FUNCTION_KEYS: [Key; 12] = [
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
];

fn function_key_label(key: Key) -> String {
    crate::input::state::actions::key_press::bindings::key_to_action_label(key)
        .expect("function keys have binding labels")
}

/// Every default binding on a function key, with the key that triggers it.
fn default_function_key_bindings() -> Vec<(crate::config::KeyBinding, Key, Action)> {
    let map = crate::config::KeybindingsConfig::default()
        .build_action_map()
        .expect("default keybindings build");
    let mut bindings = Vec::new();
    for (shortcut, action) in map {
        let crate::config::Shortcut::Single(crate::config::ShortcutTrigger::Keyboard(binding)) =
            shortcut
        else {
            continue;
        };
        if let Some(key) = FUNCTION_KEYS
            .into_iter()
            .find(|key| function_key_label(*key) == binding.key)
        {
            bindings.push((binding, key, action));
        }
    }
    bindings
}

/// What each default function-key action visibly changes. A new default
/// function-key binding needs a probe here.
fn function_key_action_probe(state: &mut InputState, action: Action) -> String {
    match action {
        Action::ToggleHelp | Action::ToggleQuickHelp => format!(
            "{} {}",
            state.help_overlay.visible, state.help_overlay.quick_mode
        ),
        Action::ToggleStatusBar => state.ui_visibility.show_status_bar.to_string(),
        Action::ToggleToolbar => state.toolbar_visible().to_string(),
        Action::CycleToolbarDisplay => format!("{:?}", state.toolbar_top_display_mode()),
        Action::ToggleLightMode => state.light_mode_active().to_string(),
        Action::OpenContextMenu => state.is_context_menu_open().to_string(),
        Action::OpenConfigurator => format!("{:?}", state.take_pending_backend_action()),
        other => panic!("no probe for the default function-key action {other:?}"),
    }
}

#[test]
fn every_default_function_key_binding_works_while_typing() {
    let bindings = default_function_key_bindings();
    assert!(
        bindings
            .iter()
            .any(|(_, key, action)| *key == Key::F6 && *action == Action::ToggleLightMode),
        "F6 toggles light mode by default"
    );

    for (binding, key, action) in bindings {
        let mut state = create_test_input_state();
        state.compositor_capabilities.layer_shell = true;
        state.state = DrawingState::text_input(100, 100, String::from("draft"));
        let before = function_key_action_probe(&mut state, action);
        state.modifiers.ctrl = binding.ctrl;
        state.modifiers.shift = binding.shift;
        state.modifiers.alt = binding.alt;
        state.modifiers.logo = binding.logo;

        state.on_key_press(key);

        assert_ne!(
            function_key_action_probe(&mut state, action),
            before,
            "{binding:?} did not run {action:?} while typing"
        );
    }
}

#[test]
fn user_bound_function_keys_work_while_typing() {
    for key in FUNCTION_KEYS {
        let label = function_key_label(key);
        let mut keybindings = crate::config::KeybindingsConfig::default();
        for bound in [
            &mut keybindings.ui.toggle_help,
            &mut keybindings.ui.toggle_toolbar,
            &mut keybindings.ui.cycle_toolbar_display,
            &mut keybindings.ui.toggle_light_mode,
            &mut keybindings.ui.open_configurator,
        ] {
            bound.retain(|existing| *existing != label);
        }
        keybindings.ui.toggle_status_bar = vec![label.clone()];
        let mut state = create_test_input_state_with_keybindings(keybindings);
        state.state = DrawingState::text_input(100, 100, String::from("draft"));
        let shown = state.ui_visibility.show_status_bar;

        state.on_key_press(key);

        assert_ne!(
            state.ui_visibility.show_status_bar, shown,
            "{label} bound to the status bar did nothing while typing"
        );
    }
}
