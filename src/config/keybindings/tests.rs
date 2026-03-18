use super::*;

#[test]
fn test_parse_simple_key() {
    let binding = KeyBinding::parse("Escape").unwrap();
    assert_eq!(binding.key, "Escape");
    assert!(!binding.ctrl);
    assert!(!binding.shift);
    assert!(!binding.alt);
}

#[test]
fn test_parse_ctrl_key() {
    let binding = KeyBinding::parse("Ctrl+Z").unwrap();
    assert_eq!(binding.key, "Z");
    assert!(binding.ctrl);
    assert!(!binding.shift);
    assert!(!binding.alt);
}

#[test]
fn test_parse_ctrl_shift_key() {
    let binding = KeyBinding::parse("Ctrl+Shift+W").unwrap();
    assert_eq!(binding.key, "W");
    assert!(binding.ctrl);
    assert!(binding.shift);
    assert!(!binding.alt);
}

#[test]
fn test_parse_all_modifiers() {
    let binding = KeyBinding::parse("Ctrl+Shift+Alt+A").unwrap();
    assert_eq!(binding.key, "A");
    assert!(binding.ctrl);
    assert!(binding.shift);
    assert!(binding.alt);
}

#[test]
fn test_parse_case_insensitive() {
    let binding = KeyBinding::parse("ctrl+shift+w").unwrap();
    assert_eq!(binding.key, "w");
    assert!(binding.ctrl);
    assert!(binding.shift);
}

#[test]
fn test_parse_with_spaces() {
    let binding = KeyBinding::parse("Ctrl + Shift + W").unwrap();
    assert_eq!(binding.key, "W");
    assert!(binding.ctrl);
    assert!(binding.shift);
}

#[test]
fn test_parse_plus_key() {
    let binding = KeyBinding::parse("Ctrl+Shift++").unwrap();
    assert_eq!(binding.key, "+");
    assert!(binding.ctrl);
    assert!(binding.shift);
    assert!(!binding.alt);
}

#[test]
fn test_parse_control_alias() {
    let binding = KeyBinding::parse("Control+Alt+Delete").unwrap();
    assert_eq!(binding.key, "Delete");
    assert!(binding.ctrl);
    assert!(binding.alt);
    assert!(!binding.shift);
}

#[test]
fn test_parse_requires_non_modifier_key() {
    let err = KeyBinding::parse("Ctrl+Shift").unwrap_err();
    assert!(err.contains("No key specified"));
}

#[test]
fn test_display_normalizes_modifier_order() {
    let binding = KeyBinding::parse("Shift+Ctrl+W").unwrap();
    assert_eq!(binding.to_string(), "Ctrl+Shift+W");
}

#[test]
fn test_matches() {
    let binding = KeyBinding::parse("Ctrl+Shift+W").unwrap();
    assert!(binding.matches("W", true, true, false));
    assert!(binding.matches("w", true, true, false)); // Case insensitive
    assert!(!binding.matches("W", false, true, false)); // Missing ctrl
    assert!(!binding.matches("W", true, false, false)); // Missing shift
    assert!(!binding.matches("A", true, true, false)); // Wrong key
}

#[test]
fn test_parse_modifier_order_independence() {
    // Test that modifiers can appear in any order
    let binding1 = KeyBinding::parse("Ctrl+Shift+W").unwrap();
    let binding2 = KeyBinding::parse("Shift+Ctrl+W").unwrap();

    assert_eq!(binding1.key, "W");
    assert_eq!(binding2.key, "W");
    assert_eq!(binding1.ctrl, binding2.ctrl);
    assert_eq!(binding1.shift, binding2.shift);
    assert_eq!(binding1.alt, binding2.alt);
    assert!(binding1.ctrl);
    assert!(binding1.shift);

    // Test three modifiers in different orders
    let binding3 = KeyBinding::parse("Ctrl+Alt+Shift+W").unwrap();
    let binding4 = KeyBinding::parse("Shift+Alt+Ctrl+W").unwrap();
    let binding5 = KeyBinding::parse("Alt+Shift+Ctrl+W").unwrap();

    assert_eq!(binding3.key, "W");
    assert_eq!(binding4.key, "W");
    assert_eq!(binding5.key, "W");
    assert!(binding3.ctrl && binding3.shift && binding3.alt);
    assert!(binding4.ctrl && binding4.shift && binding4.alt);
    assert!(binding5.ctrl && binding5.shift && binding5.alt);
}

#[test]
fn test_build_action_map() {
    let mut config = KeybindingsConfig::default();
    config.core.exit = vec!["Ctrl+Alt+Shift+1".to_string()];
    config.core.undo = vec!["Ctrl+Alt+Shift+2".to_string()];
    config.core.redo = vec!["Ctrl+Alt+Shift+3".to_string()];
    config.ui.toggle_help = vec!["Ctrl+Alt+Shift+4".to_string()];
    config.board.toggle_whiteboard = vec!["Ctrl+Alt+Shift+5".to_string()];
    let map = config.build_action_map().unwrap();

    assert_eq!(
        map.get(&KeyBinding::parse("Ctrl+Alt+Shift+1").unwrap()),
        Some(&Action::Exit)
    );
    assert_eq!(
        map.get(&KeyBinding::parse("Ctrl+Alt+Shift+2").unwrap()),
        Some(&Action::Undo)
    );
    assert_eq!(
        map.get(&KeyBinding::parse("Ctrl+Alt+Shift+3").unwrap()),
        Some(&Action::Redo)
    );
    assert_eq!(
        map.get(&KeyBinding::parse("Ctrl+Alt+Shift+4").unwrap()),
        Some(&Action::ToggleHelp)
    );
    assert_eq!(
        map.get(&KeyBinding::parse("Ctrl+Alt+Shift+5").unwrap()),
        Some(&Action::ToggleWhiteboard)
    );
}

#[test]
fn test_duplicate_keybinding_detection() {
    // Create a config with duplicate keybindings
    let mut config = KeybindingsConfig::default();
    config.core.exit = vec!["Ctrl+Z".to_string()];
    config.core.undo = vec!["Ctrl+Z".to_string()];

    // This should fail with a duplicate error
    let result = config.build_action_map();
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("Duplicate keybinding"));
    assert!(err_msg.contains("Ctrl+Z"));
}

#[test]
fn test_duplicate_with_different_modifier_order() {
    // Even with different modifier orders, these are the same keybinding
    let mut config = KeybindingsConfig::default();
    config.core.exit = vec!["Ctrl+Shift+W".to_string()];
    config.board.toggle_whiteboard = vec!["Shift+Ctrl+W".to_string()];

    // This should fail because they normalize to the same binding
    let result = config.build_action_map();
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("Duplicate keybinding"));
    assert!(err_msg.contains("Shift+Ctrl+W"));
}

#[test]
fn test_parse_plus_key_without_modifiers() {
    let binding = KeyBinding::parse("+").unwrap();
    assert_eq!(binding.key, "+");
    assert!(!binding.ctrl);
    assert!(!binding.shift);
    assert!(!binding.alt);
}

#[test]
fn test_parse_trims_surrounding_whitespace() {
    let binding = KeyBinding::parse("  Escape  ").unwrap();
    assert_eq!(binding.key, "Escape");
    assert!(!binding.ctrl);
    assert!(!binding.shift);
    assert!(!binding.alt);
}

#[test]
fn test_matches_requires_exact_alt_state() {
    let binding = KeyBinding::parse("Alt+X").unwrap();
    assert!(binding.matches("x", false, false, true));
    assert!(!binding.matches("x", false, false, false));
}

#[test]
fn test_build_action_bindings_preserves_declared_binding_order() {
    let mut config = KeybindingsConfig::default();
    config.ui.toggle_help = vec![
        "Ctrl+Alt+Shift+1".to_string(),
        "Ctrl+Alt+Shift+2".to_string(),
    ];
    config.core.redo = vec![
        "Ctrl+Alt+Shift+3".to_string(),
        "Ctrl+Alt+Shift+4".to_string(),
    ];
    let bindings = config.build_action_bindings().unwrap();

    assert_eq!(
        bindings.get(&Action::ToggleHelp),
        Some(&vec![
            KeyBinding::parse("Ctrl+Alt+Shift+1").unwrap(),
            KeyBinding::parse("Ctrl+Alt+Shift+2").unwrap(),
        ])
    );
    assert_eq!(
        bindings.get(&Action::Redo),
        Some(&vec![
            KeyBinding::parse("Ctrl+Alt+Shift+3").unwrap(),
            KeyBinding::parse("Ctrl+Alt+Shift+4").unwrap(),
        ])
    );
}

#[test]
fn test_build_action_bindings_reports_duplicate_keybindings() {
    let mut config = KeybindingsConfig::default();
    config.core.exit = vec!["Ctrl+Z".to_string()];
    config.core.undo = vec!["Ctrl+Z".to_string()];

    let result = config.build_action_bindings();
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("Duplicate keybinding"));
    assert!(err_msg.contains("Ctrl+Z"));
}
