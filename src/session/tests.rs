use super::*;
use crate::config::{Action, BoardConfig, SessionConfig, SessionStorageMode};
use crate::draw::FontDescriptor;
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::draw::{Color, Shape};
use crate::input::{ClickHighlightSettings, InputState, board_mode::BoardMode};
use std::collections::HashMap;
use std::fs;
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
        12.0,
        0.32,
        false,
        32.0,
        FontDescriptor::default(),
        false,
        20.0,
        30.0,
        true,
        BoardConfig::default(),
        action_map,
        usize::MAX,
        ClickHighlightSettings::disabled(),
        0,
        0,
        true,
        0,
        0,
        5,
        5,
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
            fill: false,
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

#[test]
fn snapshot_preserves_history_only_frames() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-history");
    options.persist_transparent = true;
    options.persist_history = true;

    let mut input = dummy_input_state();
    let frame = input.canvas_set.active_frame_mut();
    let id = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 50,
        y2: 50,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 3.0,
    });
    let index = frame.find_index(id).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(index, frame.shape(id).unwrap().clone())],
        },
        input.undo_stack_limit,
    );
    // Undo to place the action in the redo stack and clear the visible canvas.
    frame.undo_last();

    assert!(frame.shapes.is_empty());
    assert_eq!(frame.redo_stack_len(), 1);

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
    save_snapshot(&snapshot, &options).expect("save snapshot");

    let loaded = load_snapshot(&options)
        .expect("load snapshot result")
        .expect("snapshot data");

    let mut restored = dummy_input_state();
    apply_snapshot(&mut restored, loaded, &options);
    restored.canvas_set.switch_mode(BoardMode::Transparent);
    let frame = restored.canvas_set.active_frame_mut();
    assert_eq!(frame.shapes.len(), 0);
    assert_eq!(frame.redo_stack_len(), 1);
    frame.redo_last();
    assert_eq!(frame.shapes.len(), 1);
}

#[test]
fn modify_delete_cycle_survives_restore() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-modify-delete");
    options.persist_transparent = true;
    options.persist_history = true;

    let mut input = dummy_input_state();
    let frame = input.canvas_set.active_frame_mut();
    let id = frame.add_shape(Shape::Line {
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
    let index = frame.find_index(id).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(index, frame.shape(id).unwrap().clone())],
        },
        input.undo_stack_limit,
    );

    // Modify the shape (simulate an edit).
    frame.push_undo_action(
        UndoAction::Modify {
            shape_id: id,
            before: ShapeSnapshot {
                shape: frame.shape(id).unwrap().shape.clone(),
                locked: false,
            },
            after: ShapeSnapshot {
                shape: Shape::Line {
                    x1: 0,
                    y1: 0,
                    x2: 20,
                    y2: 20,
                    color: Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    thick: 1.0,
                },
                locked: false,
            },
        },
        input.undo_stack_limit,
    );

    // Delete it (pushes a Delete with embedded shape data).
    let delete_action = frame
        .remove_shape_by_id(id)
        .map(|(idx, shape)| UndoAction::Delete {
            shapes: vec![(idx, shape)],
        });
    frame.push_undo_action(delete_action.unwrap(), input.undo_stack_limit);

    assert!(frame.shapes.is_empty());
    assert!(frame.undo_stack_len() >= 3);

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
    save_snapshot(&snapshot, &options).expect("save snapshot");

    let loaded = load_snapshot(&options)
        .expect("load snapshot result")
        .expect("snapshot data");

    let mut restored = dummy_input_state();
    apply_snapshot(&mut restored, loaded, &options);
    restored.canvas_set.switch_mode(BoardMode::Transparent);
    let frame = restored.canvas_set.active_frame_mut();

    // Undo should re-create the deleted shape, then undo the modify, restoring original coords.
    frame.undo_last(); // undo delete -> shape back (modified version)
    frame.undo_last(); // undo modify -> original version

    let restored_shape = frame.shape(id).expect("shape restored");
    if let Shape::Line { x2, y2, .. } = restored_shape.shape {
        assert_eq!((x2, y2), (10, 10));
    } else {
        panic!("Expected line shape");
    }
}

#[test]
fn clear_all_can_be_undone_after_restore() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-clear-all");
    options.persist_transparent = true;
    options.persist_history = true;

    let mut input = dummy_input_state();
    let frame = input.canvas_set.active_frame_mut();
    for i in 0..3 {
        frame.add_shape(Shape::Rect {
            x: i * 10,
            y: i * 10,
            w: 5,
            h: 5,
            fill: false,
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 1.0,
        });
    }

    assert_eq!(frame.shapes.len(), 3);
    assert!(input.clear_all());
    assert_eq!(input.canvas_set.active_frame().shapes.len(), 0);
    assert!(input.canvas_set.active_frame().undo_stack_len() > 0);

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
    save_snapshot(&snapshot, &options).expect("save snapshot");
    let loaded = load_snapshot(&options)
        .expect("load snapshot result")
        .expect("snapshot data");

    let mut restored = dummy_input_state();
    apply_snapshot(&mut restored, loaded, &options);
    restored.canvas_set.switch_mode(BoardMode::Transparent);
    let frame = restored.canvas_set.active_frame_mut();
    assert_eq!(frame.shapes.len(), 0);
    assert!(frame.undo_stack_len() > 0);
    frame.undo_last();
    assert_eq!(frame.shapes.len(), 3);
}

#[test]
fn corrupted_history_is_dropped_but_shapes_load() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-corrupt");
    options.persist_transparent = true;
    options.persist_history = true;

    let mut input = dummy_input_state();
    let frame = input.canvas_set.active_frame_mut();
    let id = frame.add_shape(Shape::Line {
        x1: 1,
        y1: 1,
        x2: 2,
        y2: 2,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.5,
    });
    let index = frame.find_index(id).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(index, frame.shape(id).unwrap().clone())],
        },
        input.undo_stack_limit,
    );

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
    save_snapshot(&snapshot, &options).expect("save snapshot");

    // Corrupt the undo stack to force deserialization failure.
    let path = options.session_file_path();
    let data = fs::read_to_string(&path).expect("read session file");
    let corrupted = data.replace(
        "\"undo_stack\": [",
        "\"undo_stack\": [{\"kind\": \"compound\", \"garbage\": true},",
    );
    fs::write(&path, corrupted).expect("write corrupt session file");

    let loaded = load_snapshot(&options)
        .expect("load snapshot result")
        .expect("snapshot present after dropping history");

    let mut restored = dummy_input_state();
    apply_snapshot(&mut restored, loaded, &options);
    restored.canvas_set.switch_mode(BoardMode::Transparent);
    let frame = restored.canvas_set.active_frame();
    assert_eq!(frame.shapes.len(), 1);
    assert_eq!(frame.undo_stack_len(), 0);
    assert_eq!(frame.redo_stack_len(), 0);
}
