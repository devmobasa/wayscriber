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
