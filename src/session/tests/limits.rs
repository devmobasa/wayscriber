use super::super::*;
use super::helpers::dummy_input_state;
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::draw::{Color, FontDescriptor, Shape};
use crate::input::EraserMode;
use std::fs;
use std::path::Path;

#[test]
fn save_snapshot_errors_when_payload_exceeds_max_file_size() {
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
        active_board_id: "transparent".to_string(),
        boards: Vec::new(),
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
            font_descriptor: Some(FontDescriptor::default()),
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: Some(false),
            arrow_label_enabled: Some(false),
            board_previous_color: None,
            show_status_bar: true,
            tool_settings: None,
        }),
    };

    let err = save_snapshot(&snapshot, &options).expect_err("oversize snapshot should fail");
    assert!(
        err.to_string().contains("exceeds the configured limit"),
        "unexpected error: {err:#}"
    );

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
        let frame = input.boards.active_frame_mut();
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
        .boards
        .iter()
        .find(|board| board.id == "transparent")
        .expect("transparent board should be present");
    assert_eq!(
        transparent.pages.pages[0].shapes.len(),
        2,
        "frame should be truncated to max_shapes_per_frame"
    );
}

#[test]
fn save_snapshot_drops_history_when_modified_stroke_exceeds_limit() {
    let temp = tempfile::tempdir().unwrap();
    let mut input = dummy_input_state();
    let point_count = 1_500;

    {
        let frame = input.boards.active_frame_mut();
        let id = frame.add_shape(large_freehand(point_count, 0));
        let current = frame.shape(id).expect("shape should exist");
        let before = ShapeSnapshot {
            shape: current.shape.clone(),
            locked: current.locked,
        };
        let after_shape = large_freehand(point_count, 1);
        frame.shape_mut(id).expect("shape should exist").shape = after_shape.clone();
        frame.push_undo_action(
            UndoAction::Modify {
                shape_id: id,
                before,
                after: ShapeSnapshot {
                    shape: after_shape,
                    locked: false,
                },
            },
            input.undo_stack_limit,
        );
    }

    let mut full_options = limit_test_options(temp.path(), "display-full", true);
    full_options.max_file_size_bytes = u64::MAX;
    let full_snapshot = snapshot_from_input(&input, &full_options).expect("snapshot present");
    save_snapshot(&full_snapshot, &full_options).expect("full save should fit measuring cap");
    let full_size = fs::metadata(full_options.session_file_path())
        .expect("full session metadata")
        .len();

    let mut visible_options = limit_test_options(temp.path(), "display-visible", false);
    visible_options.max_file_size_bytes = u64::MAX;
    let visible_snapshot =
        snapshot_from_input(&input, &visible_options).expect("visible snapshot present");
    save_snapshot(&visible_snapshot, &visible_options)
        .expect("visible-only save should fit measuring cap");
    let visible_size = fs::metadata(visible_options.session_file_path())
        .expect("visible session metadata")
        .len();
    assert!(
        full_size > visible_size,
        "history should make the session larger than visible-only data"
    );

    let mut options = limit_test_options(temp.path(), "display-fallback", true);
    options.max_file_size_bytes = visible_size + (full_size - visible_size) / 2;
    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");

    save_snapshot(&snapshot, &options).expect("save should drop oversized history");

    let saved_size = fs::metadata(options.session_file_path())
        .expect("fallback session metadata")
        .len();
    assert!(
        saved_size <= options.max_file_size_bytes,
        "saved visible-only session should fit the configured limit"
    );

    let loaded = load_snapshot(&options)
        .expect("load_snapshot should succeed")
        .expect("snapshot should be present");
    let transparent = loaded
        .boards
        .iter()
        .find(|board| board.id == "transparent")
        .expect("transparent board should be present");
    let frame = &transparent.pages.pages[0];

    assert_eq!(frame.shapes.len(), 1, "visible stroke should be saved");
    assert_eq!(
        frame.undo_stack_len(),
        0,
        "oversized undo history is dropped"
    );
    assert_eq!(
        frame.redo_stack_len(),
        0,
        "oversized redo history is dropped"
    );

    match &frame.shapes[0].shape {
        Shape::Freehand { points, .. } => {
            assert_eq!(points.len(), point_count);
            assert_eq!(points[0], (1, 1));
        }
        other => panic!("expected freehand stroke, got {other:?}"),
    }
}

#[test]
fn save_snapshot_keeps_largest_recent_history_depth_that_fits() {
    let temp = tempfile::tempdir().unwrap();
    let mut input = dummy_input_state();
    let point_count = 600;

    {
        let frame = input.boards.active_frame_mut();
        let id = frame.add_shape(large_freehand(point_count, 0));
        for offset in 1..=3 {
            let current = frame.shape(id).expect("shape should exist");
            let before = ShapeSnapshot {
                shape: current.shape.clone(),
                locked: current.locked,
            };
            let after_shape = large_freehand(point_count, offset);
            frame.shape_mut(id).expect("shape should exist").shape = after_shape.clone();
            frame.push_undo_action(
                UndoAction::Modify {
                    shape_id: id,
                    before,
                    after: ShapeSnapshot {
                        shape: after_shape,
                        locked: false,
                    },
                },
                input.undo_stack_limit,
            );
        }
    }

    let mut depth_two_options = limit_test_options(temp.path(), "display-depth-two", true);
    depth_two_options.max_file_size_bytes = u64::MAX;
    depth_two_options.max_persisted_undo_depth = Some(2);
    let depth_two_snapshot =
        snapshot_from_input(&input, &depth_two_options).expect("depth two snapshot present");
    save_snapshot(&depth_two_snapshot, &depth_two_options).expect("depth two save should fit");
    let depth_two_size = fs::metadata(depth_two_options.session_file_path())
        .expect("depth two metadata")
        .len();

    let mut depth_three_options = limit_test_options(temp.path(), "display-depth-three", true);
    depth_three_options.max_file_size_bytes = u64::MAX;
    depth_three_options.max_persisted_undo_depth = Some(3);
    let depth_three_snapshot =
        snapshot_from_input(&input, &depth_three_options).expect("depth three snapshot present");
    save_snapshot(&depth_three_snapshot, &depth_three_options)
        .expect("depth three save should fit");
    let depth_three_size = fs::metadata(depth_three_options.session_file_path())
        .expect("depth three metadata")
        .len();
    assert!(
        depth_three_size > depth_two_size,
        "extra history depth should make the session larger"
    );

    let mut options = limit_test_options(temp.path(), "display-trimmed", true);
    options.max_file_size_bytes = depth_two_size + (depth_three_size - depth_two_size) / 2;
    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");

    save_snapshot(&snapshot, &options).expect("save should trim history to the fitting depth");

    let loaded = load_snapshot(&options)
        .expect("load_snapshot should succeed")
        .expect("snapshot should be present");
    let transparent = loaded
        .boards
        .iter()
        .find(|board| board.id == "transparent")
        .expect("transparent board should be present");
    let frame = &transparent.pages.pages[0];

    assert_eq!(frame.shapes.len(), 1, "visible stroke should be saved");
    assert_eq!(
        frame.undo_stack_len(),
        2,
        "save should keep the largest recent undo depth that fits"
    );
    assert_eq!(frame.redo_stack_len(), 0);
}

fn limit_test_options(base_dir: &Path, display_id: &str, persist_history: bool) -> SessionOptions {
    let mut options = SessionOptions::new(base_dir.to_path_buf(), display_id);
    options.persist_transparent = true;
    options.persist_history = persist_history;
    options.restore_tool_state = false;
    options.compression = CompressionMode::Off;
    options.backup_retention = 0;
    options
}

fn large_freehand(point_count: usize, offset: i32) -> Shape {
    let points = (0..point_count)
        .map(|index| {
            let x = i32::try_from(index).expect("test point count fits i32");
            (x + offset, (x % 173) + offset)
        })
        .collect();

    Shape::Freehand {
        points,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 3.0,
    }
}
