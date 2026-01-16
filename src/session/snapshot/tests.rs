use super::compression::is_gzip;
use super::load::load_snapshot_inner;
use super::types::{
    BoardPagesSnapshot, BoardSnapshot, CURRENT_VERSION, SessionFile, SessionSnapshot,
};
use super::{load_snapshot, save_snapshot};
use crate::draw::{Color, Frame, Shape};
use crate::session::options::{CompressionMode, SessionOptions};
use crate::time_utils::now_rfc3339;
use tempfile::tempdir;

fn sample_snapshot() -> SessionSnapshot {
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

    SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: vec![BoardSnapshot {
            id: "transparent".to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![frame],
                active: 0,
            },
        }],
        tool_state: None,
    }
}

#[test]
fn save_snapshot_respects_auto_compression_threshold() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut plain = SessionOptions::new(temp.path().join("plain"), "plain");
    plain.persist_transparent = true;
    plain.compression = CompressionMode::Auto;
    plain.auto_compress_threshold_bytes = u64::MAX;
    save_snapshot(&snapshot, &plain).expect("save_snapshot should succeed");
    let plain_bytes = std::fs::read(plain.session_file_path()).unwrap();
    assert!(
        !is_gzip(&plain_bytes),
        "expected uncompressed session payload"
    );

    let mut compressed = SessionOptions::new(temp.path().join("compressed"), "compressed");
    compressed.persist_transparent = true;
    compressed.compression = CompressionMode::Auto;
    compressed.auto_compress_threshold_bytes = 1;
    save_snapshot(&snapshot, &compressed).expect("save_snapshot should succeed");
    let compressed_bytes = std::fs::read(compressed.session_file_path()).unwrap();
    assert!(is_gzip(&compressed_bytes), "expected gzip payload");
}

#[test]
fn load_snapshot_inner_reports_compression_and_version() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "compressed");
    options.persist_transparent = true;
    options.compression = CompressionMode::On;
    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

    let loaded = load_snapshot_inner(&options.session_file_path(), &options)
        .expect("load_snapshot_inner should succeed")
        .expect("snapshot should be present");
    assert!(loaded.compressed);
    assert_eq!(loaded.version, CURRENT_VERSION);
    assert!(
        loaded
            .snapshot
            .boards
            .iter()
            .any(|board| board.id == "transparent")
    );
}

#[test]
fn load_snapshot_inner_skips_newer_versions() {
    let temp = tempdir().unwrap();
    let session_path = temp.path().join("session.json");

    let file = SessionFile {
        version: CURRENT_VERSION + 1,
        last_modified: now_rfc3339(),
        active_board_id: Some("transparent".to_string()),
        active_mode: None,
        boards: Vec::new(),
        transparent: None,
        whiteboard: None,
        blackboard: None,
        transparent_pages: None,
        whiteboard_pages: None,
        blackboard_pages: None,
        transparent_active_page: None,
        whiteboard_active_page: None,
        blackboard_active_page: None,
        tool_state: None,
    };
    let bytes = serde_json::to_vec_pretty(&file).unwrap();
    std::fs::write(&session_path, bytes).unwrap();

    let options = SessionOptions::new(temp.path().to_path_buf(), "skip");
    let loaded =
        load_snapshot_inner(&session_path, &options).expect("load_snapshot_inner should work");
    assert!(loaded.is_none());
}

#[test]
fn save_snapshot_preserves_multiple_pages() {
    let temp = tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "multi");
    options.persist_transparent = true;

    let mut first = Frame::new();
    first.add_shape(Shape::Line {
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

    let mut second = Frame::new();
    second.add_shape(Shape::Rect {
        x: 5,
        y: 5,
        w: 8,
        h: 8,
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
        active_board_id: "transparent".to_string(),
        boards: vec![BoardSnapshot {
            id: "transparent".to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![first, second],
                active: 1,
            },
        }],
        tool_state: None,
    };

    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

    let loaded = load_snapshot(&options)
        .expect("load_snapshot should succeed")
        .expect("snapshot should be present");
    let pages = loaded
        .boards
        .iter()
        .find(|board| board.id == "transparent")
        .expect("transparent pages should be present");
    assert_eq!(pages.pages.pages.len(), 2);
    assert_eq!(pages.pages.active, 1);
    assert_eq!(pages.pages.pages[0].shapes.len(), 1);
    assert_eq!(pages.pages.pages[1].shapes.len(), 1);
}

#[test]
fn save_snapshot_keeps_empty_pages() {
    let temp = tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "empty-pages");
    options.persist_transparent = true;

    let snapshot = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: vec![BoardSnapshot {
            id: "transparent".to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![Frame::new(), Frame::new(), Frame::new()],
                active: 2,
            },
        }],
        tool_state: None,
    };

    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

    let loaded = load_snapshot(&options)
        .expect("load_snapshot should succeed")
        .expect("snapshot should be present");
    let pages = loaded
        .boards
        .iter()
        .find(|board| board.id == "transparent")
        .expect("transparent pages should be present");
    assert_eq!(pages.pages.pages.len(), 3);
    assert_eq!(pages.pages.active, 2);
}

#[test]
fn load_snapshot_inner_migrates_legacy_frame_to_pages() {
    let temp = tempdir().unwrap();
    let session_path = temp.path().join("session.json");

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

    let file = SessionFile {
        version: CURRENT_VERSION,
        last_modified: now_rfc3339(),
        active_board_id: None,
        active_mode: Some("transparent".to_string()),
        boards: Vec::new(),
        transparent: Some(frame),
        whiteboard: None,
        blackboard: None,
        transparent_pages: None,
        whiteboard_pages: None,
        blackboard_pages: None,
        transparent_active_page: None,
        whiteboard_active_page: None,
        blackboard_active_page: None,
        tool_state: None,
    };
    let bytes = serde_json::to_vec_pretty(&file).unwrap();
    std::fs::write(&session_path, bytes).unwrap();

    let options = SessionOptions::new(temp.path().to_path_buf(), "legacy");
    let loaded = load_snapshot_inner(&session_path, &options)
        .expect("load_snapshot_inner should succeed")
        .expect("snapshot should be present");
    let pages = loaded
        .snapshot
        .boards
        .iter()
        .find(|board| board.id == "transparent")
        .expect("transparent pages");
    assert_eq!(pages.pages.pages.len(), 1);
    assert_eq!(pages.pages.active, 0);
    assert_eq!(pages.pages.pages[0].shapes.len(), 1);
}
