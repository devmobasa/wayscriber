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
