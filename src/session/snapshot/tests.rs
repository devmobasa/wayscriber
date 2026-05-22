use super::compression::is_gzip;
use super::load::{
    LoadSnapshotOutcome, load_snapshot_inner, load_snapshot_inner_with_expanded_limit,
    load_snapshot_with_expanded_limit,
};
use super::save::save_snapshot_with_expanded_limit;
use super::types::{
    BoardFile, BoardPagesSnapshot, BoardSnapshot, CURRENT_VERSION, SessionFile, SessionSnapshot,
};
use super::{load_snapshot, save_snapshot};
use crate::draw::{Color, Frame, Shape};
use crate::session::options::{CompressionMode, SessionOptions};
use crate::test_temp::tempdir;
use crate::time_utils::now_rfc3339;

fn sample_frame() -> Frame {
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
    frame
}

fn sample_snapshot() -> SessionSnapshot {
    SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: vec![BoardSnapshot {
            id: "transparent".to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![sample_frame()],
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
fn load_snapshot_inner_refuses_compressed_payload_over_expanded_limit() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "expanded-inner");
    options.persist_transparent = true;
    options.compression = CompressionMode::On;
    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

    let Err(err) =
        load_snapshot_inner_with_expanded_limit(&options.session_file_path(), &options, 16)
    else {
        panic!("expanded payload should exceed the test cap");
    };
    assert!(
        err.to_string().contains("exceeds the safety limit"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn load_snapshot_expansion_limit_leaves_primary_file_unchanged() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "expanded-outer");
    options.persist_transparent = true;
    options.compression = CompressionMode::On;
    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

    let session_path = options.session_file_path();
    let original_bytes = std::fs::read(&session_path).expect("session bytes");
    let outcome = load_snapshot_with_expanded_limit(&options, 16)
        .expect("expanded-cap refusal should not be a load error");

    assert!(matches!(
        outcome,
        LoadSnapshotOutcome::ExpandedTooLarge {
            max_expanded_size: 16,
            ..
        }
    ));
    assert_eq!(
        std::fs::read(&session_path).expect("session should remain in place"),
        original_bytes
    );
    assert!(
        !options.backup_file_path().exists(),
        "expanded-cap refusal should not rotate the primary session into backup"
    );
}

#[test]
fn load_snapshot_reports_successful_recovery_source() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "valid-recovery");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");

    let recovery_file = SessionFile {
        version: CURRENT_VERSION,
        last_modified: now_rfc3339(),
        active_board_id: Some("transparent".to_string()),
        active_mode: None,
        boards: vec![BoardFile {
            id: "transparent".to_string(),
            pages: vec![sample_frame()],
            active_page: 0,
        }],
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
    std::fs::write(
        options.recovery_file_path(),
        serde_json::to_vec_pretty(&recovery_file).expect("recovery json"),
    )
    .expect("recovery write");

    let outcome =
        load_snapshot_with_expanded_limit(&options, 64 * 1024).expect("valid recovery should load");
    assert!(
        matches!(outcome, LoadSnapshotOutcome::LoadedFromRecovery(_)),
        "valid recovery should be surfaced in the load outcome"
    );
}

#[test]
fn load_snapshot_falls_back_to_normal_when_recovery_is_corrupt() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "corrupt-recovery");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");

    let recovery_path = options.recovery_file_path();
    std::fs::write(&recovery_path, b"{not valid json").expect("recovery write");

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("corrupt recovery should fall back to normal session");
    assert_loaded_sample_snapshot(outcome);
    assert!(
        !recovery_path.exists(),
        "corrupt recovery should be moved out of the recovery path"
    );
    assert!(
        recovery_path.with_extension("recovery.bak").exists(),
        "corrupt recovery should be backed up for inspection"
    );
}

#[test]
fn load_snapshot_falls_back_to_normal_when_recovery_is_empty() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "empty-recovery");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");

    let empty_file = SessionFile {
        version: CURRENT_VERSION,
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
    let recovery_path = options.recovery_file_path();
    std::fs::write(
        &recovery_path,
        serde_json::to_vec_pretty(&empty_file).expect("empty recovery json"),
    )
    .expect("recovery write");

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("empty recovery should fall back to normal session");
    assert_loaded_sample_snapshot(outcome);
    assert!(
        !recovery_path.exists(),
        "empty recovery should be moved out of the recovery path"
    );
    assert!(
        recovery_path.with_extension("recovery.empty").exists(),
        "empty recovery should be preserved for inspection"
    );
}

#[test]
fn load_snapshot_rejects_oversized_plain_recovery_before_falling_back() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    const MAX_EXPANDED_SIZE: u64 = 16 * 1024;

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "huge-plain-recovery");
    options.persist_transparent = true;
    options.max_file_size_bytes = u64::MAX;
    save_snapshot(&snapshot, &options).expect("normal session should save");

    let recovery_path = options.recovery_file_path();
    std::fs::write(
        &recovery_path,
        vec![b' '; usize::try_from(MAX_EXPANDED_SIZE + 1).expect("test size fits")],
    )
    .expect("recovery write");

    let outcome = load_snapshot_with_expanded_limit(&options, MAX_EXPANDED_SIZE)
        .expect("oversized plain recovery should fall back to normal session");
    assert_loaded_sample_snapshot(outcome);
    assert!(
        !recovery_path.exists(),
        "oversized recovery should be moved out of the recovery path"
    );
    assert!(
        recovery_path.with_extension("recovery.too-large").exists(),
        "oversized recovery should be preserved for inspection"
    );
}

#[test]
fn save_snapshot_refuses_compressed_payload_over_expanded_limit() {
    let temp = tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "expanded-save");
    options.persist_transparent = true;
    options.compression = CompressionMode::On;
    options.max_file_size_bytes = u64::MAX;

    let mut frame = Frame::new();
    frame.add_shape(Shape::Text {
        x: 1,
        y: 2,
        text: "x".repeat(4096),
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        size: 24.0,
        font_descriptor: Default::default(),
        background_enabled: false,
        wrap_width: None,
    });
    let snapshot = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: vec![BoardSnapshot {
            id: "transparent".to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![frame],
                active: 0,
            },
        }],
        tool_state: None,
    };

    let err = save_snapshot_with_expanded_limit(&snapshot, &options, 512)
        .expect_err("compressed raw payload over expanded cap should not be written");
    assert!(
        err.to_string().contains("load safety limit"),
        "unexpected error: {err:#}"
    );
    assert!(
        !options.session_file_path().exists(),
        "unloadable compressed session should not be created"
    );
}

fn assert_loaded_sample_snapshot(outcome: LoadSnapshotOutcome) {
    let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
        panic!("expected normal session to load, got {outcome:?}");
    };
    assert_eq!(snapshot.boards.len(), 1);
    assert_eq!(snapshot.boards[0].id, "transparent");
    assert_eq!(snapshot.boards[0].pages.pages.len(), 1);
    assert_eq!(snapshot.boards[0].pages.pages[0].shapes.len(), 1);
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

#[test]
fn load_snapshot_inner_falls_back_when_active_board_is_missing() {
    let temp = tempdir().unwrap();
    let session_path = temp.path().join("session.json");

    let file = SessionFile {
        version: CURRENT_VERSION,
        last_modified: now_rfc3339(),
        active_board_id: Some("missing".to_string()),
        active_mode: None,
        boards: vec![BoardFile {
            id: "transparent".to_string(),
            pages: vec![sample_frame()],
            active_page: 0,
        }],
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

    let options = SessionOptions::new(temp.path().to_path_buf(), "missing-active-board");
    let loaded = load_snapshot_inner(&session_path, &options)
        .expect("load_snapshot_inner should succeed")
        .expect("snapshot should be present");

    assert_eq!(loaded.snapshot.active_board_id, "transparent");
}

#[test]
fn load_snapshot_inner_normalizes_empty_legacy_page_lists() {
    let temp = tempdir().unwrap();
    let session_path = temp.path().join("session.json");

    let file = SessionFile {
        version: CURRENT_VERSION,
        last_modified: now_rfc3339(),
        active_board_id: None,
        active_mode: Some("transparent".to_string()),
        boards: Vec::new(),
        transparent: None,
        whiteboard: None,
        blackboard: None,
        transparent_pages: Some(Vec::new()),
        whiteboard_pages: Some(vec![sample_frame()]),
        blackboard_pages: None,
        transparent_active_page: Some(99),
        whiteboard_active_page: Some(0),
        blackboard_active_page: None,
        tool_state: None,
    };
    let bytes = serde_json::to_vec_pretty(&file).unwrap();
    std::fs::write(&session_path, bytes).unwrap();

    let options = SessionOptions::new(temp.path().to_path_buf(), "empty-legacy-pages");
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
}
