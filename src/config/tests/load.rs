use super::super::*;
use super::save_through_document;
use crate::config::test_helpers::with_temp_config_home;
use std::fs;

#[test]
fn load_prefers_primary_directory() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        fs::write(
            primary_dir.join("config.toml"),
            "[drawing]\ndefault_color = 'red'\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds");
        assert!(matches!(loaded.source, ConfigSource::Primary));
    });
}

#[test]
fn load_parses_xdg_focus_loss_behavior_stay() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        fs::write(
            primary_dir.join("config.toml"),
            "[ui]\nxdg_focus_loss_behavior = 'stay'\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds");
        assert_eq!(
            loaded.config.ui.xdg_focus_loss_behavior,
            XdgFocusLossBehavior::Stay
        );
    });
}

#[test]
fn ui_theme_defaults_to_auto_and_parses_explicit_values() {
    let default_config: Config = toml::from_str("").expect("empty config should use defaults");
    assert_eq!(default_config.ui.theme, UiTheme::Auto);

    for (value, expected) in [
        ("auto", UiTheme::Auto),
        ("dark", UiTheme::Dark),
        ("light", UiTheme::Light),
    ] {
        let config: Config = toml::from_str(&format!("[ui]\ntheme = '{value}'\n"))
            .expect("supported ui theme should parse");
        assert_eq!(config.ui.theme, expected);
    }
}

#[test]
fn presenter_toolbar_mode_defaults_to_hidden_and_round_trips() {
    let default_config: Config = toml::from_str("").expect("empty config should use defaults");
    assert_eq!(
        default_config.presenter_mode.toolbar_mode,
        crate::config::PresenterToolbarMode::Hidden
    );

    for (value, expected) in [
        ("hidden", crate::config::PresenterToolbarMode::Hidden),
        ("micro", crate::config::PresenterToolbarMode::Micro),
    ] {
        let config: Config =
            toml::from_str(&format!("[presenter_mode]\ntoolbar_mode = '{value}'\n"))
                .expect("supported presenter toolbar mode should parse");
        assert_eq!(config.presenter_mode.toolbar_mode, expected);

        let serialized = toml::to_string(&config).expect("config serializes");
        let reloaded: Config = toml::from_str(&serialized).expect("round trip parses");
        assert_eq!(reloaded.presenter_mode.toolbar_mode, expected);
    }

    let error = toml::from_str::<Config>("[presenter_mode]\ntoolbar_mode = 'tiny'\n")
        .expect_err("unknown presenter toolbar mode should fail");
    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn toolbar_top_display_mode_defaults_to_full_and_round_trips() {
    use crate::config::TopDisplayMode;

    let default_config: Config = toml::from_str("").expect("empty config should use defaults");
    assert_eq!(
        default_config.ui.toolbar.top_display_mode,
        TopDisplayMode::Full
    );

    for (value, expected) in [
        ("full", TopDisplayMode::Full),
        ("micro", TopDisplayMode::Micro),
        ("hidden", TopDisplayMode::Hidden),
    ] {
        let config: Config =
            toml::from_str(&format!("[ui.toolbar]\ntop_display_mode = '{value}'\n"))
                .expect("supported top display mode should parse");
        assert_eq!(config.ui.toolbar.top_display_mode, expected);
    }

    // Hidden never persists: startup is governed by `top_pinned` (the pins
    // the F9 toggle records durably), so the persisted form collapses to full.
    assert_eq!(TopDisplayMode::Hidden.persisted(), TopDisplayMode::Full);
    assert_eq!(TopDisplayMode::Micro.persisted(), TopDisplayMode::Micro);
}

#[test]
fn toolbar_side_layout_defaults_to_pill_and_round_trips() {
    use crate::config::ToolbarSideLayout;

    // Pill became the default once the Session/Settings panes were
    // re-hosted as top-strip overflow popovers (M4-B3); the classic panel
    // remains a deprecated escape hatch.
    let default_config: Config = toml::from_str("").expect("empty config should use defaults");
    assert_eq!(
        default_config.ui.toolbar.side_layout,
        ToolbarSideLayout::Pill
    );

    for (value, expected) in [
        ("pill", ToolbarSideLayout::Pill),
        ("panel", ToolbarSideLayout::Panel),
    ] {
        let config: Config = toml::from_str(&format!("[ui.toolbar]\nside_layout = '{value}'\n"))
            .expect("supported side layout should parse");
        assert_eq!(config.ui.toolbar.side_layout, expected);

        let saved = toml::to_string(&config).expect("config should serialize");
        let reloaded: Config = toml::from_str(&saved).expect("saved config should reload");
        assert_eq!(reloaded.ui.toolbar.side_layout, expected);
    }

    let error = toml::from_str::<Config>("[ui.toolbar]\nside_layout = 'drawer'\n")
        .expect_err("unknown side layout should fail");
    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn ui_theme_rejects_unknown_values() {
    let error = toml::from_str::<Config>("[ui]\ntheme = 'sepia'\n")
        .expect_err("unknown ui theme should fail");
    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn ui_reduced_motion_defaults_to_auto_and_parses_explicit_values() {
    let default_config: Config = toml::from_str("").expect("empty config should use defaults");
    assert_eq!(default_config.ui.reduced_motion, ReducedMotion::Auto);

    for (value, expected) in [
        ("auto", ReducedMotion::Auto),
        ("on", ReducedMotion::On),
        ("off", ReducedMotion::Off),
    ] {
        let config: Config = toml::from_str(&format!("[ui]\nreduced_motion = '{value}'\n"))
            .expect("supported reduced motion value should parse");
        assert_eq!(config.ui.reduced_motion, expected);
    }
}

#[test]
fn ui_reduced_motion_rejects_unknown_values() {
    let error = toml::from_str::<Config>("[ui]\nreduced_motion = 'sometimes'\n")
        .expect_err("unknown reduced motion value should fail");
    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn ui_reduced_motion_maps_to_motion_enabled() {
    assert!(ReducedMotion::Auto.motion_enabled());
    assert!(ReducedMotion::Off.motion_enabled());
    assert!(!ReducedMotion::On.motion_enabled());
}

#[test]
fn status_bar_content_flags_default_true_and_round_trip() {
    let defaults: Config = toml::from_str("").expect("empty config should use defaults");
    for item in StatusBarItem::ALL {
        assert!(
            defaults.ui.status_bar_item_visible(item),
            "{item:?} should be visible by default"
        );
    }

    let source = r#"
[ui]
status_bar_interactive = false
active_output_badge = false
show_status_selection_info = false
show_status_board_badge = false
show_status_page_badge = false
show_status_color = false
show_status_tool = false
show_status_size = false
show_status_context_indicators = false
show_toolbar_hint = false
show_status_help = false
show_status_about = false
"#;
    let config: Config = toml::from_str(source).expect("status-bar content flags should parse");
    assert!(!config.ui.status_bar_interactive);
    for item in StatusBarItem::ALL {
        assert!(
            !config.ui.status_bar_item_visible(item),
            "{item:?} should honor its explicit false value"
        );
    }

    let serialized = toml::to_string(&config).expect("status-bar flags should serialize");
    let reloaded: Config = toml::from_str(&serialized).expect("serialized flags should reload");
    assert!(!reloaded.ui.status_bar_interactive);
    for item in StatusBarItem::ALL {
        assert!(!reloaded.ui.status_bar_item_visible(item));
    }
}

#[test]
fn tray_icon_style_defaults_to_auto_and_parses_explicit_values() {
    let default_config: Config = toml::from_str("").expect("empty config should use defaults");
    assert_eq!(default_config.tray.icon_style, TrayIconStyle::Auto);

    for (value, expected) in [
        ("auto", TrayIconStyle::Auto),
        ("symbolic", TrayIconStyle::Symbolic),
        ("colored", TrayIconStyle::Colored),
    ] {
        let config: Config = toml::from_str(&format!("[tray]\nicon_style = '{value}'\n"))
            .expect("supported tray icon style should parse");
        assert_eq!(config.tray.icon_style, expected);
    }
}

#[test]
fn tray_icon_style_rejects_unknown_values() {
    let error = toml::from_str::<Config>("[tray]\nicon_style = 'yellow'\n")
        .expect_err("unknown tray icon style should fail");
    assert!(error.to_string().contains("unknown variant"));
}

/// The legacy `Ctrl+K` palette / `Ctrl+Shift+P` capture pair is what the file
/// says, so it is what the session gets — and the file keeps both its text and
/// its revision, because nothing on this path writes `config.toml`.
#[test]
fn a_legacy_shortcut_pair_is_honored_as_authored_and_the_file_is_untouched() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        let config_path = primary_dir.join("config.toml");
        let original = "[keybindings]\ntoggle_command_palette = ['Ctrl+K']\ncapture_full_screen = ['Ctrl+Shift+P']\n";
        fs::write(&config_path, original).unwrap();

        let loaded = Config::load().expect("load succeeds");

        assert_eq!(
            loaded.config.keybindings.ui.toggle_command_palette,
            ["Ctrl+K"]
        );
        assert_eq!(
            loaded.config.keybindings.capture.capture_full_screen,
            ["Ctrl+Shift+P"]
        );
        assert_eq!(
            loaded.config.config_revision, 0,
            "loading never advances the authored revision"
        );
        assert!(loaded.validation.is_empty());
        assert_eq!(fs::read_to_string(config_path).unwrap(), original);
    });
}

/// A pair the user restores deliberately used to need the revision stamp to
/// protect it from the next load. Presence protects it directly now: the keys
/// are in the file, so nothing re-derives them.
#[test]
fn a_deliberately_restored_legacy_pair_survives_a_save_and_reload() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        let config_path = primary_dir.join("config.toml");
        fs::write(
            &config_path,
            "[keybindings]\ntoggle_command_palette = ['Ctrl+K']\ncapture_full_screen = ['Ctrl+Shift+P']\n",
        )
        .unwrap();

        let mut restored = Config::load().expect("legacy load succeeds").config;
        restored.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
        restored.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];
        save_through_document(restored);

        let reloaded = Config::load().expect("current load succeeds").config;
        assert_eq!(reloaded.config_revision, 0);
        assert_eq!(reloaded.keybindings.ui.toggle_command_palette, ["Ctrl+K"]);
        assert_eq!(
            reloaded.keybindings.capture.capture_full_screen,
            ["Ctrl+Shift+P"]
        );
    });
}

/// The old `config.example.toml` shipped `toggle_toolbar = ["F2", "F9"]`
/// verbatim, and `cycle_toolbar_display` arrived later wanting `F2`. The
/// authored pair keeps both keys and the newcomer's default stands down — the
/// #293 case, settled without a migration and without a write.
#[test]
fn an_authored_toolbar_pair_keeps_f2_from_the_newer_default() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        let config_path = primary_dir.join("config.toml");
        let original = "[keybindings]\ntoggle_toolbar = ['F2', 'F9']\nundo = ['Ctrl+Alt+U']\n";
        fs::write(&config_path, original).unwrap();

        let loaded = Config::load().expect("load succeeds");

        assert_eq!(loaded.config.keybindings.ui.toggle_toolbar, ["F2", "F9"]);
        assert!(
            loaded
                .config
                .keybindings
                .ui
                .cycle_toolbar_display
                .is_empty()
        );
        assert_eq!(
            loaded.config.keybindings.core.undo,
            ["Ctrl+Alt+U"],
            "custom bindings survive untouched"
        );
        assert_eq!(loaded.config.config_revision, 0);
        assert!(loaded.config.keybindings.build_action_map().is_ok());
        assert!(
            loaded.validation.keybinding_conflicts.is_empty(),
            "only one side of this is authored, so it is not the user's conflict"
        );
        let skipped = &loaded.validation.skipped_default_shortcuts;
        assert_eq!(skipped.len(), 1, "unexpected report: {skipped:?}");
        assert_eq!(skipped[0].binding(), "F2");
        assert_eq!(skipped[0].action(), Action::CycleToolbarDisplay);
        assert_eq!(skipped[0].claimed_by(), Action::ToggleToolbar);
        assert_eq!(fs::read_to_string(config_path).unwrap(), original);
    });
}

#[test]
fn custom_f2_toggle_toolbar_binding_keeps_f2_and_unbinds_cycle() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        fs::write(
            primary_dir.join("config.toml"),
            "[keybindings]\ntoggle_toolbar = ['F2']\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds").config;

        assert_eq!(
            loaded.keybindings.ui.toggle_toolbar,
            ["F2"],
            "a deliberate custom F2 binding keeps its old meaning"
        );
        assert!(
            loaded.keybindings.ui.cycle_toolbar_display.is_empty(),
            "the new cycle action must not steal the user's F2"
        );
        assert!(loaded.keybindings.build_action_map().is_ok());
    });
}

/// The authored side of the #293 case survives a save of one of its own
/// shortcuts and every load after it.
#[test]
fn a_deliberately_restored_f2_toggle_pair_survives_a_save_and_reload() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        let config_path = primary_dir.join("config.toml");
        fs::write(
            &config_path,
            "[keybindings]\ntoggle_toolbar = ['F2', 'F9']\n",
        )
        .unwrap();

        let mut restored = Config::load().expect("legacy load succeeds").config;
        restored.keybindings.ui.toggle_toolbar = vec!["F2".to_string(), "F9".to_string()];
        restored.keybindings.ui.cycle_toolbar_display = Vec::new();
        save_through_document(restored);

        let reloaded = Config::load().expect("current load succeeds").config;
        assert_eq!(reloaded.keybindings.ui.toggle_toolbar, ["F2", "F9"]);
        assert!(reloaded.keybindings.ui.cycle_toolbar_display.is_empty());
        assert!(reloaded.keybindings.build_action_map().is_ok());
    });
}

#[test]
fn a_pre_input_hud_file_that_claims_ctrl_shift_k_keeps_it_and_unbinds_the_hud() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        // Revision 2 is the last revision written before `toggle_input_hud`
        // existed, so the file cannot have opted out of its default.
        fs::write(
            primary_dir.join("config.toml"),
            "config_revision = 2\n\n[keybindings]\ncapture_clipboard_full = ['Ctrl+Shift+K']\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds").config;

        assert_eq!(
            loaded.keybindings.capture.capture_clipboard_full,
            ["Ctrl+Shift+K"],
            "the authored binding keeps the shortcut"
        );
        assert!(
            loaded.keybindings.ui.toggle_input_hud.is_empty(),
            "the input HUD must not steal a shortcut the file already used"
        );
        assert_eq!(loaded.config_revision, 2);
        assert!(loaded.keybindings.build_action_map().is_ok());
    });
}

/// Every revision reads the same way now, because nothing about resolution
/// depends on the stamp: an old file's authored shortcuts are authored, its
/// omitted actions are offered defaults, and the stamp stays where it is for
/// the configurator to act on.
#[test]
fn an_old_revision_resolves_by_presence_and_keeps_its_stamp() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        fs::write(
            primary_dir.join("config.toml"),
            "config_revision = 1\n\n[keybindings]\ntoggle_toolbar = ['F2', 'F9']\ntoggle_command_palette = ['Ctrl+K']\ncapture_full_screen = ['Ctrl+Shift+P']\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds").config;

        assert_eq!(loaded.keybindings.ui.toggle_toolbar, ["F2", "F9"]);
        assert!(loaded.keybindings.ui.cycle_toolbar_display.is_empty());
        assert_eq!(loaded.keybindings.ui.toggle_command_palette, ["Ctrl+K"]);
        assert_eq!(
            loaded.keybindings.capture.capture_full_screen,
            ["Ctrl+Shift+P"]
        );
        assert_eq!(loaded.config_revision, 1);
    });
}

/// Source presence decides authorship, not value comparison: a list that spells
/// out today's default is still the user's, and it outranks a default the file
/// never mentions. Written the other way round, this is the bug that made a
/// shipped default look authored and take a key from a binding that was.
#[test]
fn an_authored_list_equal_to_a_default_still_outranks_an_omitted_one() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        // `F2` is exactly what `cycle_toolbar_display` ships with, written here
        // under `toggle_toolbar` instead.
        fs::write(
            primary_dir.join("config.toml"),
            "[keybindings]\ntoggle_toolbar = ['F2']\n",
        )
        .unwrap();

        let report = Config::load().expect("load succeeds").validation;

        assert_eq!(report.skipped_default_shortcuts.len(), 1);
        assert_eq!(
            report.skipped_default_shortcuts[0].action(),
            Action::CycleToolbarDisplay
        );
        assert!(report.keybinding_conflicts.is_empty());
    });
}

/// An explicit empty list means unbound, and it says so at full volume: the
/// action is authored, so no default is ever offered to it.
#[test]
fn an_explicit_empty_list_stays_unbound() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        fs::write(
            primary_dir.join("config.toml"),
            "[keybindings]\nundo = []\ntoggle_input_hud = []\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds");

        assert!(loaded.config.keybindings.core.undo.is_empty());
        assert!(loaded.config.keybindings.ui.toggle_input_hud.is_empty());
        assert!(
            loaded.validation.is_empty(),
            "unbinding an action is not a problem to report: {:?}",
            loaded.validation
        );
    });
}

/// Two spellings of one chord in two authored lists is the user's conflict, and
/// the keymap traversal order settles it. Nothing about presence changes that.
#[test]
fn two_authored_actions_claiming_one_chord_are_settled_by_traversal_order() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        fs::write(
            primary_dir.join("config.toml"),
            "[keybindings]\nundo = ['ctrl+alt+u']\nredo = ['Ctrl+Alt+U']\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds");

        assert_eq!(loaded.config.keybindings.core.undo, ["ctrl+alt+u"]);
        assert!(loaded.config.keybindings.core.redo.is_empty());
        assert_eq!(loaded.validation.keybinding_conflicts.len(), 1);
        assert_eq!(
            loaded.validation.keybinding_conflicts[0].kept(),
            Action::Undo
        );
        assert!(loaded.validation.skipped_default_shortcuts.is_empty());
    });
}

#[test]
fn click_highlight_force_in_light_mode_defaults_true_and_parses_false() {
    let config: Config = toml::from_str("[ui.click_highlight]\nenabled = false\n")
        .expect("missing force_in_light_mode should use default");
    assert!(config.ui.click_highlight.force_in_light_mode);

    let config: Config = toml::from_str("[ui.click_highlight]\nforce_in_light_mode = false\n")
        .expect("explicit force_in_light_mode should parse");
    assert!(!config.ui.click_highlight.force_in_light_mode);
}

#[test]
fn input_hud_defaults_are_off_with_auto_mode_at_bottom_center() {
    let config = Config::default();
    assert!(!config.ui.input_hud.enabled);
    assert_eq!(config.ui.input_hud.mode, InputHudMode::Auto);
    assert_eq!(config.ui.input_hud.position, InputHudPosition::BottomCenter);
    assert!(config.ui.input_hud.show_mouse);
    assert!(config.ui.input_hud.show_bare_modifiers);
    assert_eq!(config.ui.input_hud.display_ms, 1600);
    assert_eq!(config.ui.input_hud.fade_ms, 350);
    assert_eq!(config.ui.input_hud.max_entries, 6);
    assert!(config.ui.input_hud.combine_repeats);
    assert!(!config.presenter_mode.enable_input_hud);
}

#[test]
fn input_hud_section_round_trips_its_kebab_case_enums() {
    let config: Config = toml::from_str(
        r#"
[ui.input_hud]
enabled = true
mode = "system"
position = "top-right"
show_mouse = false
display_ms = 900
max_entries = 3
"#,
    )
    .expect("input HUD section should parse");

    assert!(config.ui.input_hud.enabled);
    assert_eq!(config.ui.input_hud.mode, InputHudMode::System);
    assert_eq!(config.ui.input_hud.position, InputHudPosition::TopRight);
    assert!(!config.ui.input_hud.show_mouse);
    assert_eq!(config.ui.input_hud.display_ms, 900);
    assert_eq!(config.ui.input_hud.max_entries, 3);
    // Omitted keys keep their documented defaults.
    assert!(config.ui.input_hud.show_bare_modifiers);
    assert_eq!(config.ui.input_hud.fade_ms, 350);

    let rendered = toml::to_string(&config).expect("config should serialize");
    let reloaded: Config = toml::from_str(&rendered).expect("config should round-trip");
    assert_eq!(reloaded.ui.input_hud.mode, InputHudMode::System);
    assert_eq!(reloaded.ui.input_hud.position, InputHudPosition::TopRight);
}

/// The middle grid row of anchors parses like the corner rows.
#[test]
fn input_hud_center_anchors_parse() {
    for (name, expected) in [
        ("center-left", InputHudPosition::CenterLeft),
        ("center", InputHudPosition::Center),
        ("center-right", InputHudPosition::CenterRight),
    ] {
        let config: Config = toml::from_str(&format!("[ui.input_hud]\nposition = \"{name}\"\n"))
            .expect("center anchor should parse");
        assert_eq!(config.ui.input_hud.position, expected);
    }
}

#[test]
fn presenter_mode_enable_input_hud_parses() {
    let config: Config = toml::from_str(
        "[presenter_mode]
enable_input_hud = true
",
    )
    .expect("presenter input HUD toggle should parse");
    assert!(config.presenter_mode.enable_input_hud);
}

#[test]
fn drawing_quick_colors_default_when_drawing_table_omits_field() {
    let config: Config = toml::from_str("[drawing]\ndefault_color = 'blue'\n")
        .expect("drawing table without quick colors should parse");

    assert_eq!(config.drawing.quick_colors, QuickColorsConfig::default());
}

#[test]
fn drawing_quick_colors_default_when_drawing_table_is_missing() {
    let config: Config = toml::from_str("[ui]\nshow_status_bar = false\n")
        .expect("missing drawing table should parse");

    assert_eq!(config.drawing.quick_colors, QuickColorsConfig::default());
}

#[test]
fn drawing_quick_colors_parse_ordered_entries_with_hex_and_rgb() {
    let config: Config = toml::from_str(
        "[[drawing.quick_colors]]\nlabel = 'Soft pink'\ncolor = '#FFB3BA'\n\n[[drawing.quick_colors]]\nlabel = 'Ink'\ncolor = [1, 2, 3]\n",
    )
    .expect("ordered quick colors should parse");

    let palette = QuickColorPalette::from_config(&config.drawing.quick_colors);
    assert!(color_approx_eq(
        &palette.color_for_index(0).unwrap(),
        &crate::draw::Color {
            r: 1.0,
            g: 179.0 / 255.0,
            b: 186.0 / 255.0,
            a: 1.0,
        },
    ));
    assert!(color_approx_eq(
        &palette.color_for_index(1).unwrap(),
        &crate::draw::Color {
            r: 1.0 / 255.0,
            g: 2.0 / 255.0,
            b: 3.0 / 255.0,
            a: 1.0,
        },
    ));
    assert_eq!(
        palette.entry(0).map(|entry| entry.label.as_str()),
        Some("Soft pink")
    );
    assert_eq!(
        palette.entry(1).map(|entry| entry.label.as_str()),
        Some("Ink")
    );
}

#[test]
fn drawing_quick_colors_missing_shortcut_slots_backfill_defaults() {
    let config: Config =
        toml::from_str("[[drawing.quick_colors]]\nlabel = 'Only'\ncolor = 'blue'\n")
            .expect("short quick color list should parse");

    let palette = QuickColorPalette::from_config(&config.drawing.quick_colors);

    assert_eq!(palette.len(), 8);
    assert_eq!(
        palette.entry(0).map(|entry| entry.label.as_str()),
        Some("Only")
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorRed),
        Some(tuned_default("#3584E4"))
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorGreen),
        Some(tuned_default("#2EC27E"))
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorBlack),
        Some(tuned_default("#241F31"))
    );
}

#[test]
fn quick_colors_set_color_at_keeps_labels_and_materializes_defaults() {
    // A single authored entry: recoloring a backfilled slot must write the
    // whole effective palette so no slot is left implied.
    let mut config: Config =
        toml::from_str("[[drawing.quick_colors]]\nlabel = 'Only'\ncolor = 'blue'\n")
            .expect("short quick color list should parse");
    let crimson = crate::draw::Color {
        r: 220.0 / 255.0,
        g: 20.0 / 255.0,
        b: 60.0 / 255.0,
        a: 1.0,
    };

    assert_eq!(
        config.drawing.quick_colors.set_color_at(2, crimson),
        QuickColorWrite::Written
    );

    let entries = &config.drawing.quick_colors.entries;
    assert_eq!(entries.len(), 8, "backfilled slots become explicit");
    assert_eq!(entries[0].label, "Only", "authored labels survive");
    assert_eq!(
        entries[2].label, "Blue",
        "a recolored slot keeps its slot identity instead of being renamed"
    );
    let palette = QuickColorPalette::from_config(&config.drawing.quick_colors);
    assert!(color_approx_eq(
        &palette.color_for_index(2).unwrap(),
        &crimson
    ));

    // Rewriting the same color is not a change, so it cannot churn the file —
    // and it is reported apart from a slot that is not there at all, because a
    // caller writing to disk must not mistake one for the other.
    assert_eq!(
        config.drawing.quick_colors.set_color_at(2, crimson),
        QuickColorWrite::Unchanged
    );
    // A slot past the effective palette is a stale click, not a new entry.
    assert_eq!(
        config.drawing.quick_colors.set_color_at(8, crimson),
        QuickColorWrite::SlotMissing
    );
    assert_eq!(config.drawing.quick_colors.entries.len(), 8);
}

#[test]
fn quick_colors_set_color_at_starts_from_defaults_when_unconfigured() {
    let mut config = Config::default();
    assert!(config.drawing.quick_colors.is_implicit_default());
    let teal = crate::draw::Color {
        r: 0.0,
        g: 128.0 / 255.0,
        b: 128.0 / 255.0,
        a: 1.0,
    };

    assert_eq!(
        config.drawing.quick_colors.set_color_at(10, teal),
        QuickColorWrite::Written
    );

    // The palette is now authored, so it stops tracking default changes.
    assert!(!config.drawing.quick_colors.is_implicit_default());
    let palette = QuickColorPalette::from_config(&config.drawing.quick_colors);
    assert_eq!(palette.len(), 11);
    assert!(color_approx_eq(
        &palette.color_for_index(10).unwrap(),
        &teal
    ));
    assert_eq!(
        palette.entry(10).map(|entry| entry.label.as_str()),
        Some("Gray")
    );
}

#[test]
fn shipped_quick_color_defaults_cover_only_the_built_in_palette() {
    use crate::config::default_quick_color_for_index;

    // The values a "Default" restore puts back are the tuned built-ins, so
    // they must match the palette wayscriber ships.
    assert!(color_approx_eq(
        &default_quick_color_for_index(0).expect("built-in red"),
        &tuned_default("#F5333F")
    ));
    assert!(color_approx_eq(
        &default_quick_color_for_index(10).expect("built-in gray"),
        &tuned_default("#666666")
    ));
    // Slots past the built-in palette are user-added and have no default.
    assert_eq!(default_quick_color_for_index(11), None);
    assert_eq!(
        default_quick_color_for_index(QuickColorPalette::default().len()),
        None
    );
}

#[test]
fn palette_set_color_for_index_recolors_in_place() {
    let mut palette = QuickColorPalette::default();
    let label = palette.entry(1).map(|entry| entry.label.clone());
    let color = crate::draw::Color {
        r: 0.25,
        g: 0.5,
        b: 0.75,
        a: 1.0,
    };

    assert!(palette.set_color_for_index(1, color));
    assert_eq!(palette.color_for_index(1), Some(color));
    assert_eq!(palette.entry(1).map(|entry| entry.label.clone()), label);

    assert!(!palette.set_color_for_index(1, color), "no-op recolor");
    assert!(
        !palette.set_color_for_index(palette.len(), color),
        "index past the palette cannot grow it"
    );
}

/// The radial menu caches its static surface by this key, so an alpha-only
/// recolor of any slot has to change it or the ring keeps painting the old
/// swatch while clicking it applies the new value.
#[test]
fn palette_cache_key_changes_when_only_a_slot_alpha_changes() {
    let mut palette = QuickColorPalette::default();
    let opaque = crate::draw::Color {
        r: 0.25,
        g: 0.5,
        b: 0.75,
        a: 1.0,
    };
    assert!(palette.set_color_for_index(1, opaque));
    let before = palette.cache_key();

    assert!(palette.set_color_for_index(1, crate::draw::Color { a: 0.4, ..opaque }));

    assert_ne!(palette.cache_key(), before);
}

#[test]
fn drawing_quick_colors_default_palette_preserves_extended_toolbar_colors() {
    let palette = QuickColorPalette::default();

    assert_eq!(palette.len(), 11);
    assert_eq!(
        palette.rendered_len(),
        11,
        "toolbar palettes keep legacy extended colors"
    );
    assert_eq!(
        palette.radial_rendered_len(),
        8,
        "default radial menu preserves the pre-configurable 8-color ring"
    );
    assert_eq!(
        palette.entry(8).map(|entry| entry.label.as_str()),
        Some("Cyan")
    );
    assert_eq!(
        palette.entry(9).map(|entry| entry.label.as_str()),
        Some("Purple")
    );
    assert_eq!(
        palette.entry(10).map(|entry| entry.label.as_str()),
        Some("Gray")
    );
    assert!(color_approx_eq(
        &palette.color_for_index(8).unwrap(),
        &crate::draw::Color {
            r: 0.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    ));
    assert!(color_approx_eq(
        &palette.color_for_index(9).unwrap(),
        &crate::draw::Color {
            r: 153.0 / 255.0,
            g: 102.0 / 255.0,
            b: 204.0 / 255.0,
            a: 1.0,
        },
    ));
    assert!(color_approx_eq(
        &palette.color_for_index(10).unwrap(),
        &crate::draw::Color {
            r: 102.0 / 255.0,
            g: 102.0 / 255.0,
            b: 102.0 / 255.0,
            a: 1.0,
        },
    ));
}

#[test]
fn drawing_quick_colors_explicit_extra_entries_extend_radial_palette() {
    let config: Config = toml::from_str(
        r##"
[[drawing.quick_colors]]
label = "Red"
color = "red"
[[drawing.quick_colors]]
label = "Green"
color = "green"
[[drawing.quick_colors]]
label = "Blue"
color = "blue"
[[drawing.quick_colors]]
label = "Yellow"
color = "yellow"
[[drawing.quick_colors]]
label = "Orange"
color = "orange"
[[drawing.quick_colors]]
label = "Pink"
color = "pink"
[[drawing.quick_colors]]
label = "White"
color = "white"
[[drawing.quick_colors]]
label = "Black"
color = "black"
[[drawing.quick_colors]]
label = "Cyan"
color = "#00FFFF"
"##,
    )
    .expect("explicit quick colors should parse");

    let palette = QuickColorPalette::from_config(&config.drawing.quick_colors);

    assert_eq!(
        config.drawing.quick_colors.configured_entry_count(),
        Some(9)
    );
    assert_eq!(palette.rendered_len(), 9);
    assert_eq!(palette.radial_rendered_len(), 9);
    assert_eq!(palette.radial_rendered_entries()[8].label.as_str(), "Cyan");
}

#[test]
fn drawing_quick_colors_implicit_defaults_do_not_serialize_as_explicit_entries() {
    let config_str =
        toml::to_string_pretty(&Config::default()).expect("default config should serialize");

    assert!(
        !config_str.contains("quick_colors"),
        "implicit quick color defaults should not become explicit radial extras on save"
    );
}

#[test]
fn drawing_quick_color_actions_stay_limited_to_first_eight_slots() {
    let palette = QuickColorPalette::default();

    assert_eq!(
        palette.color_for_action(Action::SetColorRed),
        Some(tuned_default("#F5333F"))
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorGreen),
        Some(tuned_default("#2EC27E"))
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorBlue),
        Some(tuned_default("#3584E4"))
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorYellow),
        Some(tuned_default("#F6D32D"))
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorOrange),
        Some(tuned_default("#FF7800"))
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorPink),
        Some(tuned_default("#C061CB"))
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorWhite),
        Some(tuned_default("#FFFFFF"))
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorBlack),
        Some(tuned_default("#241F31"))
    );
    assert_eq!(QuickColorPalette::action_for_index(8), None);
    assert_eq!(QuickColorPalette::action_for_index(9), None);
    assert_eq!(QuickColorPalette::action_for_index(10), None);
}

#[test]
fn drawing_quick_color_rendered_entries_are_capped_without_dropping_config() {
    let entries = (0..QUICK_COLOR_RENDER_LIMIT + 3)
        .map(|index| QuickColorPaletteEntry {
            label: format!("Color {index}"),
            color: crate::draw::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        })
        .collect();
    let palette = QuickColorPalette::from_entries(entries);

    assert_eq!(palette.len(), QUICK_COLOR_RENDER_LIMIT + 3);
    assert_eq!(palette.rendered_len(), QUICK_COLOR_RENDER_LIMIT);
    assert_eq!(palette.rendered_entries().len(), QUICK_COLOR_RENDER_LIMIT);
    assert_eq!(palette.radial_rendered_len(), QUICK_COLOR_RENDER_LIMIT);
    assert!(palette.color_for_index(QUICK_COLOR_RENDER_LIMIT).is_some());
    assert!(
        palette
            .rendered_color_for_index(QUICK_COLOR_RENDER_LIMIT)
            .is_none()
    );
}

#[test]
fn drawing_quick_colors_invalid_hash_hex_warns_and_falls_back_red() {
    let config: Config = toml::from_str(
        "[[drawing.quick_colors]]\nlabel = 'Invalid'\ncolor = '#GG0000'\n\n[[drawing.quick_colors]]\nlabel = 'Short'\ncolor = '#12345'\n",
    )
    .expect("invalid hash-looking hex strings keep load compatibility");

    let palette = QuickColorPalette::from_config(&config.drawing.quick_colors);
    assert_eq!(palette.color_for_index(0), Some(tuned_default("#F5333F")));
    assert_eq!(palette.color_for_index(1), Some(tuned_default("#F5333F")));
}

#[test]
fn named_colors_bit_match_default_quick_color_slots() {
    let palette = QuickColorPalette::default();

    for (index, name) in [
        "red", "green", "blue", "yellow", "orange", "pink", "white", "black",
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            Some(ColorSpec::Name(name.to_string()).to_color()),
            palette.color_for_index(index),
            "named '{name}' must bit-match default quick color slot {index}"
        );
    }

    // The startup pen color is `default_color = "red"`; it must bit-match
    // slot 0 so the swatch selection ring is shown on default configs.
    assert_eq!(
        Some(DrawingConfig::default().default_color.to_color()),
        palette.color_for_index(0)
    );
}

#[test]
fn drawing_quick_colors_empty_array_uses_runtime_default_palette() {
    let config: Config =
        toml::from_str("[drawing]\nquick_colors = []\n").expect("empty quick color array parses");

    let palette = QuickColorPalette::from_config(&config.drawing.quick_colors);
    assert_eq!(palette, QuickColorPalette::default());
    assert_eq!(palette.radial_rendered_len(), 8);
}

#[test]
fn pdf_transparent_background_defaults_to_none() {
    assert_eq!(
        Config::default().export.pdf.transparent_background,
        PdfTransparentBackground::None
    );
}

/// Resolves one of the tuned built-in palette hex values exactly like
/// `ColorSpec::to_color`, so assertions can compare with `==`.
fn tuned_default(hex: &str) -> crate::draw::Color {
    crate::util::parse_config_hex_color(hex).expect("tuned default hex is valid")
}

fn color_approx_eq(a: &crate::draw::Color, b: &crate::draw::Color) -> bool {
    (a.r - b.r).abs() < 0.001
        && (a.g - b.g).abs() < 0.001
        && (a.b - b.b).abs() < 0.001
        && (a.a - b.a).abs() < 0.001
}

#[test]
fn pdf_transparent_background_parses_desktop() {
    let config: Config = toml::from_str("[export.pdf]\ntransparent_background = 'desktop'\n")
        .expect("desktop transparent background should parse");

    assert_eq!(
        config.export.pdf.transparent_background,
        PdfTransparentBackground::Desktop
    );
}

#[test]
fn pdf_transparent_background_rejects_unknown_values() {
    let err = toml::from_str::<Config>("[export.pdf]\ntransparent_background = 'wallpaper'\n")
        .expect_err("unknown transparent background should be rejected");

    assert!(
        err.to_string().contains("wallpaper"),
        "unexpected error: {err}"
    );
}

#[test]
fn load_parses_mouse_button_drag_tool_bindings() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        fs::write(
            primary_dir.join("config.toml"),
            "[drawing.drag_tools.left]\ndrag_tool = 'line'\nshift_drag_tool = 'pen'\n\n[drawing.drag_tools.right]\ndrag_tool = 'pen'\ndrag_color = 'blue'\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds");
        let drag_tools = loaded.config.drawing.drag_tools.expect("drag tools config");
        assert_eq!(drag_tools.left.drag_tool, crate::input::DragTool::Line);
        assert_eq!(drag_tools.left.shift_drag_tool, crate::input::DragTool::Pen);
        assert_eq!(drag_tools.right.drag_tool, crate::input::DragTool::Pen);
        assert_eq!(
            drag_tools.right.drag_color,
            Some(ColorSpec::Name("blue".to_string()))
        );
    });
}

#[test]
fn legacy_drag_fields_accept_drag_bindable_polygon_tools() {
    let config: Config =
        toml::from_str("[drawing]\ndrag_tool = 'regular-polygon'\nshift_drag_tool = 'triangle'\n")
            .expect("drag-bindable polygon tools should parse");

    assert_eq!(
        config.drawing.drag_tool,
        crate::input::DragBindableTool::RegularPolygon
    );
    assert_eq!(
        config.drawing.shift_drag_tool,
        crate::input::DragBindableTool::Triangle
    );
    let drag_tools = config.drawing.effective_drag_tools();
    assert_eq!(
        drag_tools.left.drag_tool,
        crate::input::DragTool::RegularPolygon
    );
    assert_eq!(
        drag_tools.left.shift_drag_tool,
        crate::input::DragTool::Triangle
    );
}

#[test]
fn drag_config_rejects_freeform_polygon() {
    let legacy_err = toml::from_str::<Config>("[drawing]\ndrag_tool = 'freeform-polygon'\n")
        .expect_err("freeform polygon must not parse in legacy drag fields");
    assert!(legacy_err.to_string().contains("freeform-polygon"));

    let per_button_err =
        toml::from_str::<Config>("[drawing.drag_tools.left]\ndrag_tool = 'freeform-polygon'\n")
            .expect_err("freeform polygon must not parse in per-button drag fields");
    assert!(per_button_err.to_string().contains("freeform-polygon"));
}

#[test]
fn effective_drag_tools_preserve_legacy_left_when_only_right_is_configured() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        fs::write(
            primary_dir.join("config.toml"),
            "[drawing]\ndrag_tool = 'arrow'\nshift_drag_tool = 'eraser'\n\n[drawing.drag_tools.right]\ndrag_tool = 'pen'\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds");
        let drag_tools = loaded.config.drawing.effective_drag_tools();
        assert_eq!(drag_tools.left.drag_tool, crate::input::DragTool::Arrow);
        assert_eq!(
            drag_tools.left.shift_drag_tool,
            crate::input::DragTool::Eraser
        );
        assert_eq!(drag_tools.right.drag_tool, crate::input::DragTool::Pen);
    });
}

#[test]
fn effective_drag_tools_preserve_explicit_builtin_left_mapping() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        fs::write(
            primary_dir.join("config.toml"),
            "[drawing]\ndrag_tool = 'arrow'\nshift_drag_tool = 'eraser'\n\n[drawing.drag_tools.left]\ndrag_tool = 'pen'\nshift_drag_tool = 'line'\nctrl_drag_tool = 'rect'\nctrl_shift_drag_tool = 'arrow'\ntab_drag_tool = 'ellipse'\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds");
        let drag_tools = loaded.config.drawing.effective_drag_tools();
        assert_eq!(drag_tools.left.drag_tool, crate::input::DragTool::Pen);
        assert_eq!(
            drag_tools.left.shift_drag_tool,
            crate::input::DragTool::Line
        );
        assert_eq!(drag_tools.left.ctrl_drag_tool, crate::input::DragTool::Rect);
        assert_eq!(
            drag_tools.left.ctrl_shift_drag_tool,
            crate::input::DragTool::Arrow
        );
        assert_eq!(
            drag_tools.left.tab_drag_tool,
            crate::input::DragTool::Ellipse
        );
    });
}

#[test]
fn ui_defaults_follow_desktop_for_xdg_focus_loss() {
    let desktop_like_gnome = crate::env_vars::DESKTOP_ENV_KEYS
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .any(|value| {
            let value = value.to_lowercase();
            value.contains("ubuntu") || value.contains("gnome")
        });
    let expected = if cfg!(target_os = "linux") && desktop_like_gnome {
        XdgFocusLossBehavior::Stay
    } else {
        XdgFocusLossBehavior::Exit
    };

    assert_eq!(Config::default().ui.xdg_focus_loss_behavior, expected);
}

#[test]
fn ui_status_bar_interactive_defaults_to_true() {
    assert!(Config::default().ui.status_bar_interactive);

    // Omitting the key in an existing [ui] table keeps the default.
    let config: Config = toml::from_str("[ui]\nshow_status_bar = true\n")
        .expect("ui table without status_bar_interactive should parse");
    assert!(config.ui.status_bar_interactive);
}

#[test]
fn ui_status_bar_interactive_round_trips_disabled_value() {
    let parsed: Config = toml::from_str("[ui]\nstatus_bar_interactive = false\n")
        .expect("status_bar_interactive = false should parse");
    assert!(!parsed.ui.status_bar_interactive);

    let serialized = toml::to_string(&parsed).expect("config serializes");
    let reparsed: Config = toml::from_str(&serialized).expect("serialized config reparses");
    assert!(!reparsed.ui.status_bar_interactive);
}

#[cfg(feature = "tablet-input")]
#[test]
fn load_defaults_tablet_input_to_enabled_when_section_is_missing() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        fs::write(
            primary_dir.join("config.toml"),
            "[drawing]\ndefault_color = 'red'\n",
        )
        .unwrap();

        let loaded = Config::load().expect("load succeeds");
        assert!(loaded.config.tablet.enabled);
    });
}

#[cfg(feature = "tablet-input")]
#[test]
fn tablet_stylus_button_bindings_default_to_primary_radial_menu() {
    let config = Config::default();

    assert_eq!(
        config.tablet.stylus_button.action,
        Some(Action::ToggleRadialMenu)
    );
    assert_eq!(config.tablet.stylus_button2.action, None);
}

#[cfg(feature = "tablet-input")]
#[test]
fn tablet_stylus_button_action_omission_unbinds_button() {
    let config: Config =
        toml::from_str("[tablet.stylus_button]\n").expect("empty stylus button table should parse");

    assert_eq!(config.tablet.stylus_button.action, None);
}

#[cfg(feature = "tablet-input")]
#[test]
fn tablet_stylus_button_bindings_parse_custom_actions() {
    let config: Config = toml::from_str(
        "[tablet.stylus_button]\naction = 'undo'\n\n[tablet.stylus_button2]\naction = 'redo'\n",
    )
    .expect("stylus button actions should parse");

    assert_eq!(config.tablet.stylus_button.action, Some(Action::Undo));
    assert_eq!(config.tablet.stylus_button2.action, Some(Action::Redo));
}
