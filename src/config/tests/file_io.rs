use super::super::*;
use crate::config::test_helpers::with_temp_config_home;
use std::fs;

#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink};

#[test]
fn save_with_backup_creates_timestamped_file() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        let original = "# Keep this backup source comment.\n[ui.toolbar]\nside_pinned = true\n";
        fs::write(&config_file, original).unwrap();

        let mut config = Config::load()
            .expect("load config before backup save")
            .config;
        config.ui.toolbar.side_pinned = false;
        let backup_path = config
            .save_with_backup()
            .expect("save_with_backup should succeed")
            .expect("backup should be created");

        assert!(backup_path.exists());
        assert!(
            backup_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("config.toml."),
            "backup file should include timestamp suffix"
        );
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), original);

        let new_contents = fs::read_to_string(&config_file).unwrap();
        assert!(new_contents.contains("# Keep this backup source comment."));
        assert!(new_contents.contains("side_pinned = false"));
        assert!(!new_contents.contains("[drawing]"));
    });
}

#[test]
fn runtime_toolbar_preference_save_preserves_comments_and_unrelated_toml_formatting() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        fs::write(
            &config_file,
            r#"# Keep this user comment.

[ui.toolbar]
side_pinned = true

[boards]
default_board = "transparent"

[[boards.items]]
id = "transparent"
name = "Overlay"
background = "transparent"

[[boards.items]]
id = "whiteboard"
name = "Whiteboard"
background = { rgb = [0.992, 0.992, 0.992] }
default_pen_color = { rgb = [0.0, 0.0, 0.0] }
"#,
        )
        .unwrap();

        let mut config = Config::load().expect("load sparse config").config;
        config.ui.toolbar.side_pinned = false;
        config.save().expect("save runtime toolbar preference");

        let saved = fs::read_to_string(&config_file).unwrap();
        assert!(saved.contains("# Keep this user comment."));
        assert!(saved.contains("side_pinned = false"));
        assert!(saved.contains("background = { rgb = [0.992, 0.992, 0.992] }"));
        assert!(saved.contains("default_pen_color = { rgb = [0.0, 0.0, 0.0] }"));
        assert!(!saved.contains("[drawing]"));
    });
}

#[test]
fn runtime_save_updates_an_intentionally_changed_inline_board_color() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        fs::write(
            &config_file,
            r#"# Preserve this comment while changing the color.

[boards]
default_board = "transparent"

[[boards.items]]
id = "transparent"
name = "Overlay"
background = "transparent"

[[boards.items]]
id = "whiteboard"
name = "Whiteboard"
background = { rgb = [0.992, 0.992, 0.992] }
"#,
        )
        .unwrap();

        let mut config = Config::load().expect("load board config").config;
        let whiteboard = config
            .boards
            .as_mut()
            .unwrap()
            .items
            .iter_mut()
            .find(|board| board.id == "whiteboard")
            .unwrap();
        whiteboard.background =
            BoardBackgroundConfig::Color(BoardColorConfig::Rgb([0.2, 0.3, 0.4]));
        config.save().expect("save changed board color");

        let saved = fs::read_to_string(&config_file).unwrap();
        assert!(saved.contains("# Preserve this comment while changing the color."));
        assert!(!saved.contains("background = { rgb = [0.992, 0.992, 0.992] }"));
        let reloaded = Config::load().expect("reload changed board config").config;
        let whiteboard = reloaded
            .boards
            .as_ref()
            .unwrap()
            .items
            .iter()
            .find(|board| board.id == "whiteboard")
            .unwrap();
        match &whiteboard.background {
            BoardBackgroundConfig::Color(color) => {
                assert_eq!(color.rgb(), [0.2, 0.3, 0.4]);
            }
            BoardBackgroundConfig::Transparent(value) => {
                panic!("expected changed color board, got {value}");
            }
        }
    });
}

#[cfg(unix)]
#[test]
fn save_with_backup_preserves_symlinked_config_target_and_backup_contents() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        let managed_dir = config_root.join("managed-config");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&managed_dir).unwrap();

        let target = managed_dir.join("config.toml");
        let config_link = config_dir.join("config.toml");
        let original = "# Keep this symlinked comment.\n[ui.toolbar]\nside_pinned = true\n";
        fs::write(&target, original).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&target, &config_link).unwrap();

        let mut config = Config::load().expect("load symlinked config").config;
        config.ui.toolbar.side_pinned = false;
        let backup_path = config
            .save_with_backup()
            .expect("save_with_backup should succeed for symlinked config")
            .expect("backup should be created for symlinked config");

        assert!(
            fs::symlink_metadata(&config_link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "config path should remain a symlink"
        );
        assert_eq!(fs::read_link(&config_link).unwrap(), target);
        assert_eq!(
            fs::read_to_string(&backup_path).unwrap(),
            original,
            "backup should capture the pre-save target contents"
        );
        assert!(
            backup_path
                .parent()
                .is_some_and(|parent| parent == config_dir),
            "backup should stay next to the user-facing config path"
        );

        let target_contents = fs::read_to_string(&target).unwrap();
        assert!(target_contents.contains("# Keep this symlinked comment."));
        assert!(target_contents.contains("side_pinned = false"));
        assert!(!target_contents.contains("[drawing]"));
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600,
            "symlink target permissions should be preserved"
        );
    });
}

#[test]
fn create_default_file_writes_example_when_missing() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        assert!(
            !config_dir.join("config.toml").exists(),
            "config.toml should not exist before create_default_file"
        );

        Config::create_default_file().expect("create_default_file should succeed");

        let config_path = config_dir.join("config.toml");
        let contents = fs::read_to_string(&config_path).expect("config file should be readable");
        assert!(
            contents.contains("[drawing]"),
            "default config should include [drawing] section"
        );
    });
}

#[test]
fn config_example_parses_and_documents_current_user_facing_fields() {
    let example = include_str!("../../../config.example.toml");
    toml::from_str::<Config>(example).expect("config.example.toml should parse");

    assert!(
        example.contains("show_floating_badge_always ="),
        "example should use the current floating badge field name"
    );
    assert!(
        !example.contains("show_page_badge_with_status_bar ="),
        "example should not use the old floating badge alias"
    );

    for field in [
        "undo_all",
        "redo_all",
        "undo_all_delayed",
        "redo_all_delayed",
        "board_1",
        "board_2",
        "board_3",
        "board_4",
        "board_5",
        "board_6",
        "board_7",
        "board_8",
        "board_9",
        "board_prev",
        "board_next",
        "board_new",
        "board_duplicate",
        "board_delete",
        "board_picker",
        "toggle_quick_help",
        "toggle_command_palette",
    ] {
        assert!(
            example.contains(&format!("{field} =")),
            "example should document keybinding field `{field}`"
        );
    }
}

#[test]
fn create_default_file_errors_when_config_exists() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("config.toml");
        fs::write(&config_path, "custom = true").unwrap();

        let err = Config::create_default_file()
            .expect_err("create_default_file should fail when config exists");
        let msg = err.to_string();
        assert!(
            msg.contains("already exists"),
            "error message should mention existing config, got: {msg}"
        );
    });
}
