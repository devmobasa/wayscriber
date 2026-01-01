use super::super::*;
use super::helpers::dummy_input_state;
use crate::draw::{Color, Shape};
use crate::input::EraserMode;
use crate::input::board_mode::BoardMode;
use std::fs;

#[test]
fn save_snapshot_skips_when_payload_exceeds_max_file_size() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-too-big");
    options.persist_transparent = false;
    options.persist_whiteboard = false;
    options.persist_blackboard = false;
    options.persist_history = false;
    options.restore_tool_state = true;
    options.max_file_size_bytes = 1; // smaller than any valid JSON payload
    options.backup_retention = 0;

    let snapshot = SessionSnapshot {
        active_mode: BoardMode::Transparent,
        transparent: None,
        whiteboard: None,
        blackboard: None,
        tool_state: Some(ToolStateSnapshot {
            current_color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            current_thickness: 3.0,
            eraser_size: 12.0,
            eraser_kind: crate::draw::EraserKind::Circle,
            eraser_mode: EraserMode::Brush,
            marker_opacity: Some(0.32),
            fill_enabled: Some(false),
            tool_override: None,
            current_font_size: 24.0,
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: Some(false),
            board_previous_color: None,
            show_status_bar: true,
        }),
    };

    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed even when skipping");

    let session_path = options.session_file_path();
    assert!(
        !session_path.exists(),
        "session file should not be created when payload exceeds max_file_size_bytes"
    );
}

#[test]
fn load_snapshot_refuses_file_larger_than_max() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-large-file");
    options.persist_transparent = true;
    options.max_file_size_bytes = 8; // very small

    let session_path = options.session_file_path();
    // Create a file larger than the configured max size
    fs::write(
        &session_path,
        b"this file is definitely larger than 8 bytes",
    )
    .unwrap();

    let loaded = load_snapshot(&options).expect("load_snapshot should not error");
    assert!(
        loaded.is_none(),
        "snapshot should not be loaded when file exceeds max_file_size_bytes"
    );
}

#[test]
fn load_snapshot_truncates_shapes_when_exceeding_max_shapes_per_frame() {
    let temp = tempfile::tempdir().unwrap();
    let mut save_options = SessionOptions::new(temp.path().to_path_buf(), "display-shape-limit");
    save_options.persist_transparent = true;

    let mut input = dummy_input_state();
    {
        let frame = input.canvas_set.active_frame_mut();
        for i in 0..5 {
            frame.add_shape(Shape::Rect {
                x: i * 10,
                y: i * 10,
                w: 5,
                h: 5,
                fill: false,
                color: Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                thick: 1.0,
            });
        }
    }

    let snapshot = snapshot_from_input(&input, &save_options).expect("snapshot present");
    save_snapshot(&snapshot, &save_options).expect("save_snapshot should succeed");

    // Reload with a stricter max_shapes_per_frame limit.
    let mut load_options = SessionOptions::new(temp.path().to_path_buf(), "display-shape-limit");
    load_options.persist_transparent = true;
    load_options.max_shapes_per_frame = 2;

    let loaded = load_snapshot(&load_options)
        .expect("load_snapshot should succeed")
        .expect("snapshot should be present after load");

    let transparent = loaded
        .transparent
        .expect("transparent frame should be present");
    assert_eq!(
        transparent.pages[0].shapes.len(),
        2,
        "frame should be truncated to max_shapes_per_frame"
    );
}
