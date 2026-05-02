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
