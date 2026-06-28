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
        fs::write(&config_file, "old_content = true").unwrap();

        let config = Config::default();
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
        assert_eq!(
            fs::read_to_string(&backup_path).unwrap(),
            "old_content = true"
        );

        let new_contents = fs::read_to_string(&config_file).unwrap();
        assert!(
            new_contents.contains("[drawing]"),
            "new config should be serialized TOML"
        );
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
        fs::write(&target, "old_content = true").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&target, &config_link).unwrap();

        let backup_path = Config::default()
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
            "old_content = true",
            "backup should capture the pre-save target contents"
        );
        assert!(
            backup_path
                .parent()
                .is_some_and(|parent| parent == config_dir),
            "backup should stay next to the user-facing config path"
        );

        let target_contents = fs::read_to_string(&target).unwrap();
        assert!(
            target_contents.contains("[drawing]"),
            "symlink target should receive the serialized config"
        );
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
