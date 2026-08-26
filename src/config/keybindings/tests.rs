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

/// Dispatch matches key names case-insensitively, so equality and hashing must
/// share that rule — two spellings of one chord as distinct map entries would
/// dodge every conflict check and leave dispatch to pick nondeterministically.
#[test]
fn bindings_differing_only_in_key_case_are_the_same_binding() {
    let lower = KeyBinding::parse("ctrl+z").unwrap();
    let upper = KeyBinding::parse("Ctrl+Z").unwrap();
    assert_eq!(lower, upper);

    let mut map = std::collections::HashMap::new();
    map.insert(lower, ());
    assert!(map.contains_key(&upper));
    assert_eq!(
        upper.to_string(),
        "Ctrl+Z",
        "equality folds case; display keeps the authored spelling"
    );
}

#[test]
fn test_parse_requires_non_modifier_key() {
    let err = KeyBinding::parse("Ctrl+Shift").unwrap_err();
    assert!(err.contains("No key specified"));
    let super_only = KeyBinding::parse("Super").unwrap_err();
    assert!(super_only.contains("No key specified"));
}

#[test]
fn super_aliases_parse_display_equal_and_match_as_super() {
    let canonical = KeyBinding::parse("Super+X").unwrap();
    assert!(canonical.logo);
    assert!(!canonical.ctrl);
    assert_eq!(canonical.to_string(), "Super+X");
    assert!(canonical.matches("x", false, false, false, true));
    assert!(!canonical.matches("x", false, false, false, false));
    assert!(!canonical.matches("x", true, false, false, false));

    for alias in ["Meta+X", "Logo+X", "Win+X", "Windows+X", "meta+x", "WIN+X"] {
        let parsed = KeyBinding::parse(alias).unwrap();
        assert_eq!(parsed, canonical, "{alias} must equal Super+X");
        assert!(
            parsed.to_string().starts_with("Super+"),
            "{alias} displays Super, got {parsed}"
        );
    }

    let shifted = KeyBinding::parse("Shift+Super+F5").unwrap();
    assert_eq!(shifted.to_string(), "Shift+Super+F5");
    assert_eq!(KeyBinding::parse("Super+Shift+F5").unwrap(), shifted);

    let mut map = std::collections::HashMap::new();
    map.insert(canonical.clone(), ());
    assert!(map.contains_key(&KeyBinding::parse("Meta+X").unwrap()));
    assert!(!map.contains_key(&KeyBinding::parse("Ctrl+X").unwrap()));
    assert!(!map.contains_key(&KeyBinding::parse("X").unwrap()));
}

#[test]
fn test_display_normalizes_modifier_order() {
    let binding = KeyBinding::parse("Shift+Ctrl+W").unwrap();
    assert_eq!(binding.to_string(), "Ctrl+Shift+W");
}

#[test]
fn test_matches() {
    let binding = KeyBinding::parse("Ctrl+Shift+W").unwrap();
    assert!(binding.matches("W", true, true, false, false));
    assert!(binding.matches("w", true, true, false, false)); // Case insensitive
    assert!(!binding.matches("W", false, true, false, false)); // Missing ctrl
    assert!(!binding.matches("W", true, false, false, false)); // Missing shift
    assert!(!binding.matches("A", true, true, false, false)); // Wrong key
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
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+1").unwrap()),
        Some(&Action::Exit)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+2").unwrap()),
        Some(&Action::Undo)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+3").unwrap()),
        Some(&Action::Redo)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+4").unwrap()),
        Some(&Action::ToggleHelp)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+5").unwrap()),
        Some(&Action::ToggleWhiteboard)
    );
}

#[test]
fn command_palette_and_full_screen_capture_defaults_are_distinct_and_ordered() {
    let config = KeybindingsConfig::default();
    assert_eq!(config.ui.toggle_command_palette, ["Ctrl+K", "Ctrl+Shift+P"]);
    assert_eq!(config.capture.capture_full_screen, ["Ctrl+Alt+F"]);

    let map = config.build_action_map().expect("default keymap is valid");
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+K").unwrap()),
        Some(&Action::ToggleCommandPalette)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Shift+P").unwrap()),
        Some(&Action::ToggleCommandPalette)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+F").unwrap()),
        Some(&Action::CaptureFullScreen)
    );
}

#[test]
fn toolbar_display_cycle_owns_f2_and_toggle_toolbar_keeps_f9() {
    let config = KeybindingsConfig::default();
    assert_eq!(config.ui.cycle_toolbar_display, ["F2"]);
    assert_eq!(config.ui.toggle_toolbar, ["F9"]);

    // The split leaves no duplicate binding in the defaults.
    let map = config.build_action_map().expect("default keymap is valid");
    assert_eq!(
        map.get(&Shortcut::parse("F2").unwrap()),
        Some(&Action::CycleToolbarDisplay)
    );
    assert_eq!(
        map.get(&Shortcut::parse("F9").unwrap()),
        Some(&Action::ToggleToolbar)
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
fn device_triggers_round_trip_through_the_action_map() {
    let mut config = KeybindingsConfig::default();
    config.core.undo = vec!["MouseBack".to_string(), "Ctrl+StylusSecondary".to_string()];
    let map = config.build_action_map().unwrap();
    assert_eq!(
        map.get(&Shortcut::parse("MouseBack").unwrap()),
        Some(&Action::Undo)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+StylusSecondary").unwrap()),
        Some(&Action::Undo)
    );
}

#[test]
fn primary_mouse_strings_are_rejected_from_the_keymap() {
    let mut config = KeybindingsConfig::default();
    config.core.undo = vec!["MouseLeft".to_string()];
    let err = config.build_action_map().unwrap_err();
    assert!(err.contains("cannot be bound"), "{err}");
}

#[test]
fn sequence_prefix_conflict_is_rejected_and_branching_is_allowed() {
    let prefix = Shortcut::parse("Ctrl+Shift+Alt+K").unwrap();
    let copy = Shortcut::parse("Ctrl+Shift+Alt+K > Ctrl+Shift+Alt+C").unwrap();
    let cut = Shortcut::parse("Ctrl+Shift+Alt+K > Ctrl+Shift+Alt+X").unwrap();

    let mut config = KeybindingsConfig::default();
    config.core.undo = vec![copy.to_string()];
    config.core.redo = vec![prefix.to_string()];
    let err = config.build_action_map().unwrap_err();
    assert!(
        err.contains("Sequence prefix conflict"),
        "strict map should reject a prefix: {err}"
    );

    let mut config = KeybindingsConfig::default();
    config.core.undo = vec![copy.to_string()];
    config.core.redo = vec![cut.to_string()];
    let map = config
        .build_action_map()
        .unwrap_or_else(|err| panic!("branching sequences should be valid: {err}"));
    assert_eq!(map.get(&copy), Some(&Action::Undo));
    assert_eq!(map.get(&cut), Some(&Action::Redo));
}

#[test]
fn collect_binding_conflicts_reports_a_sequence_prefix() {
    let prefix = Shortcut::parse("Ctrl+Shift+Alt+K").unwrap();
    let sequence = Shortcut::parse("Ctrl+Shift+Alt+K > Ctrl+Shift+Alt+C").unwrap();
    let mut config = KeybindingsConfig::default();
    config.core.undo = vec![sequence.to_string()];
    config.core.redo = vec![prefix.to_string()];
    let conflicts = config.collect_binding_conflicts().expect("prefix is data");
    assert!(
        conflicts.iter().any(|conflict| {
            (conflict.binding() == &prefix || conflict.binding() == &sequence)
                && conflict.actions().contains(&Action::Undo)
                && conflict.actions().contains(&Action::Redo)
        }),
        "{conflicts:?}"
    );
}

#[test]
fn collect_binding_conflicts_reports_both_sides_of_a_same_action_prefix() {
    let prefix = Shortcut::parse("Ctrl+Shift+Alt+K").unwrap();
    let sequence = Shortcut::parse("Ctrl+Shift+Alt+K > Ctrl+Shift+Alt+C").unwrap();
    let mut config = KeybindingsConfig::default();
    config.core.undo = vec![prefix.to_string(), sequence.to_string()];
    let conflicts = config
        .collect_binding_conflicts()
        .expect("self-prefix is data");
    assert!(
        conflicts
            .iter()
            .any(|conflict| conflict.binding() == &prefix && conflict.actions() == [Action::Undo]),
        "{conflicts:?}"
    );
    assert!(
        conflicts
            .iter()
            .any(|conflict| conflict.binding() == &sequence && conflict.actions() == [Action::Undo]),
        "{conflicts:?}"
    );
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
    assert!(binding.matches("x", false, false, true, false));
    assert!(!binding.matches("x", false, false, false, false));
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
            Shortcut::parse("Ctrl+Alt+Shift+1").unwrap(),
            Shortcut::parse("Ctrl+Alt+Shift+2").unwrap(),
        ])
    );
    assert_eq!(
        bindings.get(&Action::Redo),
        Some(&vec![
            Shortcut::parse("Ctrl+Alt+Shift+3").unwrap(),
            Shortcut::parse("Ctrl+Alt+Shift+4").unwrap(),
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

#[test]
fn build_action_map_includes_canvas_export_bindings() {
    let mut config = KeybindingsConfig::default();
    config.capture.export_canvas_file = vec!["Ctrl+Alt+Shift+F".to_string()];
    config.capture.export_canvas_clipboard = vec!["Ctrl+Alt+Shift+C".to_string()];
    config.capture.export_canvas_clipboard_and_file = vec!["Ctrl+Alt+Shift+B".to_string()];
    config.capture.export_board_pdf_file = vec!["Ctrl+Alt+Shift+P".to_string()];
    config.capture.export_all_boards_pdf_file = vec!["Ctrl+Alt+Shift+A".to_string()];

    let map = config.build_action_map().unwrap();

    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+F").unwrap()),
        Some(&Action::ExportCanvasFile)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+C").unwrap()),
        Some(&Action::ExportCanvasClipboard)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+B").unwrap()),
        Some(&Action::ExportCanvasClipboardAndFile)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+P").unwrap()),
        Some(&Action::ExportBoardPdfFile)
    );
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+A").unwrap()),
        Some(&Action::ExportAllBoardsPdfFile)
    );
}

#[test]
fn screen_eyedropper_defaults_to_i_and_maps_when_reconfigured() {
    let mut config = KeybindingsConfig::default();
    assert_eq!(config.colors.pick_screen_color, vec!["I".to_string()]);

    let default_map = config.build_action_map().unwrap();
    assert_eq!(
        default_map.get(&Shortcut::parse("I").unwrap()),
        Some(&Action::PickScreenColor)
    );

    config.colors.pick_screen_color = vec!["Ctrl+Alt+Shift+E".to_string()];

    let map = config.build_action_map().unwrap();

    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+E").unwrap()),
        Some(&Action::PickScreenColor)
    );
}

#[test]
fn screen_text_recognition_has_a_chord_and_still_leaves_o_to_orange() {
    let mut config = KeybindingsConfig::default();
    assert_eq!(
        config.capture.copy_text_from_screen,
        vec!["Ctrl+Shift+X".to_string()]
    );

    let default_map = config.build_action_map().unwrap();
    assert_eq!(
        default_map.get(&Shortcut::parse("O").unwrap()),
        Some(&Action::SetColorOrange),
        "the quick colour keeps the letter recognition would have wanted"
    );
    assert_eq!(
        default_map.get(&Shortcut::parse("Ctrl+Shift+X").unwrap()),
        Some(&Action::CopyTextFromScreen)
    );

    config.capture.copy_text_from_screen = vec!["Ctrl+Alt+T".to_string()];
    let map = config.build_action_map().unwrap();
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+T").unwrap()),
        Some(&Action::CopyTextFromScreen)
    );
}

#[test]
fn interactive_region_capture_owns_the_region_chord_and_remaps_when_configured() {
    let mut config = KeybindingsConfig::default();
    assert_eq!(
        config.capture.capture_region_interactive,
        vec!["Ctrl+Shift+C".to_string()]
    );
    assert!(
        config.capture.capture_clipboard_selection.is_empty(),
        "the immediate region copy gives up the chord rather than colliding"
    );
    assert_eq!(
        config
            .build_action_map()
            .unwrap()
            .get(&Shortcut::parse("Ctrl+Shift+C").unwrap()),
        Some(&Action::CaptureRegionInteractive)
    );

    config.capture.capture_region_interactive = vec!["Ctrl+Alt+Shift+I".to_string()];
    let map = config.build_action_map().unwrap();
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+Shift+I").unwrap()),
        Some(&Action::CaptureRegionInteractive)
    );
}

#[test]
fn measure_mode_is_unbound_by_default_and_maps_when_configured() {
    let mut config = KeybindingsConfig::default();
    assert!(config.capture.measure_mode.is_empty());
    assert!(
        !config
            .build_action_map()
            .unwrap()
            .values()
            .any(|action| *action == Action::MeasureMode)
    );

    config.capture.measure_mode = vec!["Ctrl+Alt+M".to_string()];
    let map = config.build_action_map().unwrap();
    assert_eq!(
        map.get(&Shortcut::parse("Ctrl+Alt+M").unwrap()),
        Some(&Action::MeasureMode)
    );
}

#[test]
fn canvas_export_actions_deserialize_from_config_names() {
    #[derive(serde::Deserialize)]
    struct ActionFixture {
        action: Action,
    }

    assert_eq!(
        toml::from_str::<ActionFixture>("action = \"export_canvas_file\"")
            .unwrap()
            .action,
        Action::ExportCanvasFile
    );
    assert_eq!(
        toml::from_str::<ActionFixture>("action = \"export_canvas_clipboard\"")
            .unwrap()
            .action,
        Action::ExportCanvasClipboard
    );
    assert_eq!(
        toml::from_str::<ActionFixture>("action = \"export_canvas_clipboard_and_file\"")
            .unwrap()
            .action,
        Action::ExportCanvasClipboardAndFile
    );
    assert_eq!(
        toml::from_str::<ActionFixture>("action = \"export_board_pdf_file\"")
            .unwrap()
            .action,
        Action::ExportBoardPdfFile
    );
    assert_eq!(
        toml::from_str::<ActionFixture>("action = \"export_all_boards_pdf_file\"")
            .unwrap()
            .action,
        Action::ExportAllBoardsPdfFile
    );
}

/// The shipped defaults must never contend with each other. Omitted actions
/// are offered their defaults one key at a time, in traversal order, and a key
/// an earlier default already took is silently dropped — so a collision in this
/// table would unbind whichever action happens to be visited second.
#[test]
fn default_keybindings_have_no_conflicts() {
    let conflicts = KeybindingsConfig::default()
        .collect_binding_conflicts()
        .expect("every default binding string parses");

    assert!(
        conflicts.is_empty(),
        "shipped defaults collide; a default binding must not be claimed twice: {conflicts:?}"
    );
}

#[test]
fn collect_binding_conflicts_reports_every_collision_in_traversal_order() {
    let mut config = KeybindingsConfig::default();
    config.core.exit = vec!["Ctrl+Alt+Shift+1".to_string()];
    config.core.undo = vec!["Ctrl+Alt+Shift+1".to_string()];
    config.ui.toggle_help = vec!["Ctrl+Alt+Shift+2".to_string()];
    config.capture.capture_selection = vec!["Ctrl+Alt+Shift+2".to_string()];

    let conflicts = config
        .collect_binding_conflicts()
        .expect("valid binding strings");

    assert_eq!(conflicts.len(), 2, "collection does not stop at the first");
    assert_eq!(
        conflicts[0].binding(),
        &Shortcut::parse("Ctrl+Alt+Shift+1").unwrap()
    );
    assert_eq!(conflicts[0].actions(), [Action::Exit, Action::Undo]);
    assert_eq!(
        conflicts[1].actions(),
        [Action::ToggleHelp, Action::CaptureSelection],
        "ui is traversed before capture"
    );
}

#[test]
fn collect_binding_conflicts_reports_a_key_three_actions_claim() {
    let mut config = KeybindingsConfig::default();
    config.core.exit = vec!["Ctrl+Alt+Shift+3".to_string()];
    config.ui.toggle_help = vec!["Ctrl+Alt+Shift+3".to_string()];
    config.zoom.zoom_in = vec!["Ctrl+Alt+Shift+3".to_string()];

    let conflicts = config
        .collect_binding_conflicts()
        .expect("valid binding strings");

    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        conflicts[0].actions(),
        [Action::Exit, Action::ToggleHelp, Action::ZoomIn]
    );
}

#[test]
fn collect_binding_conflicts_reports_one_action_listing_a_key_twice() {
    let mut config = KeybindingsConfig::default();
    config.core.exit = vec![
        "Ctrl+Alt+Shift+4".to_string(),
        "Ctrl+Alt+Shift+4".to_string(),
    ];

    let conflicts = config
        .collect_binding_conflicts()
        .expect("valid binding strings");

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].actions(), [Action::Exit]);
}

#[test]
fn collect_binding_conflicts_still_rejects_an_unparseable_binding() {
    let mut config = KeybindingsConfig::default();
    config.core.exit = vec!["Ctrl+Shift".to_string()];

    assert!(config.collect_binding_conflicts().is_err());
}

#[test]
fn config_key_for_action_matches_the_toml_field_names() {
    assert_eq!(
        KeybindingsConfig::config_key_for_action(Action::CycleToolbarDisplay),
        Some("cycle_toolbar_display")
    );
    assert_eq!(
        KeybindingsConfig::config_key_for_action(Action::CaptureFullScreen),
        Some("capture_full_screen")
    );
    assert_eq!(
        KeybindingsConfig::config_key_for_action(Action::ReplayTour),
        None
    );
}

/// Every shipped default binding, action by action, in the order
/// [`KeybindingsConfig::configurable_actions`] declares them.
///
/// Changing or adding a default binding requires updating this table in the
/// same change, so that the change is deliberate and reviewable. It no longer
/// requires a `CURRENT_CONFIG_REVISION` bump: a default is only ever offered to
/// an action a configuration omits, and only where the key is free, so a
/// newcomer cannot land on a shortcut the file bound to something else (#293,
/// #315). What it does still require is that the new default not collide with
/// another shipped one — `default_keybindings_have_no_conflicts` guards that.
/// Adding an action that starts unbound needs only the new `&[]` row.
const DEFAULT_BINDING_SNAPSHOT: &[(&str, &[&str])] = &[
    ("exit", &["Escape", "Ctrl+Q"]),
    ("enter_text_mode", &["T"]),
    ("enter_sticky_note_mode", &["N"]),
    ("clear_canvas", &["E"]),
    ("undo", &["Ctrl+Z"]),
    ("redo", &["Ctrl+Shift+Z", "Ctrl+Y"]),
    ("undo_all", &[]),
    ("redo_all", &[]),
    ("undo_all_delayed", &[]),
    ("redo_all_delayed", &[]),
    ("duplicate_selection", &["Ctrl+D"]),
    ("copy_selection", &["Ctrl+Alt+C"]),
    ("paste_selection", &["Ctrl+Alt+V"]),
    ("select_all", &["Ctrl+A"]),
    ("move_selection_to_front", &["]"]),
    ("move_selection_to_back", &["["]),
    ("nudge_selection_up", &["ArrowUp"]),
    ("nudge_selection_down", &["ArrowDown"]),
    ("nudge_selection_left", &["ArrowLeft", "Shift+PageUp"]),
    ("nudge_selection_right", &["ArrowRight", "Shift+PageDown"]),
    ("nudge_selection_up_large", &["PageUp"]),
    ("nudge_selection_down_large", &["PageDown"]),
    ("move_selection_to_start", &["Home"]),
    ("move_selection_to_end", &["End"]),
    ("move_selection_to_top", &["Ctrl+Home"]),
    ("move_selection_to_bottom", &["Ctrl+End"]),
    ("delete_selection", &["Delete"]),
    ("increase_thickness", &["+", "="]),
    ("decrease_thickness", &["-", "_"]),
    ("increase_marker_opacity", &["Ctrl+Alt+ArrowUp"]),
    ("decrease_marker_opacity", &["Ctrl+Alt+ArrowDown"]),
    ("select_selection_tool", &["V"]),
    ("select_marker_tool", &["H"]),
    ("select_step_marker_tool", &[]),
    ("select_eraser_tool", &["D"]),
    ("toggle_eraser_mode", &["Ctrl+Shift+E"]),
    ("cycle_font_family", &["Shift+T"]),
    ("open_font_picker", &[]),
    ("increase_pen_smoothing", &[]),
    ("decrease_pen_smoothing", &[]),
    ("cycle_blur_style", &[]),
    ("cycle_arrow_style", &[]),
    ("select_pen_tool", &["F"]),
    ("select_line_tool", &[]),
    ("select_rect_tool", &[]),
    ("select_ellipse_tool", &[]),
    ("select_triangle_tool", &[]),
    ("select_parallelogram_tool", &[]),
    ("select_rhombus_tool", &[]),
    ("select_regular_polygon_tool", &[]),
    ("select_freeform_polygon_tool", &[]),
    ("select_arrow_tool", &[]),
    ("select_blur_tool", &[]),
    ("select_spotlight_tool", &[]),
    ("select_highlight_tool", &[]),
    ("toggle_highlight_tool", &["Ctrl+Alt+H"]),
    ("increase_font_size", &["Ctrl+Shift++", "Ctrl+Shift+="]),
    ("decrease_font_size", &["Ctrl+Shift+-", "Ctrl+Shift+_"]),
    ("reset_arrow_labels", &["Ctrl+Shift+R"]),
    ("reset_step_markers", &[]),
    ("toggle_whiteboard", &["Ctrl+W"]),
    ("toggle_blackboard", &["Ctrl+B"]),
    ("return_to_transparent", &["Ctrl+Shift+T"]),
    ("board_1", &["Ctrl+Shift+1"]),
    ("board_2", &["Ctrl+Shift+2"]),
    ("board_3", &["Ctrl+Shift+3"]),
    ("board_4", &["Ctrl+Shift+4"]),
    ("board_5", &["Ctrl+Shift+5"]),
    ("board_6", &["Ctrl+Shift+6"]),
    ("board_7", &["Ctrl+Shift+7"]),
    ("board_8", &["Ctrl+Shift+8"]),
    ("board_9", &["Ctrl+Shift+9"]),
    ("board_next", &["Ctrl+Shift+ArrowRight"]),
    ("board_prev", &["Ctrl+Shift+ArrowLeft"]),
    ("board_new", &["Ctrl+Shift+N"]),
    ("board_delete", &["Ctrl+Shift+Delete"]),
    ("board_picker", &["Ctrl+Shift+B"]),
    ("board_duplicate", &["Ctrl+Shift+D"]),
    ("focus_next_output", &["Ctrl+Alt+Shift+ArrowRight"]),
    ("focus_prev_output", &["Ctrl+Alt+Shift+ArrowLeft"]),
    ("page_prev", &["Ctrl+Alt+ArrowLeft", "Ctrl+Alt+PageUp"]),
    ("page_next", &["Ctrl+Alt+ArrowRight", "Ctrl+Alt+PageDown"]),
    ("page_new", &["Ctrl+Alt+N"]),
    ("page_duplicate", &["Ctrl+Alt+D"]),
    ("page_delete", &["Ctrl+Alt+Delete"]),
    ("toggle_help", &["F10", "F1"]),
    ("toggle_quick_help", &["Shift+F1"]),
    ("toggle_status_bar", &["F12", "F4"]),
    ("toggle_floating_badge", &[]),
    ("toggle_zoom_chip", &[]),
    ("toggle_focus_mode", &[]),
    ("toggle_click_highlight", &["Ctrl+Shift+H"]),
    ("toggle_input_hud", &["Ctrl+Shift+K"]),
    ("toggle_toolbar", &["F9"]),
    ("cycle_toolbar_display", &["F2"]),
    ("toggle_presenter_mode", &["Ctrl+Shift+M"]),
    ("toggle_light_mode", &["F6"]),
    ("toggle_light_mode_drawing", &[]),
    ("render_profile_next", &[]),
    ("render_profile_previous", &[]),
    ("render_profile_off", &[]),
    ("toggle_fill", &[]),
    ("toggle_radial_menu", &[]),
    ("toggle_selection_properties", &["Ctrl+Alt+P"]),
    ("open_context_menu", &["Shift+F10", "Menu"]),
    ("open_configurator", &["F11"]),
    ("open_about", &[]),
    ("toggle_command_palette", &["Ctrl+K", "Ctrl+Shift+P"]),
    ("set_color_red", &["R"]),
    ("set_color_green", &["G"]),
    ("set_color_blue", &["B"]),
    ("set_color_yellow", &["Y"]),
    ("set_color_orange", &["O"]),
    ("set_color_pink", &["P"]),
    ("set_color_white", &["W"]),
    ("set_color_black", &["K"]),
    ("pick_screen_color", &["I"]),
    ("capture_full_screen", &["Ctrl+Alt+F"]),
    ("capture_active_window", &["Ctrl+Shift+O"]),
    ("capture_selection", &["Ctrl+Shift+I"]),
    ("capture_clipboard_full", &["Ctrl+C"]),
    ("capture_file_full", &["Ctrl+S"]),
    ("capture_clipboard_selection", &[]),
    ("capture_file_selection", &["Ctrl+Shift+S"]),
    ("capture_clipboard_region", &["Ctrl+6"]),
    ("capture_file_region", &["Ctrl+Alt+6"]),
    ("capture_region_interactive", &["Ctrl+Shift+C"]),
    ("measure_mode", &[]),
    ("export_canvas_file", &[]),
    ("export_canvas_clipboard", &[]),
    ("export_canvas_clipboard_and_file", &[]),
    ("export_board_pdf_file", &[]),
    ("export_all_boards_pdf_file", &[]),
    ("open_capture_folder", &["Ctrl+Alt+O"]),
    // `O` stays the orange quick color; recognition takes the free letter.
    ("copy_text_from_screen", &["Ctrl+Shift+X"]),
    ("toggle_frozen_mode", &["Ctrl+Shift+F"]),
    ("zoom_in", &["Ctrl+Alt++", "Ctrl+Alt+="]),
    ("zoom_out", &["Ctrl+Alt+-", "Ctrl+Alt+_"]),
    ("reset_zoom", &["Ctrl+Alt+0"]),
    ("toggle_zoom_lock", &["Ctrl+Alt+L"]),
    ("refresh_zoom_capture", &["Ctrl+Alt+R"]),
    ("apply_preset_1", &["1"]),
    ("apply_preset_2", &["2"]),
    ("apply_preset_3", &["3"]),
    ("apply_preset_4", &["4"]),
    ("apply_preset_5", &["5"]),
    ("save_preset_1", &["Shift+1"]),
    ("save_preset_2", &["Shift+2"]),
    ("save_preset_3", &["Shift+3"]),
    ("save_preset_4", &["Shift+4"]),
    ("save_preset_5", &["Shift+5"]),
    ("clear_preset_1", &["Ctrl+1"]),
    ("clear_preset_2", &["Ctrl+2"]),
    ("clear_preset_3", &["Ctrl+3"]),
    ("clear_preset_4", &["Ctrl+4"]),
    ("clear_preset_5", &["Ctrl+5"]),
];

/// Tripwire for [`DEFAULT_BINDING_SNAPSHOT`]: a default that moves, appears, or
/// disappears fails here until the snapshot records it, so that shipping a
/// different shortcut to every user who never configured one is a decision
/// rather than a side effect.
///
/// The action list comes from the same macro the `[keybindings]` fields do, so
/// a new configurable action shows up here without anyone remembering to add it.
/// The GNOME/Ubuntu branch of the page-navigation defaults, which the snapshot
/// cannot record because it pins the other one.
#[test]
fn gnome_sessions_get_the_unprefixed_page_navigation_defaults() {
    let (prev, next) = crate::test_env::with_env_var(
        crate::env_vars::XDG_CURRENT_DESKTOP_ENV,
        Some(std::ffi::OsStr::new("GNOME")),
        || {
            let defaults = KeybindingsConfig::default();
            (
                defaults.board.page_prev.clone(),
                defaults.board.page_next.clone(),
            )
        },
    );

    assert_eq!(prev, ["Ctrl+ArrowLeft", "Ctrl+PageUp"]);
    assert_eq!(next, ["Ctrl+ArrowRight", "Ctrl+PageDown"]);
}

#[test]
fn default_bindings_match_the_checked_in_snapshot() {
    let defaults = crate::test_env::with_scrubbed_desktop_env(KeybindingsConfig::default);
    let actual = KeybindingsConfig::configurable_actions()
        .iter()
        .map(|action| {
            let key = KeybindingsConfig::config_key_for_action(*action)
                .expect("a configurable action stores its bindings under a config key");
            let bindings = defaults
                .bindings_for_action(*action)
                .expect("a configurable action has a binding list");
            (key, bindings)
        })
        .collect::<Vec<_>>();

    let mut differences = Vec::new();
    for (key, bindings) in &actual {
        match DEFAULT_BINDING_SNAPSHOT
            .iter()
            .find(|(snapshot_key, _)| snapshot_key == key)
        {
            Some((_, snapshot)) if snapshot == bindings => {}
            Some((_, snapshot)) => {
                differences.push(format!(
                    "{key}: snapshot {snapshot:?}, defaults {bindings:?}"
                ));
            }
            None => differences.push(format!("{key}: new action, defaults {bindings:?}")),
        }
    }
    for (key, snapshot) in DEFAULT_BINDING_SNAPSHOT {
        if !actual.iter().any(|(actual_key, _)| actual_key == key) {
            differences.push(format!("{key}: dropped action, snapshot {snapshot:?}"));
        }
    }

    assert!(
        differences.is_empty(),
        "the shipped default keybindings changed. Confirm the new default does not \
         collide with another one, then update DEFAULT_BINDING_SNAPSHOT:\n{}",
        differences.join("\n")
    );
    assert_eq!(
        actual.len(),
        DEFAULT_BINDING_SNAPSHOT.len(),
        "the snapshot lists an action twice"
    );
}

/// The recognized-key vocabulary is only meaningful if it matches what the
/// input layer actually produces, so pin it against that mapping. A new key
/// the backend learns to deliver fails here until `NAMED_KEYS` records it.
#[test]
fn every_name_the_input_layer_produces_is_deliverable() {
    use crate::config::keybindings::is_deliverable_key_name;
    use crate::input::events::Key;

    let keys = [
        Key::Escape,
        Key::Return,
        Key::Backspace,
        Key::Space,
        Key::Menu,
        Key::Delete,
        Key::Home,
        Key::End,
        Key::PageUp,
        Key::PageDown,
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
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
        Key::Char('a'),
        Key::Char('7'),
        Key::Char('/'),
    ];
    for key in keys {
        let name = crate::input::state::key_to_action_label_for_test(key)
            .unwrap_or_else(|| panic!("{key:?} produces no binding name"));
        assert!(
            is_deliverable_key_name(&name),
            "the input layer produces {name:?}, which NAMED_KEYS does not list"
        );
    }
}

#[test]
fn a_misspelled_modifier_is_reported_with_a_suggestion() {
    use crate::config::keybindings::{is_deliverable_key_name, suggest_key_name};

    let parsed = KeyBinding::parse("Ctlr+Z").expect("the string still parses");
    assert!(
        !is_deliverable_key_name(&parsed.key),
        "no key event carries {:?}",
        parsed.key
    );
    assert_eq!(suggest_key_name(&parsed.key).as_deref(), Some("Ctrl+Z"));

    let super_typo = KeyBinding::parse("Supre+X").expect("the string still parses");
    assert_eq!(
        suggest_key_name(&super_typo.key).as_deref(),
        Some("Super+X")
    );
}

#[test]
fn a_near_miss_on_a_named_key_is_reported_with_a_suggestion() {
    use crate::config::keybindings::{is_deliverable_key_name, suggest_key_name};

    for (typo, expected) in [
        ("Escpae", "Escape"),
        ("Retrun", "Return"),
        ("PageUP", "PageUp"),
        ("ArrowLef", "ArrowLeft"),
    ] {
        if typo.eq_ignore_ascii_case(expected) {
            // Case alone is fine: matching ignores it.
            assert!(is_deliverable_key_name(typo));
            continue;
        }
        assert!(!is_deliverable_key_name(typo), "{typo} should be unknown");
        assert_eq!(
            suggest_key_name(typo).as_deref(),
            Some(expected),
            "for {typo}"
        );
    }
}

#[test]
fn ordinary_bindings_are_not_flagged() {
    use crate::config::keybindings::is_deliverable_key_name;

    for binding in [
        "Ctrl+Z",
        "Escape",
        "F10",
        "Ctrl+Shift+P",
        "Super+X",
        "Meta+X",
        "a",
        "/",
        "PageUp",
    ] {
        let parsed = KeyBinding::parse(binding).expect("valid binding");
        assert!(
            is_deliverable_key_name(&parsed.key),
            "{binding} should be recognized"
        );
    }
}
