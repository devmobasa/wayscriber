use super::super::*;
use crate::config::test_helpers::with_temp_config_home;
use std::fs;

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
