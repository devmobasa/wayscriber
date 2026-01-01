use super::super::*;
use crate::config::{SessionConfig, SessionStorageMode};

#[test]
fn options_from_config_custom_storage() {
    let temp = tempfile::tempdir().unwrap();
    let custom_dir = temp.path().join("sessions");

    let cfg = SessionConfig {
        persist_transparent: true,
        storage: SessionStorageMode::Custom,
        custom_directory: Some(custom_dir.to_string_lossy().to_string()),
        ..SessionConfig::default()
    };

    let mut options = options_from_config(&cfg, temp.path(), Some("display-1")).unwrap();
    assert_eq!(options.base_dir, custom_dir);
    assert!(options.persist_transparent);
    options.set_output_identity(Some("DP-1"));
    assert_eq!(
        options
            .session_file_path()
            .file_name()
            .unwrap()
            .to_string_lossy(),
        "session-display_1-DP_1.json"
    );
}

#[test]
fn options_from_config_config_storage_uses_config_dir() {
    let temp = tempfile::tempdir().unwrap();

    let cfg = SessionConfig {
        persist_whiteboard: true,
        storage: SessionStorageMode::Config,
        ..SessionConfig::default()
    };

    let original_display = std::env::var_os("WAYLAND_DISPLAY");
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    let mut options = options_from_config(&cfg, temp.path(), None).unwrap();
    if let Some(value) = original_display {
        unsafe { std::env::set_var("WAYLAND_DISPLAY", value) }
    }

    assert_eq!(options.base_dir, temp.path());
    assert!(options.persist_whiteboard);
    assert_eq!(
        options
            .session_file_path()
            .file_name()
            .unwrap()
            .to_string_lossy(),
        "session-default.json"
    );
    options.set_output_identity(Some("Monitor-Primary"));
    assert_eq!(
        options
            .session_file_path()
            .file_name()
            .unwrap()
            .to_string_lossy(),
        "session-default-Monitor_Primary.json"
    );
}

#[test]
fn session_file_without_per_output_suffix_when_disabled() {
    let mut options = SessionOptions::new(std::path::PathBuf::from("/tmp"), "display");
    options.per_output = false;
    let original = options.session_file_path();
    options.set_output_identity(Some("DP-1"));
    assert_eq!(options.session_file_path(), original);
    assert!(options.output_identity().is_none());
}
