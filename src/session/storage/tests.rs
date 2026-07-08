use super::{ClearToolStateOutcome, clear_session, clear_tool_state, inspect_session};
use crate::draw::{Color, FontDescriptor, Frame, Shape};
use crate::session::snapshot::{BoardPagesSnapshot, BoardSnapshot};
use crate::session::{
    CompressionMode, SessionOptions, SessionSnapshot, ToolStateSnapshot, save_snapshot,
};
#[cfg(unix)]
use std::{
    ffi::CString,
    os::unix::{ffi::OsStrExt, fs::symlink},
};

fn transparent_line_snapshot() -> SessionSnapshot {
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

fn sample_tool_state() -> ToolStateSnapshot {
    ToolStateSnapshot {
        current_color: Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
        current_thickness: 3.0,
        eraser_size: 12.0,
        eraser_kind: crate::draw::EraserKind::Circle,
        eraser_mode: crate::input::EraserMode::Brush,
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
        polygon_sides: crate::draw::REGULAR_POLYGON_DEFAULT_SIDES,
        board_previous_color: None,
        show_status_bar: true,
        tool_settings: None,
    }
}

#[test]
fn clear_session_removes_all_variants_for_prefix() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-1");
    options.per_output = true;

    let base_dir = &options.base_dir;
    std::fs::write(base_dir.join("session-display_1.json"), b"{}").unwrap();
    std::fs::write(base_dir.join("session-display_1-DP_1.json"), b"{}").unwrap();
    std::fs::write(base_dir.join("session-display_1.json.cleared"), b"{}").unwrap();
    std::fs::write(base_dir.join("session-display_1-DP_1.json.cleared"), b"{}").unwrap();
    std::fs::write(base_dir.join("session-display_1.json.bak"), b"{}").unwrap();
    std::fs::write(base_dir.join("session-display_1-DP_1.json.bak"), b"{}").unwrap();
    std::fs::write(
        base_dir.join("session-display_1.json.bak.recoverable"),
        b"{}",
    )
    .unwrap();
    std::fs::write(
        base_dir.join("session-display_1-DP_1.json.bak.recoverable"),
        b"{}",
    )
    .unwrap();
    std::fs::write(base_dir.join("session-display_1.json.recovery"), b"{}").unwrap();
    std::fs::write(
        base_dir.join("session-display_1.json.recovery.recoverable"),
        b"{}",
    )
    .unwrap();
    std::fs::write(
        base_dir.join("session-display_1.json.recovery.empty"),
        b"{}",
    )
    .unwrap();
    std::fs::write(base_dir.join("session-display_1-DP_1.json.recovery"), b"{}").unwrap();
    std::fs::write(base_dir.join("session-display_1.lock"), b"").unwrap();
    std::fs::write(base_dir.join("session-display_1-DP_1.lock"), b"").unwrap();

    let outcome = clear_session(&options).expect("clear_session should succeed");
    assert!(
        outcome.removed_session,
        "at least one session file variant should be removed"
    );
    assert!(
        outcome.removed_backup,
        "at least one backup variant should be removed"
    );
    assert!(
        outcome.removed_recovery,
        "at least one recovery variant should be removed"
    );
    assert!(
        outcome.removed_lock,
        "at least one lock variant should be removed"
    );

    // The main, non-per-output files should always be gone.
    assert!(
        !base_dir.join("session-display_1.json").exists(),
        "primary session file should be removed"
    );
    assert!(
        !base_dir.join("session-display_1.json.cleared").exists(),
        "primary clear marker should be removed"
    );
    assert!(
        !base_dir
            .join("session-display_1-DP_1.json.cleared")
            .exists(),
        "per-output clear marker should be removed"
    );
    assert!(
        !base_dir.join("session-display_1.json.bak").exists(),
        "primary backup file should be removed"
    );
    assert!(
        !base_dir
            .join("session-display_1.json.bak.recoverable")
            .exists(),
        "primary backup recovery marker should be removed"
    );
    assert!(
        !base_dir.join("session-display_1.json.recovery").exists(),
        "primary recovery file should be removed"
    );
    assert!(
        !base_dir
            .join("session-display_1.json.recovery.recoverable")
            .exists(),
        "primary recovery recoverable marker should be removed"
    );
    assert!(
        !base_dir
            .join("session-display_1.json.recovery.empty")
            .exists(),
        "preserved primary recovery file should be removed"
    );
    assert!(
        !base_dir
            .join("session-display_1-DP_1.json.recovery")
            .exists(),
        "per-output recovery file should be removed even when the primary recovery existed"
    );
    assert!(
        !base_dir.join("session-display_1.lock").exists(),
        "primary lock file should be removed"
    );

    // Per-output variants may or may not be removed depending on identity resolution,
    // so we only assert that the primary files are gone.
}

#[test]
fn clear_session_keeps_neighbor_display_prefix_variants() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "wayland-1");
    options.per_output = true;

    let base_dir = &options.base_dir;
    for name in [
        "session-wayland_1.json",
        "session-wayland_1-DP_1.json",
        "session-wayland_1.json.cleared",
        "session-wayland_1-DP_1.json.cleared",
        "session-wayland_1.json.bak",
        "session-wayland_1-DP_1.json.bak",
        "session-wayland_1.json.bak.recoverable",
        "session-wayland_1-DP_1.json.bak.recoverable",
        "session-wayland_1.json.recovery",
        "session-wayland_1.json.recovery.recoverable",
        "session-wayland_1.json.recovery.empty",
        "session-wayland_1-DP_1.json.recovery.too-large",
        "session-wayland_1.lock",
        "session-wayland_1-DP_1.lock",
        "session-wayland_10.json",
        "session-wayland_10-DP_1.json",
        "session-wayland_10.json.cleared",
        "session-wayland_10.json.bak",
        "session-wayland_10.json.bak.recoverable",
        "session-wayland_10.json.recovery",
        "session-wayland_10.json.recovery.recoverable",
        "session-wayland_10.json.recovery.empty",
        "session-wayland_10.lock",
    ] {
        std::fs::write(base_dir.join(name), b"{}").unwrap();
    }

    clear_session(&options).expect("clear_session should succeed");

    for removed in [
        "session-wayland_1.json",
        "session-wayland_1-DP_1.json",
        "session-wayland_1.json.cleared",
        "session-wayland_1-DP_1.json.cleared",
        "session-wayland_1.json.bak",
        "session-wayland_1-DP_1.json.bak",
        "session-wayland_1.json.bak.recoverable",
        "session-wayland_1-DP_1.json.bak.recoverable",
        "session-wayland_1.json.recovery",
        "session-wayland_1.json.recovery.recoverable",
        "session-wayland_1.json.recovery.empty",
        "session-wayland_1-DP_1.json.recovery.too-large",
        "session-wayland_1.lock",
        "session-wayland_1-DP_1.lock",
    ] {
        assert!(
            !base_dir.join(removed).exists(),
            "{removed} should be removed"
        );
    }

    for preserved in [
        "session-wayland_10.json",
        "session-wayland_10-DP_1.json",
        "session-wayland_10.json.cleared",
        "session-wayland_10.json.bak",
        "session-wayland_10.json.bak.recoverable",
        "session-wayland_10.json.recovery",
        "session-wayland_10.json.recovery.recoverable",
        "session-wayland_10.json.recovery.empty",
        "session-wayland_10.lock",
    ] {
        assert!(
            base_dir.join(preserved).exists(),
            "{preserved} should not be removed"
        );
    }
}

#[test]
fn inspect_session_reports_counts_and_flags() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-inspect");
    options.persist_transparent = true;
    options.persist_whiteboard = false;
    options.persist_blackboard = false;
    options.persist_history = true;
    options.restore_tool_state = true;
    options.compression = CompressionMode::On;

    // Build a simple snapshot with one transparent shape and tool state.
    let mut frame = Frame::new();
    frame.add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: Color {
            r: 1.0,
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
                pages: vec![frame],
                active: 0,
            },
        }],
        tool_state: Some(sample_tool_state()),
    };

    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

    let inspection = inspect_session(&options).expect("inspect_session should succeed");
    assert!(inspection.exists);
    assert!(inspection.session_path.exists());

    let counts = inspection
        .frame_counts
        .expect("frame_counts should be populated");
    assert_eq!(counts.transparent, 1);
    assert_eq!(counts.whiteboard, 0);
    assert_eq!(counts.blackboard, 0);

    let history_counts = inspection
        .history_counts
        .expect("history_counts should be populated");
    assert_eq!(history_counts.transparent.undo, 0);
    assert_eq!(history_counts.transparent.redo, 0);
    assert!(!inspection.history_present);

    assert!(inspection.tool_state_present);
    assert!(inspection.compressed);
    assert!(inspection.persist_transparent);
    assert!(!inspection.persist_whiteboard);
    assert!(!inspection.persist_blackboard);
    assert!(inspection.persist_history);
}

#[test]
fn clear_tool_state_preserves_board_data() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-clear-tool");
    options.persist_transparent = true;
    options.persist_history = true;
    options.restore_tool_state = true;

    let mut snapshot = transparent_line_snapshot();
    snapshot.tool_state = Some(sample_tool_state());
    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

    let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");
    assert_eq!(
        outcome,
        ClearToolStateOutcome::Cleared {
            preserved_board_data: true
        }
    );

    let loaded = crate::session::load_snapshot(&options)
        .expect("load should succeed")
        .expect("snapshot should remain");
    assert!(loaded.tool_state.is_none(), "tool state should be removed");
    assert_eq!(loaded.boards.len(), 1);
    assert_eq!(loaded.boards[0].pages.pages[0].shapes.len(), 1);
}

#[test]
fn clear_tool_state_rewrites_compressed_session() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-compressed");
    options.persist_transparent = true;
    options.restore_tool_state = true;
    options.compression = CompressionMode::On;

    let mut snapshot = transparent_line_snapshot();
    snapshot.tool_state = Some(sample_tool_state());
    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");
    assert!(
        inspect_session(&options)
            .expect("inspect should succeed")
            .compressed
    );

    let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");
    assert_eq!(
        outcome,
        ClearToolStateOutcome::Cleared {
            preserved_board_data: true
        }
    );

    let inspection = inspect_session(&options).expect("inspect should succeed");
    assert!(inspection.exists);
    assert!(inspection.compressed);
    assert!(!inspection.tool_state_present);
    assert_eq!(
        inspection
            .frame_counts
            .expect("frame counts should be present")
            .transparent,
        1
    );
}

#[test]
fn clear_tool_state_without_output_identity_clears_all_per_output_variants() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-1");
    options.persist_transparent = true;
    options.persist_history = true;
    options.restore_tool_state = true;
    options.per_output = true;

    let mut first_output = options.clone();
    first_output.set_output_identity(Some("DP_1"));
    let mut first_snapshot = transparent_line_snapshot();
    first_snapshot.tool_state = Some(sample_tool_state());
    save_snapshot(&first_snapshot, &first_output).expect("first output save should succeed");

    let mut second_output = options.clone();
    second_output.set_output_identity(Some("HDMI_A_1"));
    let mut second_snapshot = transparent_line_snapshot();
    second_snapshot.tool_state = Some(sample_tool_state());
    save_snapshot(&second_snapshot, &second_output).expect("second output save should succeed");

    let mut neighbor = SessionOptions::new(temp.path().to_path_buf(), "display-10");
    neighbor.persist_transparent = true;
    neighbor.restore_tool_state = true;
    neighbor.set_output_identity(Some("DP_1"));
    let mut neighbor_snapshot = transparent_line_snapshot();
    neighbor_snapshot.tool_state = Some(sample_tool_state());
    save_snapshot(&neighbor_snapshot, &neighbor).expect("neighbor save should succeed");

    let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

    assert_eq!(
        outcome,
        ClearToolStateOutcome::Cleared {
            preserved_board_data: true
        }
    );

    let first_loaded = crate::session::load_snapshot(&first_output)
        .expect("first output load should succeed")
        .expect("first output snapshot should remain");
    let second_loaded = crate::session::load_snapshot(&second_output)
        .expect("second output load should succeed")
        .expect("second output snapshot should remain");
    let neighbor_loaded = crate::session::load_snapshot(&neighbor)
        .expect("neighbor load should succeed")
        .expect("neighbor snapshot should remain");

    assert!(first_loaded.tool_state.is_none());
    assert!(second_loaded.tool_state.is_none());
    assert!(
        neighbor_loaded.tool_state.is_some(),
        "neighboring display prefix should be preserved"
    );
    assert!(first_loaded.has_board_data());
    assert!(second_loaded.has_board_data());
}

#[test]
fn clear_tool_state_missing_session_is_nonfatal() {
    let temp = crate::test_temp::tempdir().unwrap();
    let options = SessionOptions::new(temp.path().to_path_buf(), "display-missing");

    let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

    assert_eq!(outcome, ClearToolStateOutcome::NoSession);
}

#[test]
fn clear_tool_state_without_tool_state_is_nonfatal_and_preserves_boards() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-no-tool");
    options.persist_transparent = true;
    options.restore_tool_state = true;

    let snapshot = transparent_line_snapshot();
    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

    let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");
    assert_eq!(outcome, ClearToolStateOutcome::NoToolState);

    let loaded = crate::session::load_snapshot(&options)
        .expect("load should succeed")
        .expect("snapshot should remain");
    assert!(loaded.tool_state.is_none());
    assert_eq!(loaded.boards[0].pages.pages[0].shapes.len(), 1);
}

#[test]
fn clear_named_tool_state_targets_only_selected_file() {
    let temp = crate::test_temp::tempdir().unwrap();
    let selected = temp.path().join("lecture-04.wayscriber-session");
    let sibling = temp.path().join("lecture-05.wayscriber-session");

    let mut selected_options = SessionOptions::new(temp.path().to_path_buf(), "display");
    selected_options.set_named_file_target(selected);
    selected_options.persist_transparent = true;
    selected_options.restore_tool_state = true;

    let mut sibling_options = SessionOptions::new(temp.path().to_path_buf(), "display");
    sibling_options.set_named_file_target(sibling);
    sibling_options.persist_transparent = true;
    sibling_options.restore_tool_state = true;

    let mut selected_snapshot = transparent_line_snapshot();
    selected_snapshot.tool_state = Some(sample_tool_state());
    save_snapshot(&selected_snapshot, &selected_options).expect("selected save should succeed");

    let mut sibling_snapshot = transparent_line_snapshot();
    sibling_snapshot.tool_state = Some(sample_tool_state());
    save_snapshot(&sibling_snapshot, &sibling_options).expect("sibling save should succeed");

    let outcome =
        clear_tool_state(&selected_options).expect("selected clear_tool_state should succeed");
    assert_eq!(
        outcome,
        ClearToolStateOutcome::Cleared {
            preserved_board_data: true
        }
    );

    let selected_loaded = crate::session::load_snapshot(&selected_options)
        .expect("selected load should succeed")
        .expect("selected snapshot should remain");
    let sibling_loaded = crate::session::load_snapshot(&sibling_options)
        .expect("sibling load should succeed")
        .expect("sibling snapshot should remain");

    assert!(selected_loaded.tool_state.is_none());
    assert!(
        sibling_loaded.tool_state.is_some(),
        "sibling tool state should be preserved"
    );
}

#[test]
fn inspect_named_file_missing_parent_reports_absent_session() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display");
    let named_path = temp
        .path()
        .join("missing")
        .join("lecture.wayscriber-session");
    options.set_named_file_target(named_path.clone());

    let inspection = inspect_session(&options).expect("inspect should succeed");

    assert_eq!(inspection.session_path, named_path);
    assert!(!inspection.exists);
    assert!(!inspection.backup_exists);
}

#[test]
fn inspect_named_file_does_not_create_missing_lock() {
    let temp = crate::test_temp::tempdir().unwrap();
    let named_path = temp.path().join("lecture.wayscriber-session");
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display");
    options.set_named_file_target(named_path);
    options.persist_transparent = true;

    let snapshot = transparent_line_snapshot();

    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");
    let lock_path = options.lock_file_path();
    std::fs::remove_file(&lock_path).expect("remove named session lock");

    let inspection = inspect_session(&options).expect("inspect should succeed");

    assert!(inspection.exists);
    assert!(
        !lock_path.exists(),
        "named session info should not create {}",
        lock_path.display()
    );
    assert_eq!(
        inspection
            .frame_counts
            .expect("frame counts should be loaded")
            .transparent,
        1
    );
}

#[cfg(unix)]
#[test]
fn inspect_named_file_rejects_special_lock_without_blocking() {
    let temp = crate::test_temp::tempdir().unwrap();
    let named_path = temp.path().join("lecture.wayscriber-session");
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display");
    options.set_named_file_target(named_path);
    options.persist_transparent = true;

    let snapshot = transparent_line_snapshot();
    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");
    let lock_path = options.lock_file_path();
    std::fs::remove_file(&lock_path).expect("remove named session lock");
    make_fifo(&lock_path);

    let err = inspect_session(&options).expect_err("special lock should be rejected");

    assert!(
        err.to_string()
            .contains("session lock file is not a regular file"),
        "{err:#}"
    );
}

#[test]
fn clear_named_file_removes_only_selected_artifacts() {
    let temp = crate::test_temp::tempdir().unwrap();
    let selected = temp.path().join("lecture-04.wayscriber-session");
    let sibling = temp.path().join("lecture-05.wayscriber-session");
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display");
    options.set_named_file_target(selected.clone());

    for path in [
        options.session_file_path(),
        options.backup_file_path(),
        options.backup_recovery_marker_file_path(),
        options.recovery_file_path(),
        options.recovery_recoverable_marker_file_path(),
        options.clear_marker_file_path(),
        options.lock_file_path(),
    ] {
        std::fs::write(path, b"artifact").unwrap();
    }
    for suffix in ["", ".bak", ".recovery", ".cleared", ".lock"] {
        std::fs::write(
            crate::session::append_path_suffix(&sibling, suffix),
            b"sibling",
        )
        .unwrap();
    }

    let outcome = clear_session(&options).expect("clear should succeed");

    assert!(outcome.removed_session);
    assert!(outcome.removed_backup);
    assert!(outcome.removed_recovery);
    assert!(outcome.removed_lock);
    for path in [
        options.session_file_path(),
        options.backup_file_path(),
        options.backup_recovery_marker_file_path(),
        options.recovery_file_path(),
        options.recovery_recoverable_marker_file_path(),
        options.clear_marker_file_path(),
        options.lock_file_path(),
    ] {
        assert!(!path.exists(), "{} should be removed", path.display());
    }
    for suffix in ["", ".bak", ".recovery", ".cleared", ".lock"] {
        let path = crate::session::append_path_suffix(&sibling, suffix);
        assert!(path.exists(), "{} should be preserved", path.display());
    }
}

#[cfg(unix)]
#[test]
fn clear_named_file_rejects_symlink_primary_without_removing_artifacts() {
    let temp = crate::test_temp::tempdir().unwrap();
    let target = temp.path().join("real-session.wayscriber-session");
    let link = temp.path().join("linked-session.wayscriber-session");
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display");
    options.set_named_file_target(link.clone());

    std::fs::write(&target, b"target").expect("write target session");
    symlink(&target, &link).expect("create symlink primary");
    std::fs::write(options.backup_file_path(), b"backup").expect("write backup sidecar");

    let err = clear_session(&options).expect_err("symlink primary should reject clear");

    assert!(err.to_string().contains("not a symlink"), "{err:#}");
    assert!(
        std::fs::symlink_metadata(&link)
            .expect("symlink should remain")
            .file_type()
            .is_symlink(),
        "clear must not remove the symlink primary"
    );
    assert_eq!(
        std::fs::read(&target).expect("target remains"),
        b"target",
        "clear must not affect the symlink target"
    );
    assert!(
        options.backup_file_path().exists(),
        "clear must not remove selected sidecars after primary validation fails"
    );
}

#[cfg(unix)]
fn make_fifo(path: &std::path::Path) {
    let raw_path = CString::new(path.as_os_str().as_bytes()).expect("fifo path has no NUL bytes");
    // SAFETY: raw_path is a valid, NUL-terminated filesystem path for this process.
    let result = unsafe { libc::mkfifo(raw_path.as_ptr(), 0o600) };
    assert_eq!(
        result,
        0,
        "mkfifo {} failed: {}",
        path.display(),
        std::io::Error::last_os_error()
    );
}
