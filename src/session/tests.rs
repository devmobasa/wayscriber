use super::*;
use crate::config::{Action, BoardConfig, SessionConfig, SessionStorageMode};
use crate::draw::FontDescriptor;
use crate::draw::{Color, Shape};
use crate::input::{InputState, board_mode::BoardMode};
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
        usize::MAX,
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

#[test]
fn session_roundtrip_preserves_shapes_across_frames() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-2");
    options.persist_transparent = true;
    options.persist_whiteboard = true;
    options.persist_blackboard = true;
    options.set_output_identity(Some("HDMI-1"));

    let mut input = dummy_input_state();
    input.canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 20,
        y2: 20,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 3.0,
    });

    input.canvas_set.switch_mode(BoardMode::Whiteboard);
    input.canvas_set.active_frame_mut().add_shape(Shape::Text {
        x: 5,
        y: 5,
        text: "hello".into(),
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        size: 24.0,
        font_descriptor: FontDescriptor::default(),
        background_enabled: false,
    });

    input.canvas_set.switch_mode(BoardMode::Blackboard);
    input
        .canvas_set
        .active_frame_mut()
        .add_shape(Shape::Ellipse {
            cx: 10,
            cy: 10,
            rx: 4,
            ry: 8,
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            thick: 1.5,
        });

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot produced");
    save_snapshot(&snapshot, &options).expect("save snapshot");

    let loaded_snapshot = load_snapshot(&options)
        .expect("load snapshot result")
        .expect("snapshot present");

    let mut fresh_input = dummy_input_state();
    apply_snapshot(&mut fresh_input, loaded_snapshot, &options);

    fresh_input.canvas_set.switch_mode(BoardMode::Transparent);
    assert_eq!(fresh_input.canvas_set.active_frame().shapes.len(), 1);

    fresh_input.canvas_set.switch_mode(BoardMode::Whiteboard);
    assert_eq!(fresh_input.canvas_set.active_frame().shapes.len(), 1);

    fresh_input.canvas_set.switch_mode(BoardMode::Blackboard);
    assert_eq!(fresh_input.canvas_set.active_frame().shapes.len(), 1);
}
