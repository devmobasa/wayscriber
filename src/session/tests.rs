use super::*;
use crate::config::{Action, BoardConfig, SessionConfig, SessionStorageMode};
use crate::draw::FontDescriptor;
use crate::draw::{Color, Shape};
use crate::input::InputState;
use std::collections::HashMap;
use std::path::PathBuf;

fn dummy_input_state() -> InputState {
    use crate::config::KeyBinding;
    use crate::draw::Color as DrawColor;

    let mut action_map = HashMap::new();
    action_map.insert(KeyBinding::parse("Escape").unwrap(), Action::Exit);
    InputState::with_defaults(
        DrawColor {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        3.0,
        32.0,
        FontDescriptor::default(),
        false,
        20.0,
        30.0,
        true,
        BoardConfig::default(),
        action_map,
    )
}

#[test]
fn snapshot_skips_when_empty_and_no_tool_state() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "test");
    options.persist_transparent = true;
    options.restore_tool_state = false;
    options.max_shapes_per_frame = 100;
    options.max_file_size_bytes = 1024 * 1024;
    options.compression = CompressionMode::Off;
    options.auto_compress_threshold_bytes = DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES;

    let input = dummy_input_state();
    assert!(snapshot_from_input(&input, &options).is_none());
}

#[test]
fn snapshot_includes_frames_and_tool_state() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.persist_transparent = true;

    let mut input = dummy_input_state();
    input.canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 2.0,
    });

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
    assert!(snapshot.transparent.is_some());
    assert!(snapshot.tool_state.is_some());
}

#[test]
fn options_from_config_custom_storage() {
    let temp = tempfile::tempdir().unwrap();
    let custom_dir = temp.path().join("sessions");

    let mut cfg = SessionConfig::default();
    cfg.persist_transparent = true;
    cfg.storage = SessionStorageMode::Custom;
    cfg.custom_directory = Some(custom_dir.to_string_lossy().to_string());

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

    let mut cfg = SessionConfig::default();
    cfg.persist_whiteboard = true;
    cfg.storage = SessionStorageMode::Config;

    let original_display = std::env::var_os("WAYLAND_DISPLAY");
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    let mut options = options_from_config(&cfg, temp.path(), None).unwrap();
    match original_display {
        Some(value) => unsafe { std::env::set_var("WAYLAND_DISPLAY", value) },
        None => {}
    };

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
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.per_output = false;
    let original = options.session_file_path();
    options.set_output_identity(Some("DP-1"));
    assert_eq!(options.session_file_path(), original);
    assert!(options.output_identity().is_none());
}
