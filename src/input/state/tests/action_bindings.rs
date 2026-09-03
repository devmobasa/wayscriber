use super::*;
use crate::config::{PointerButton, Shortcut};
use std::collections::HashMap;

#[test]
fn explicit_action_binding_labels_dedup_and_preserve_order() {
    let mut state = create_test_input_state();
    let mut bindings = HashMap::new();
    bindings.insert(
        Action::ToggleHelp,
        vec![
            Shortcut::parse("Shift+F1").unwrap(),
            Shortcut::parse("Shift+F1").unwrap(),
            Shortcut::parse("F10").unwrap(),
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
    bindings.insert(Action::ToggleHelp, vec![Shortcut::parse("Menu").unwrap()]);
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
            Shortcut::parse("F4").unwrap(),
            Shortcut::parse("F12").unwrap(),
        ],
    );
    state.set_action_bindings(bindings);

    assert_eq!(
        state.action_binding_primary_label(Action::ToggleStatusBar),
        Some("F4".to_string())
    );
}

#[test]
fn sequence_binding_labels_use_then() {
    let mut state = create_test_input_state();
    let mut bindings = HashMap::new();
    bindings.insert(
        Action::ToggleHelp,
        vec![Shortcut::parse("Ctrl+K > Ctrl+C").unwrap()],
    );
    state.set_action_bindings(bindings);
    assert_eq!(
        state.action_binding_labels(Action::ToggleHelp),
        vec!["Ctrl+K then Ctrl+C".to_string()]
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

#[test]
fn pointer_shortcuts_match_current_modifiers_and_ignore_unbound_buttons() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.core.undo = vec!["Ctrl+MouseBack".to_string()];
    keybindings.core.redo = vec!["MouseForward".to_string()];
    let mut state = create_test_input_state_with_keybindings(keybindings);

    state.modifiers.ctrl = true;
    assert_eq!(
        state.find_trigger_action(&state.pointer_trigger(PointerButton::Back)),
        Some(Action::Undo)
    );
    state.modifiers.ctrl = false;
    assert_eq!(
        state.find_trigger_action(&state.pointer_trigger(PointerButton::Back)),
        None
    );
    assert_eq!(
        state.find_trigger_action(&state.pointer_trigger(PointerButton::Forward)),
        Some(Action::Redo)
    );
    assert_eq!(
        state.find_trigger_action(&state.pointer_trigger(PointerButton::Extra(1))),
        None
    );
}

#[test]
fn consumed_pointer_shortcut_buttons_clear_on_focus_loss() {
    let mut state = create_test_input_state();
    state.consume_pointer_shortcut_button(0x113);
    assert!(state.take_consumed_pointer_shortcut_button(0x113));
    assert!(!state.take_consumed_pointer_shortcut_button(0x113));
    state.consume_pointer_shortcut_button(0x113);
    state.reset_modifiers();
    assert!(!state.take_consumed_pointer_shortcut_button(0x113));
}

#[test]
fn keyboard_sequences_dispatch_after_the_last_step() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.ui.toggle_floating_badge = vec!["Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C".to_string()];
    let mut state = create_test_input_state_with_keybindings(keybindings);
    let now = std::time::Instant::now();

    state.modifiers.ctrl = true;
    state.modifiers.alt = true;
    state.modifiers.shift = true;
    assert_eq!(
        state.match_keyboard_chord("k", false, now),
        crate::input::state::core::SequenceMatch::Pending
    );
    assert_eq!(
        state.match_keyboard_chord("c", false, now),
        crate::input::state::core::SequenceMatch::Dispatched(Action::ToggleFloatingBadge)
    );
}

#[test]
fn sequence_timeout_and_focus_loss_clear_pending_without_dispatch() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.ui.toggle_floating_badge = vec!["Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C".to_string()];
    let mut state = create_test_input_state_with_keybindings(keybindings);
    let now = std::time::Instant::now();

    state.modifiers.ctrl = true;
    state.modifiers.alt = true;
    state.modifiers.shift = true;
    assert_eq!(
        state.match_keyboard_chord("k", false, now),
        crate::input::state::core::SequenceMatch::Pending
    );

    assert!(state.expire_pending_sequence(now + std::time::Duration::from_secs(1)));
    assert_eq!(
        state.match_keyboard_chord("c", false, now + std::time::Duration::from_secs(1)),
        crate::input::state::core::SequenceMatch::None
    );

    assert_eq!(
        state.match_keyboard_chord("k", false, now),
        crate::input::state::core::SequenceMatch::Pending
    );
    state.reset_modifiers();
    state.modifiers.ctrl = true;
    state.modifiers.alt = true;
    state.modifiers.shift = true;
    assert_eq!(
        state.match_keyboard_chord("c", false, now),
        crate::input::state::core::SequenceMatch::None
    );
}

#[test]
fn keymap_reload_and_modal_open_clear_pending_sequences() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.ui.toggle_floating_badge = vec!["Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C".to_string()];
    let mut state = create_test_input_state_with_keybindings(keybindings.clone());
    let now = std::time::Instant::now();
    state.modifiers.ctrl = true;
    state.modifiers.alt = true;
    state.modifiers.shift = true;
    assert_eq!(
        state.match_keyboard_chord("k", false, now),
        crate::input::state::core::SequenceMatch::Pending
    );

    let action_map = keybindings.build_action_map().unwrap();
    let action_bindings = keybindings.build_action_bindings().unwrap();
    state.set_keybinding_maps(action_map, action_bindings);
    assert_eq!(
        state.match_keyboard_chord("c", false, now),
        crate::input::state::core::SequenceMatch::None
    );

    assert_eq!(
        state.match_keyboard_chord("k", false, now),
        crate::input::state::core::SequenceMatch::Pending
    );
    state.close_modals_for_open(crate::input::state::core::modal::ModalSurface::HelpOverlay);
    assert_eq!(
        state.match_keyboard_chord("c", false, now),
        crate::input::state::core::SequenceMatch::None
    );
}

#[test]
fn on_key_press_completes_a_sequence_and_repeat_does_not() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.ui.toggle_floating_badge = vec!["Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C".to_string()];
    let mut state = create_test_input_state_with_keybindings(keybindings);
    assert!(state.ui_visibility.show_floating_badge);

    state.modifiers.ctrl = true;
    state.modifiers.alt = true;
    state.modifiers.shift = true;
    state.on_key_press(crate::input::Key::Char('k'));
    assert!(state.ui_visibility.show_floating_badge);

    state.on_key_repeat(crate::input::Key::Char('c'));
    assert!(state.ui_visibility.show_floating_badge);

    state.on_key_press(crate::input::Key::Char('c'));
    assert!(!state.ui_visibility.show_floating_badge);
}

#[test]
fn shifted_punctuation_fallback_completes_a_pending_sequence() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.ui.toggle_floating_badge = vec!["Ctrl+Alt+Shift+K > Shift+/".to_string()];
    let mut state = create_test_input_state_with_keybindings(keybindings);
    assert!(state.ui_visibility.show_floating_badge);

    state.modifiers.ctrl = true;
    state.modifiers.alt = true;
    state.modifiers.shift = true;
    state.on_key_press(crate::input::Key::Char('k'));
    assert!(state.ui_visibility.show_floating_badge);

    state.modifiers.ctrl = false;
    state.modifiers.alt = false;
    state.modifiers.shift = true;
    state.on_key_press(crate::input::Key::Char('?'));
    assert!(!state.ui_visibility.show_floating_badge);
}
