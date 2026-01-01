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
    let config = KeybindingsConfig::default();
    let map = config.build_action_map().unwrap();

    // Check that some default bindings are present
    let escape = KeyBinding::parse("Escape").unwrap();
    assert_eq!(map.get(&escape), Some(&Action::Exit));

    let ctrl_z = KeyBinding::parse("Ctrl+Z").unwrap();
    assert_eq!(map.get(&ctrl_z), Some(&Action::Undo));

    let ctrl_shift_z = KeyBinding::parse("Ctrl+Shift+Z").unwrap();
    assert_eq!(map.get(&ctrl_shift_z), Some(&Action::Redo));

    let move_front = KeyBinding::parse("]").unwrap();
    assert_eq!(map.get(&move_front), Some(&Action::MoveSelectionToFront));

    let move_back = KeyBinding::parse("[").unwrap();
    assert_eq!(map.get(&move_back), Some(&Action::MoveSelectionToBack));

    let copy_selection = KeyBinding::parse("Ctrl+Alt+C").unwrap();
    assert_eq!(map.get(&copy_selection), Some(&Action::CopySelection));

    let capture_selection = KeyBinding::parse("Ctrl+Shift+C").unwrap();
    assert_eq!(
        map.get(&capture_selection),
        Some(&Action::CaptureClipboardSelection)
    );

    let select_all = KeyBinding::parse("Ctrl+A").unwrap();
    assert_eq!(map.get(&select_all), Some(&Action::SelectAll));

    let toggle_highlight = KeyBinding::parse("Ctrl+Shift+H").unwrap();
    assert_eq!(
        map.get(&toggle_highlight),
        Some(&Action::ToggleClickHighlight)
    );

    let toggle_highlight_tool = KeyBinding::parse("Ctrl+Alt+H").unwrap();
    assert_eq!(
        map.get(&toggle_highlight_tool),
        Some(&Action::ToggleHighlightTool)
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
