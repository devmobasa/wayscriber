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
fn pdf_transparent_background_defaults_to_none() {
    assert_eq!(
        Config::default().export.pdf.transparent_background,
        PdfTransparentBackground::None
    );
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
    let desktop_like_gnome = [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_DESKTOP",
        "DESKTOP_SESSION",
    ]
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
