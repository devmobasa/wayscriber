use super::super::*;
use super::helpers::dummy_input_state;
use crate::draw::{Color, FontDescriptor, Frame, Shape};
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};
use crate::session::snapshot::BoardSnapshot;
use std::fs;

#[test]
fn session_roundtrip_preserves_shapes_across_frames() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-2");
    options.persist_transparent = true;
    options.persist_whiteboard = true;
    options.persist_blackboard = true;
    options.set_output_identity(Some("HDMI-1"));

    let mut input = dummy_input_state();
    input.boards.active_frame_mut().add_shape(Shape::Line {
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

    input.switch_board(BOARD_ID_WHITEBOARD);
    input.boards.active_frame_mut().add_shape(Shape::Text {
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
        wrap_width: None,
    });

    input.switch_board(BOARD_ID_BLACKBOARD);
    input.boards.active_frame_mut().add_shape(Shape::Ellipse {
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

    fresh_input.switch_board(BOARD_ID_TRANSPARENT);
    assert_eq!(fresh_input.boards.active_frame().shapes.len(), 1);

    fresh_input.switch_board(BOARD_ID_WHITEBOARD);
    assert_eq!(fresh_input.boards.active_frame().shapes.len(), 1);

    fresh_input.switch_board(BOARD_ID_BLACKBOARD);
    assert_eq!(fresh_input.boards.active_frame().shapes.len(), 1);
}

#[test]
fn save_snapshot_rotates_backup_when_enabled() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-backup");
    options.persist_transparent = true;
    options.backup_retention = 1;

    let session_path = options.session_file_path();
    fs::write(&session_path, b"old-session").expect("write old session");

    let mut frame = Frame::new();
    frame.add_shape(Shape::Line {
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

    let snapshot = SessionSnapshot {
        active_board_id: BOARD_ID_TRANSPARENT.to_string(),
        boards: vec![BoardSnapshot {
            id: BOARD_ID_TRANSPARENT.to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![frame],
                active: 0,
            },
        }],
        tool_state: None,
    };

    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

    let backup_path = options.backup_file_path();
    let backup = fs::read(&backup_path).expect("backup file present");
    assert_eq!(backup, b"old-session");
    let current = fs::read(&session_path).expect("session file present");
    assert_ne!(current, b"old-session");
}

#[test]
fn save_snapshot_skips_backup_when_disabled() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-no-backup");
    options.persist_transparent = true;
    options.backup_retention = 0;

    let session_path = options.session_file_path();
    fs::write(&session_path, b"old-session").expect("write old session");

    let mut frame = Frame::new();
    frame.add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 5,
        h: 5,
        fill: false,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    });

    let snapshot = SessionSnapshot {
        active_board_id: BOARD_ID_TRANSPARENT.to_string(),
        boards: vec![BoardSnapshot {
            id: BOARD_ID_TRANSPARENT.to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![frame],
                active: 0,
            },
        }],
        tool_state: None,
    };

    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");
    assert!(!options.backup_file_path().exists());
    assert!(session_path.exists());
}

#[test]
fn corrupt_session_is_backed_up_and_reset() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-bad");
    options.persist_transparent = true;

    let session_path = options.session_file_path();
    fs::write(&session_path, b"not json").expect("write corrupt session file");

    let loaded = load_snapshot(&options).expect("load should not error");
    assert!(loaded.is_none());

    let backup_path = options.backup_file_path();
    let backup = fs::read(&backup_path).expect("backup file present");
    assert_eq!(backup, b"not json");
    assert!(
        !session_path.exists(),
        "corrupt session file should be removed after backup"
    );
}
