use super::*;
use crate::config::KeyBinding;
use std::collections::HashMap;

#[test]
fn explicit_action_binding_labels_dedup_and_preserve_order() {
    let mut state = create_test_input_state();
    let mut bindings = HashMap::new();
    bindings.insert(
        Action::ToggleHelp,
        vec![
            KeyBinding::parse("Shift+F1").unwrap(),
            KeyBinding::parse("Shift+F1").unwrap(),
            KeyBinding::parse("F10").unwrap(),
        ],
    );
    state.set_action_bindings(bindings);

    assert_eq!(
        state.action_binding_labels(Action::ToggleHelp),
        vec!["Shift+F1".to_string(), "F10".to_string()]
    );
}

#[test]
fn custom_action_bindings_override_fallback_action_map_labels() {
    let mut state = create_test_input_state();
    let mut bindings = HashMap::new();
    bindings.insert(Action::ToggleHelp, vec![KeyBinding::parse("Menu").unwrap()]);
    state.set_action_bindings(bindings);

    assert_eq!(
        state.action_binding_labels(Action::ToggleHelp),
        vec!["Menu".to_string()]
    );
}

#[test]
fn fallback_action_binding_labels_are_sorted_when_explicit_bindings_are_missing() {
    let mut state = create_test_input_state();
    state.set_action_bindings(HashMap::new());

    assert_eq!(
        state.action_binding_labels(Action::ToggleHelp),
        vec!["F1".to_string(), "F10".to_string()]
    );
}

#[test]
fn action_binding_primary_label_prefers_first_explicit_binding() {
    let mut state = create_test_input_state();
    let mut bindings = HashMap::new();
    bindings.insert(
        Action::ToggleStatusBar,
        vec![
            KeyBinding::parse("F4").unwrap(),
            KeyBinding::parse("F12").unwrap(),
        ],
    );
    state.set_action_bindings(bindings);

    assert_eq!(
        state.action_binding_primary_label(Action::ToggleStatusBar),
        Some("F4".to_string())
    );
}

#[test]
fn find_action_respects_exact_modifier_matches() {
    let mut state = create_test_input_state();

    state.modifiers.ctrl = true;
    assert_eq!(state.find_action("z"), Some(Action::Undo));

    state.modifiers.shift = true;
    assert_eq!(state.find_action("z"), Some(Action::Redo));

    state.modifiers.ctrl = false;
    assert_eq!(state.find_action("z"), None);
}

#[test]
fn find_action_treats_super_as_a_distinct_modifier() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.core.exit = vec!["Super+X".to_string()];
    let mut state = create_test_input_state_with_keybindings(keybindings);

    state.modifiers.logo = true;
    assert_eq!(state.find_action("x"), Some(Action::Exit));
    assert_eq!(state.find_action("X"), Some(Action::Exit));

    state.modifiers.logo = false;
    assert_eq!(state.find_action("x"), None);

    state.modifiers.ctrl = true;
    assert_eq!(state.find_action("x"), None);

    state.modifiers.logo = true;
    state.reset_modifiers();
    assert!(!state.modifiers.logo);
    assert!(!state.modifiers.ctrl);
}
