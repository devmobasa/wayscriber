use super::super::*;
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

#[test]
fn load_migrates_legacy_shortcut_defaults_in_memory_without_rewriting_file() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        let config_path = primary_dir.join("config.toml");
        let original = "[keybindings]\ntoggle_command_palette = ['Ctrl+K']\ncapture_full_screen = ['Ctrl+Shift+P']\n";
        fs::write(&config_path, original).unwrap();

        let loaded = Config::load().expect("load succeeds");

        assert_eq!(
            loaded.config.keybindings.ui.toggle_command_palette,
            ["Ctrl+K", "Ctrl+Shift+P"]
        );
        assert_eq!(
            loaded.config.keybindings.capture.capture_full_screen,
            ["Ctrl+Alt+F"]
        );
        assert_eq!(
            loaded.config.config_revision,
            crate::config::CURRENT_CONFIG_REVISION
        );
        assert_eq!(fs::read_to_string(config_path).unwrap(), original);
    });
}

#[test]
fn saved_migration_revision_preserves_a_later_intentional_legacy_pair() {
    with_temp_config_home(|config_root| {
        let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&primary_dir).unwrap();
        let config_path = primary_dir.join("config.toml");
        fs::write(
            &config_path,
            "[keybindings]\ntoggle_command_palette = ['Ctrl+K']\ncapture_full_screen = ['Ctrl+Shift+P']\n",
        )
        .unwrap();

        let mut migrated = Config::load().expect("legacy load succeeds").config;
        migrated.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
        migrated.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];
        migrated.save().expect("saving revision succeeds");

        let reloaded = Config::load().expect("current load succeeds").config;
        assert_eq!(reloaded.config_revision, CURRENT_CONFIG_REVISION);
        assert_eq!(reloaded.keybindings.ui.toggle_command_palette, ["Ctrl+K"]);
        assert_eq!(
            reloaded.keybindings.capture.capture_full_screen,
            ["Ctrl+Shift+P"]
        );
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
