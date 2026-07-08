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
        Some(crate::draw::color::BLUE)
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorGreen),
        Some(crate::draw::color::GREEN)
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorBlack),
        Some(crate::draw::color::BLACK)
    );
}

#[test]
fn drawing_quick_colors_default_palette_preserves_extended_toolbar_colors() {
    let palette = QuickColorPalette::default();

    assert_eq!(palette.len(), 11);
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
fn drawing_quick_color_actions_stay_limited_to_first_eight_slots() {
    let palette = QuickColorPalette::default();

    assert_eq!(
        palette.color_for_action(Action::SetColorRed),
        Some(crate::draw::color::RED)
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorGreen),
        Some(crate::draw::color::GREEN)
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorBlue),
        Some(crate::draw::color::BLUE)
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorYellow),
        Some(crate::draw::color::YELLOW)
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorOrange),
        Some(crate::draw::color::ORANGE)
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorPink),
        Some(crate::draw::color::PINK)
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorWhite),
        Some(crate::draw::color::WHITE)
    );
    assert_eq!(
        palette.color_for_action(Action::SetColorBlack),
        Some(crate::draw::color::BLACK)
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
    assert_eq!(palette.color_for_index(0), Some(crate::draw::color::RED));
    assert_eq!(palette.color_for_index(1), Some(crate::draw::color::RED));
}

#[test]
fn drawing_quick_colors_empty_array_uses_runtime_default_palette() {
    let config: Config =
        toml::from_str("[drawing]\nquick_colors = []\n").expect("empty quick color array parses");

    let palette = QuickColorPalette::from_config(&config.drawing.quick_colors);
    assert_eq!(palette, QuickColorPalette::default());
}

#[test]
fn pdf_transparent_background_defaults_to_none() {
    assert_eq!(
        Config::default().export.pdf.transparent_background,
        PdfTransparentBackground::None
    );
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

#[cfg(tablet)]
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

#[cfg(tablet)]
#[test]
fn tablet_stylus_button_bindings_default_to_primary_radial_menu() {
    let config = Config::default();

    assert_eq!(
        config.tablet.stylus_button.action,
        Some(Action::ToggleRadialMenu)
    );
    assert_eq!(config.tablet.stylus_button2.action, None);
}

#[cfg(tablet)]
#[test]
fn tablet_stylus_button_action_omission_unbinds_button() {
    let config: Config =
        toml::from_str("[tablet.stylus_button]\n").expect("empty stylus button table should parse");

    assert_eq!(config.tablet.stylus_button.action, None);
}

#[cfg(tablet)]
#[test]
fn tablet_stylus_button_bindings_parse_custom_actions() {
    let config: Config = toml::from_str(
        "[tablet.stylus_button]\naction = 'undo'\n\n[tablet.stylus_button2]\naction = 'redo'\n",
    )
    .expect("stylus button actions should parse");

    assert_eq!(config.tablet.stylus_button.action, Some(Action::Undo));
    assert_eq!(config.tablet.stylus_button2.action, Some(Action::Redo));
}
