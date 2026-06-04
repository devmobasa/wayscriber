use super::compression::is_gzip;
use super::load::{
    LoadSnapshotOutcome, load_named_session_candidate,
    load_named_session_candidate_with_expanded_limit, load_snapshot_inner,
    load_snapshot_inner_with_expanded_limit, load_snapshot_with_expanded_limit,
};
use super::save::{
    save_snapshot_with_expanded_limit, save_snapshot_with_report_and_clear_boundary,
};
use super::types::{
    BoardFile, BoardPagesSnapshot, BoardSnapshot, CURRENT_VERSION, SessionFile, SessionSnapshot,
    ToolStateSnapshot,
};
use super::{load_snapshot, save_snapshot};
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::draw::{Color, FontDescriptor, Frame, Shape};
use crate::input::EraserMode;
use crate::session::options::{CompressionMode, SessionOptions};
use crate::test_temp::tempdir;
use crate::time_utils::now_rfc3339;
use std::path::Path;
use std::time::{Duration, SystemTime};
#[cfg(unix)]
use std::{
    ffi::CString,
    os::unix::{ffi::OsStrExt, fs::symlink},
};

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

fn sample_tool_state() -> ToolStateSnapshot {
    ToolStateSnapshot {
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
        polygon_sides: crate::draw::REGULAR_POLYGON_DEFAULT_SIDES,
        board_previous_color: None,
        show_status_bar: true,
        tool_settings: None,
    }
}

fn contentless_session_file() -> SessionFile {
    SessionFile {
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
        tool_state: Some(sample_tool_state()),
    }
}

fn sample_session_file() -> SessionFile {
    SessionFile {
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
    }
}

fn write_contentless_session(path: &Path) {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&contentless_session_file()).expect("contentless session json"),
    )
    .expect("contentless session write");
}

#[cfg(unix)]
fn make_fifo(path: &Path) {
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

fn set_modified(path: &Path, modified: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open file for timestamp update")
        .set_modified(modified)
        .expect("set file modified timestamp");
}

fn assert_no_candidate_sidecars(options: &SessionOptions) {
    for path in [
        options.lock_file_path(),
        options.backup_file_path(),
        options.backup_recovery_marker_file_path(),
        options.recovery_file_path(),
        options.recovery_recoverable_marker_file_path(),
        options.clear_marker_file_path(),
    ] {
        assert!(
            !path.exists(),
            "candidate load must not create or mutate sidecar {}",
            path.display()
        );
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
fn save_named_file_fails_when_parent_is_missing_without_creating_it() {
    let temp = tempdir().unwrap();
    let missing_parent = temp.path().join("missing-parent");
    let named_path = missing_parent.join("session.wayscriber-session");
    let snapshot = sample_snapshot();
    let mut options = SessionOptions::new(temp.path().join("configured"), "named");
    options.persist_transparent = true;
    options.set_named_file_target(named_path);

    let err = save_snapshot(&snapshot, &options).expect_err("named save should fail");

    assert!(
        err.to_string()
            .contains("named session parent directory does not exist"),
        "unexpected error: {err:#}"
    );
    assert!(
        !missing_parent.exists(),
        "named save must not create the missing parent"
    );
    assert!(
        !temp.path().join("configured").exists(),
        "named save must not fall back to configured storage"
    );
}

#[cfg(unix)]
#[test]
fn save_named_file_rejects_symlink_lock_without_truncating_target() {
    let temp = tempdir().unwrap();
    let named_path = temp.path().join("session.wayscriber-session");
    let lock_target = temp.path().join("lock-target");
    let snapshot = sample_snapshot();
    let mut options = SessionOptions::new(temp.path().join("configured"), "named-symlink-save");
    options.persist_transparent = true;
    options.set_named_file_target(named_path);
    std::fs::write(&lock_target, b"preserve me").expect("write lock target");
    symlink(&lock_target, options.lock_file_path()).expect("create lock symlink");

    let err =
        save_snapshot(&snapshot, &options).expect_err("named save should reject symlink lock");

    assert!(
        err.to_string().contains("failed to open session lock file"),
        "{err:#}"
    );
    assert_eq!(
        std::fs::read(&lock_target).expect("read lock target"),
        b"preserve me",
        "named save must not truncate a symlink lock target"
    );
    assert!(
        !options.session_file_path().exists(),
        "failed named save should not create the session file"
    );
}

#[cfg(unix)]
#[test]
fn save_named_file_rejects_fifo_lock_without_blocking() {
    let temp = tempdir().unwrap();
    let named_path = temp.path().join("session.wayscriber-session");
    let snapshot = sample_snapshot();
    let mut options = SessionOptions::new(temp.path().join("configured"), "named-fifo-save");
    options.persist_transparent = true;
    options.set_named_file_target(named_path);
    make_fifo(&options.lock_file_path());

    let err = save_snapshot(&snapshot, &options).expect_err("named save should reject fifo lock");

    assert!(
        format!("{err:#}").contains("session lock file is not a regular file"),
        "{err:#}"
    );
    assert!(
        !options.session_file_path().exists(),
        "failed named save should not create the session file"
    );
}

#[cfg(unix)]
#[test]
fn load_named_file_rejects_symlink_lock_without_truncating_target() {
    let temp = tempdir().unwrap();
    let named_path = temp.path().join("session.wayscriber-session");
    let lock_target = temp.path().join("lock-target");
    let snapshot = sample_snapshot();
    let mut options = SessionOptions::new(temp.path().join("configured"), "named-symlink-load");
    options.persist_transparent = true;
    options.set_named_file_target(named_path);
    save_snapshot(&snapshot, &options).expect("save named snapshot");
    std::fs::remove_file(options.lock_file_path()).expect("remove regular lock");
    std::fs::write(&lock_target, b"preserve me").expect("write lock target");
    symlink(&lock_target, options.lock_file_path()).expect("create lock symlink");

    let err = load_snapshot(&options).expect_err("named load should reject symlink lock");

    assert!(
        err.to_string().contains("failed to open session lock file"),
        "{err:#}"
    );
    assert_eq!(
        std::fs::read(&lock_target).expect("read lock target"),
        b"preserve me",
        "named load must not truncate a symlink lock target"
    );
}

#[test]
fn configured_save_still_creates_session_base_directory() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path().join("configured");
    let snapshot = sample_snapshot();
    let mut options = SessionOptions::new(base_dir.clone(), "configured");
    options.persist_transparent = true;

    save_snapshot(&snapshot, &options).expect("configured save should create base dir");

    assert!(base_dir.is_dir());
    assert!(options.session_file_path().is_file());
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

#[cfg(unix)]
#[test]
fn load_snapshot_inner_rejects_fifo_without_opening_it() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("session-fifo.wayscriber-session");
    make_fifo(&path);

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "fifo");
    options.persist_transparent = true;

    let err = match load_snapshot_inner(&path, &options) {
        Ok(_) => panic!("FIFO session artifact should be rejected before open/read"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("not a regular file"),
        "unexpected error: {err:#}"
    );
}

#[cfg(unix)]
#[test]
fn load_named_primary_rejects_symlink_without_following_target() {
    let temp = tempdir().unwrap();
    let target = temp.path().join("real-session.wayscriber-session");
    let link = temp.path().join("linked-session.wayscriber-session");
    std::fs::write(&target, b"{not valid json").expect("write invalid target");
    symlink(&target, &link).expect("create primary symlink");

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "symlink-primary");
    options.persist_transparent = true;
    options.set_named_file_target(link.clone());

    let err = match load_snapshot_inner(&link, &options) {
        Ok(_) => panic!("symlink primary should reject"),
        Err(err) => err,
    };

    assert!(
        err.to_string().contains("symlink"),
        "unexpected error: {err:#}"
    );
    assert!(
        !options.backup_file_path().exists(),
        "rejected symlink target should not be backed up or mutated"
    );
}

#[test]
fn load_named_corrupt_primary_backs_up_without_removing_selected_file() {
    let temp = tempdir().unwrap();
    let named_path = temp.path().join("corrupt.wayscriber-session");
    std::fs::write(&named_path, b"{not valid json").expect("write corrupt named primary");

    let mut options = SessionOptions::new(temp.path().join("configured"), "named-corrupt");
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("corrupt named primary should be handled");

    assert!(matches!(outcome, LoadSnapshotOutcome::Empty));
    assert_eq!(
        std::fs::read(&named_path).expect("named primary remains"),
        b"{not valid json",
        "named corrupt backup must not remove the selected primary path"
    );
    assert_eq!(
        std::fs::read(options.backup_file_path()).expect("backup bytes"),
        b"{not valid json",
        "named corrupt primary should still be backed up for diagnostics"
    );
}

#[test]
fn load_named_candidate_rejects_missing_without_creating_artifacts() {
    let temp = tempdir().unwrap();
    let named_path = temp.path().join("missing.wayscriber-session");
    let mut options = SessionOptions::new(temp.path().join("configured"), "candidate-missing");
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());

    let err = load_named_session_candidate(&options).expect_err("missing candidate should fail");

    assert!(
        err.to_string()
            .contains("named session file does not exist"),
        "unexpected error: {err:#}"
    );
    assert!(
        !named_path.exists(),
        "missing candidate load must not create a blank primary"
    );
    assert_no_candidate_sidecars(&options);
}

#[cfg(unix)]
#[test]
fn load_named_candidate_rejects_symlink_without_following_target() {
    let temp = tempdir().unwrap();
    let target = temp.path().join("real.wayscriber-session");
    let link = temp.path().join("linked.wayscriber-session");
    std::fs::write(&target, b"target bytes").expect("write target");
    symlink(&target, &link).expect("create symlink");

    let mut options = SessionOptions::new(temp.path().join("configured"), "candidate-symlink");
    options.persist_transparent = true;
    options.set_named_file_target(link);

    let err = load_named_session_candidate(&options).expect_err("symlink candidate should fail");

    assert!(err.to_string().contains("symlink"), "{err:#}");
    assert_eq!(
        std::fs::read(&target).expect("target bytes"),
        b"target bytes",
        "symlink target must not be opened or mutated"
    );
    assert_no_candidate_sidecars(&options);
}

#[cfg(unix)]
#[test]
fn load_named_candidate_waits_for_existing_lock_before_validating_primary() {
    let temp = tempdir().unwrap();
    let named_path = temp.path().join("locked-create.wayscriber-session");
    let mut options = SessionOptions::new(temp.path().join("configured"), "candidate-lock-order");
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());

    let lock_file = std::fs::File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(options.lock_file_path())
        .expect("create lock");
    crate::session::lock::lock_exclusive(&lock_file).expect("exclusive lock");

    let (tx, rx) = std::sync::mpsc::channel();
    let load_options = options.clone();
    let loader = std::thread::spawn(move || {
        tx.send(load_named_session_candidate(&load_options))
            .expect("send candidate result");
    });

    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(
        &named_path,
        serde_json::to_vec_pretty(&sample_session_file()).expect("session json"),
    )
    .expect("write session while lock is held");
    crate::session::lock::unlock(&lock_file).expect("unlock");

    let outcome = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("candidate load should finish after unlock")
        .expect("candidate load should succeed");
    loader.join().expect("loader thread");
    assert_loaded_sample_snapshot(outcome);
}

#[cfg(unix)]
#[test]
fn load_named_candidate_rejects_fifo_without_blocking_or_creating_artifacts() {
    let temp = tempdir().unwrap();
    let named_path = temp.path().join("fifo.wayscriber-session");
    make_fifo(&named_path);

    let mut options = SessionOptions::new(temp.path().join("configured"), "candidate-fifo");
    options.persist_transparent = true;
    options.set_named_file_target(named_path);

    let err = load_named_session_candidate(&options).expect_err("fifo candidate should fail");

    assert!(format!("{err:#}").contains("special file"), "{err:#}");
    assert_no_candidate_sidecars(&options);
}

#[test]
fn load_named_candidate_corrupt_primary_does_not_create_sidecars() {
    let temp = tempdir().unwrap();
    let named_path = temp.path().join("corrupt-open.wayscriber-session");
    std::fs::write(&named_path, b"{not valid json").expect("write corrupt candidate");

    let mut options = SessionOptions::new(temp.path().join("configured"), "candidate-corrupt");
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());

    let err = load_named_session_candidate(&options).expect_err("corrupt candidate should fail");

    assert!(format!("{err:#}").contains("failed to parse session json"));
    assert_eq!(
        std::fs::read(&named_path).expect("candidate bytes remain"),
        b"{not valid json"
    );
    assert_no_candidate_sidecars(&options);
}

#[test]
fn load_named_candidate_ignores_contentful_primary_older_than_clear_marker() {
    let temp = tempdir().unwrap();
    let named_path = temp.path().join("cleared-primary.wayscriber-session");
    let mut options = SessionOptions::new(temp.path().join("configured"), "candidate-cleared");
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());
    std::fs::write(
        &named_path,
        serde_json::to_vec_pretty(&sample_session_file()).expect("primary session json"),
    )
    .expect("write primary");
    std::fs::write(options.clear_marker_file_path(), b"cleared").expect("write clear marker");
    let primary_time = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let clear_time = SystemTime::UNIX_EPOCH + Duration::from_secs(20);
    set_modified(&named_path, primary_time);
    set_modified(&options.clear_marker_file_path(), clear_time);
    let primary_before = std::fs::read(&named_path).expect("primary bytes");

    let outcome = load_named_session_candidate(&options).expect("cleared primary should not load");

    assert!(matches!(outcome, LoadSnapshotOutcome::Empty));
    assert_eq!(
        std::fs::read(&named_path).expect("primary remains"),
        primary_before
    );
    assert!(
        !options.backup_file_path().exists()
            && !options.recovery_file_path().exists()
            && !options.backup_recovery_marker_file_path().exists()
            && !options.recovery_recoverable_marker_file_path().exists(),
        "clear-marker suppression must not create candidate sidecars"
    );
}

#[cfg(unix)]
#[test]
fn load_named_candidate_ignores_symlink_clear_marker() {
    let temp = tempdir().unwrap();
    let named_path = temp
        .path()
        .join("symlink-clear-marker-primary.wayscriber-session");
    let clear_marker_target = temp.path().join("clear-marker-target");
    let mut options =
        SessionOptions::new(temp.path().join("configured"), "candidate-symlink-clear");
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());
    std::fs::write(
        &named_path,
        serde_json::to_vec_pretty(&sample_session_file()).expect("primary session json"),
    )
    .expect("write primary");
    std::fs::write(&clear_marker_target, b"target clear marker").expect("write target marker");
    symlink(&clear_marker_target, options.clear_marker_file_path())
        .expect("create clear marker symlink");
    let primary_time = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let marker_time = SystemTime::UNIX_EPOCH + Duration::from_secs(20);
    set_modified(&named_path, primary_time);
    set_modified(&clear_marker_target, marker_time);

    let outcome = load_named_session_candidate(&options).expect("symlink marker should be ignored");

    assert_loaded_sample_snapshot(outcome);
    assert!(
        std::fs::symlink_metadata(options.clear_marker_file_path())
            .expect("clear marker metadata")
            .file_type()
            .is_symlink(),
        "candidate load must not replace the symlink marker"
    );
}

#[test]
fn load_named_candidate_ignores_corrupt_primary_older_than_clear_marker() {
    let temp = tempdir().unwrap();
    let named_path = temp
        .path()
        .join("cleared-corrupt-primary.wayscriber-session");
    let mut options =
        SessionOptions::new(temp.path().join("configured"), "candidate-cleared-corrupt");
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());
    std::fs::write(&named_path, b"{not valid json").expect("write corrupt primary");
    std::fs::write(options.clear_marker_file_path(), b"cleared").expect("write clear marker");
    let primary_time = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let clear_time = SystemTime::UNIX_EPOCH + Duration::from_secs(20);
    set_modified(&named_path, primary_time);
    set_modified(&options.clear_marker_file_path(), clear_time);

    let outcome =
        load_named_session_candidate(&options).expect("stale corrupt primary should not fail");

    assert!(matches!(outcome, LoadSnapshotOutcome::Empty));
    assert_eq!(
        std::fs::read(&named_path).expect("corrupt primary remains"),
        b"{not valid json"
    );
    assert!(
        !options.backup_file_path().exists()
            && !options.recovery_file_path().exists()
            && !options.backup_recovery_marker_file_path().exists()
            && !options.recovery_recoverable_marker_file_path().exists(),
        "suppressed corrupt primary must not create candidate sidecars"
    );
}

#[cfg(unix)]
#[test]
fn load_named_candidate_ignores_symlink_backup_recoverable_marker() {
    let temp = tempdir().unwrap();
    let named_path = temp
        .path()
        .join("symlink-backup-marker-primary.wayscriber-session");
    let marker_target = temp.path().join("backup-recoverable-marker-target");
    let mut options = SessionOptions::new(
        temp.path().join("configured"),
        "candidate-symlink-backup-marker",
    );
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());
    std::fs::write(
        options.backup_file_path(),
        serde_json::to_vec_pretty(&sample_session_file()).expect("backup session json"),
    )
    .expect("write backup");
    write_contentless_session(&named_path);
    std::fs::write(&marker_target, b"target backup marker").expect("write target marker");
    symlink(&marker_target, options.backup_recovery_marker_file_path())
        .expect("create backup marker symlink");
    let backup_time = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let primary_time = SystemTime::UNIX_EPOCH + Duration::from_secs(20);
    set_modified(&options.backup_file_path(), backup_time);
    set_modified(&named_path, primary_time);

    let outcome = load_named_session_candidate(&options).expect("symlink marker should be ignored");

    let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
        panic!("expected primary contentless session, got {outcome:?}");
    };
    assert!(!snapshot.has_board_data());
    assert!(snapshot.tool_state.is_some());
}

#[cfg(unix)]
#[test]
fn load_named_candidate_ignores_symlink_recovery_recoverable_marker() {
    let temp = tempdir().unwrap();
    let named_path = temp
        .path()
        .join("symlink-recovery-marker-primary.wayscriber-session");
    let marker_target = temp.path().join("recovery-recoverable-marker-target");
    let mut options = SessionOptions::new(
        temp.path().join("configured"),
        "candidate-symlink-recovery-marker",
    );
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());
    std::fs::write(
        options.recovery_file_path(),
        serde_json::to_vec_pretty(&sample_session_file()).expect("recovery session json"),
    )
    .expect("write recovery");
    write_contentless_session(&named_path);
    std::fs::write(&marker_target, b"target recovery marker").expect("write target marker");
    symlink(
        &marker_target,
        options.recovery_recoverable_marker_file_path(),
    )
    .expect("create recovery marker symlink");
    let recovery_time = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let primary_time = SystemTime::UNIX_EPOCH + Duration::from_secs(20);
    set_modified(&options.recovery_file_path(), recovery_time);
    set_modified(&named_path, primary_time);

    let outcome = load_named_session_candidate(&options).expect("symlink marker should be ignored");

    let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
        panic!("expected primary contentless session, got {outcome:?}");
    };
    assert!(!snapshot.has_board_data());
    assert!(snapshot.tool_state.is_some());
}

#[test]
fn load_named_candidate_falls_back_to_valid_primary_when_recovery_is_corrupt() {
    let temp = tempdir().unwrap();
    let named_path = temp
        .path()
        .join("primary-with-corrupt-recovery.wayscriber-session");
    let mut options = SessionOptions::new(temp.path().join("configured"), "candidate-recovery");
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());
    std::fs::write(
        &named_path,
        serde_json::to_vec_pretty(&sample_session_file()).expect("primary session json"),
    )
    .expect("write primary");

    let recovery_bytes = b"{not valid recovery json";
    std::thread::sleep(Duration::from_millis(2));
    std::fs::write(options.recovery_file_path(), recovery_bytes).expect("write corrupt recovery");

    let outcome =
        load_named_session_candidate(&options).expect("corrupt recovery should not block primary");

    assert_loaded_sample_snapshot(outcome);
    assert_eq!(
        std::fs::read(options.recovery_file_path()).expect("corrupt recovery remains"),
        recovery_bytes
    );
    assert!(
        !options.backup_file_path().exists()
            && !options.clear_marker_file_path().exists()
            && !options.recovery_recoverable_marker_file_path().exists(),
        "candidate sidecar fallback must not create or mutate diagnostic artifacts"
    );
}

#[test]
fn load_named_candidate_reports_newer_oversized_recovery_without_loading_primary() {
    let temp = tempdir().unwrap();
    let named_path = temp
        .path()
        .join("primary-with-oversized-recovery.wayscriber-session");
    let mut options = SessionOptions::new(
        temp.path().join("configured"),
        "candidate-oversized-recovery",
    );
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());
    std::fs::write(
        &named_path,
        serde_json::to_vec_pretty(&sample_session_file()).expect("primary session json"),
    )
    .expect("write primary");
    let recovery_bytes = vec![b' '; 64];
    std::fs::write(options.recovery_file_path(), &recovery_bytes).expect("write recovery");
    let primary_time = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let recovery_time = SystemTime::UNIX_EPOCH + Duration::from_secs(20);
    set_modified(&named_path, primary_time);
    set_modified(&options.recovery_file_path(), recovery_time);

    let outcome = load_named_session_candidate_with_expanded_limit(&options, 8)
        .expect("oversized recovery should be reported as a protective outcome");

    assert!(matches!(
        outcome,
        LoadSnapshotOutcome::ExpandedTooLarge {
            path,
            max_expanded_size: 8,
        } if path == options.recovery_file_path()
    ));
    assert_eq!(
        std::fs::read(options.recovery_file_path()).expect("recovery remains"),
        recovery_bytes
    );
    assert!(
        !options.backup_file_path().exists()
            && !options.clear_marker_file_path().exists()
            && !options.backup_recovery_marker_file_path().exists()
            && !options.recovery_recoverable_marker_file_path().exists(),
        "oversized candidate recovery must not create or mutate sidecars"
    );
}

#[test]
fn load_named_candidate_oversize_primary_does_not_create_sidecars() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let named_path = temp.path().join("too-large.wayscriber-session");
    let mut options = SessionOptions::new(temp.path().join("configured"), "candidate-too-large");
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());
    save_snapshot(&snapshot, &options).expect("save candidate");
    let original_bytes = std::fs::read(&named_path).expect("candidate bytes");
    std::fs::remove_file(options.lock_file_path()).expect("remove save-created lock");
    options.max_file_size_bytes = 1;

    let err = load_named_session_candidate(&options).expect_err("oversize candidate should fail");

    assert!(format!("{err:#}").contains("exceeds the configured limit"));
    assert_eq!(
        std::fs::read(&named_path).expect("candidate bytes remain"),
        original_bytes
    );
    assert_no_candidate_sidecars(&options);
}

#[test]
fn load_named_candidate_newer_version_does_not_create_sidecars() {
    let temp = tempdir().unwrap();
    let named_path = temp.path().join("newer.wayscriber-session");
    let mut file = sample_session_file();
    file.version = CURRENT_VERSION + 1;
    std::fs::write(
        &named_path,
        serde_json::to_vec_pretty(&file).expect("newer session json"),
    )
    .expect("write newer candidate");

    let mut options = SessionOptions::new(temp.path().join("configured"), "candidate-newer");
    options.persist_transparent = true;
    options.set_named_file_target(named_path);

    let outcome = load_named_session_candidate(&options).expect("newer candidate is handled");

    assert!(matches!(outcome, LoadSnapshotOutcome::Empty));
    assert_no_candidate_sidecars(&options);
}

#[cfg(unix)]
#[test]
fn load_named_candidate_rejects_special_lock_without_creating_replacement() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let named_path = temp.path().join("lock-fifo.wayscriber-session");
    let mut options = SessionOptions::new(temp.path().join("configured"), "candidate-lock-fifo");
    options.persist_transparent = true;
    options.set_named_file_target(named_path.clone());
    save_snapshot(&snapshot, &options).expect("save candidate");
    std::fs::remove_file(options.lock_file_path()).expect("remove save-created lock");
    make_fifo(&options.lock_file_path());
    let original_bytes = std::fs::read(&named_path).expect("candidate bytes");

    let err = load_named_session_candidate(&options).expect_err("fifo lock should fail");

    assert!(
        format!("{err:#}").contains("session lock file is not a regular file"),
        "{err:#}"
    );
    assert_eq!(
        std::fs::read(&named_path).expect("candidate bytes remain"),
        original_bytes
    );
    assert!(
        options.lock_file_path().exists(),
        "candidate loader must not remove a rejected lock sidecar"
    );
    assert!(
        !options.backup_file_path().exists()
            && !options.recovery_file_path().exists()
            && !options.clear_marker_file_path().exists(),
        "candidate lock rejection must not create backup/recovery/clear sidecars"
    );
}

#[cfg(unix)]
#[test]
fn load_snapshot_skips_fifo_recovery_without_opening_it() {
    let temp = tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "fifo-recovery");
    options.persist_transparent = true;
    make_fifo(&options.recovery_file_path());

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("non-regular recovery should be skipped without failing");
    assert!(matches!(outcome, LoadSnapshotOutcome::Empty));
    assert!(
        options.recovery_file_path().exists(),
        "non-regular recovery should not be renamed or backed up"
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

    let recovery_file = sample_session_file();
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
fn load_snapshot_prefers_valid_recovery_when_primary_is_non_regular() {
    let temp = tempdir().unwrap();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "nonregular-recovery");
    options.persist_transparent = true;
    std::fs::create_dir(options.session_file_path()).expect("primary directory");
    std::fs::write(
        options.recovery_file_path(),
        serde_json::to_vec_pretty(&sample_session_file()).expect("recovery json"),
    )
    .expect("recovery write");

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("valid recovery should load over non-regular primary");

    assert!(
        matches!(outcome, LoadSnapshotOutcome::LoadedFromRecovery(_)),
        "valid recovery should win before the non-regular primary outcome"
    );
    assert!(
        options.session_file_path().is_dir(),
        "non-regular primary should be left in place"
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
fn load_snapshot_restores_newer_contentful_backup_when_primary_has_no_board_data() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let older = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(20);

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "blank-primary");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::copy(options.session_file_path(), options.backup_file_path())
        .expect("backup should be seeded");

    write_contentless_session(&options.session_file_path());
    set_modified(&options.session_file_path(), older);
    set_modified(&options.backup_file_path(), newer);

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("backup fallback should load");
    let LoadSnapshotOutcome::LoadedFromBackup(snapshot) = outcome else {
        panic!("expected backup restore, got {outcome:?}");
    };
    assert_sample_snapshot(&snapshot);
}

#[test]
fn load_snapshot_ignores_older_contentful_backup_when_primary_has_no_board_data() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let older = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(20);

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "stale-backup");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::copy(options.session_file_path(), options.backup_file_path())
        .expect("backup should be seeded");
    write_contentless_session(&options.session_file_path());
    set_modified(&options.backup_file_path(), older);
    set_modified(&options.session_file_path(), newer);

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("blank primary should load without stale backup restore");
    let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
        panic!("expected primary blank session to remain authoritative, got {outcome:?}");
    };
    assert!(
        !snapshot.has_board_data(),
        "older backup must not resurrect drawings over a newer blank primary"
    );
    assert!(snapshot.tool_state.is_some());
}

#[test]
fn load_snapshot_restores_contentful_backup_when_primary_is_missing() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "backup-only");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::rename(options.session_file_path(), options.backup_file_path())
        .expect("primary should be moved to backup");

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("backup-only fallback should load");
    let LoadSnapshotOutcome::LoadedFromBackup(snapshot) = outcome else {
        panic!("expected backup-only restore, got {outcome:?}");
    };
    assert_sample_snapshot(&snapshot);
}

#[test]
fn save_contentless_snapshot_preserves_backup_when_primary_is_missing() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "backup-preserved");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::rename(options.session_file_path(), options.backup_file_path())
        .expect("primary should be moved to backup");
    let backup_before = std::fs::read(options.backup_file_path()).expect("backup bytes");

    let contentless = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: Vec::new(),
        tool_state: Some(sample_tool_state()),
    };
    save_snapshot(&contentless, &options).expect("contentless session should save");

    assert!(
        options.session_file_path().exists(),
        "contentless primary should be saved"
    );
    assert_eq!(
        std::fs::read(options.backup_file_path()).expect("backup should remain"),
        backup_before,
        "contentless save must not delete the only backup when primary was missing"
    );
    assert!(
        options.backup_recovery_marker_file_path().exists(),
        "backup-only recovery must be marked so the newer blank primary cannot shadow it"
    );
    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("preserved backup should load over blank primary");
    let LoadSnapshotOutcome::LoadedFromBackup(snapshot) = outcome else {
        panic!("expected preserved backup restore, got {outcome:?}");
    };
    assert_sample_snapshot(&snapshot);
}

#[test]
fn save_contentless_snapshot_preserves_recovery_when_primary_is_missing() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "recovery-preserved");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::rename(options.session_file_path(), options.recovery_file_path())
        .expect("primary should be moved to recovery");
    let recovery_before = std::fs::read(options.recovery_file_path()).expect("recovery bytes");

    let contentless = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: Vec::new(),
        tool_state: Some(sample_tool_state()),
    };
    save_snapshot(&contentless, &options).expect("contentless session should save");

    assert!(
        options.session_file_path().exists(),
        "contentless primary should be saved"
    );
    assert_eq!(
        std::fs::read(options.recovery_file_path()).expect("recovery should remain"),
        recovery_before,
        "contentless save must not delete the only recovery artifact when primary was missing"
    );
    assert!(
        options.recovery_recoverable_marker_file_path().exists(),
        "recovery-only board data must be marked so the newer blank primary cannot shadow it"
    );
    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("preserved recovery should load over blank primary");
    let LoadSnapshotOutcome::LoadedFromRecovery(snapshot) = outcome else {
        panic!("expected preserved recovery restore, got {outcome:?}");
    };
    assert_sample_snapshot(&snapshot);

    save_snapshot(&contentless, &options).expect("second contentless session should save");
    assert_eq!(
        std::fs::read(options.recovery_file_path())
            .expect("recovery should remain after second save"),
        recovery_before,
        "a later non-clear contentless save must not remove the recoverable recovery artifact"
    );
}

#[test]
fn clear_empty_snapshot_removes_backup_when_primary_is_missing() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "clear-backup");
    options.persist_transparent = true;
    options.restore_tool_state = false;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::rename(options.session_file_path(), options.backup_file_path())
        .expect("primary should be moved to backup");
    std::fs::write(options.backup_recovery_marker_file_path(), b"recoverable")
        .expect("backup recovery marker");

    let cleared = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: Vec::new(),
        tool_state: None,
    };
    save_snapshot(&cleared, &options).expect("empty clear should save");

    assert!(
        !options.backup_file_path().exists(),
        "intentional empty clear must remove stale backup-only board data"
    );
    assert!(
        !options.backup_recovery_marker_file_path().exists(),
        "intentional empty clear must remove stale backup recovery markers"
    );
    assert!(
        options.clear_marker_file_path().exists(),
        "intentional empty clear should leave a marker that suppresses stale artifacts"
    );
    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("cleared session should load as empty");
    let LoadSnapshotOutcome::Empty = outcome else {
        panic!("expected cleared session to stay empty, got {outcome:?}");
    };
}

#[test]
fn clear_empty_snapshot_removes_recovery_when_primary_is_missing() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "clear-recovery");
    options.persist_transparent = true;
    options.restore_tool_state = false;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::rename(options.session_file_path(), options.recovery_file_path())
        .expect("primary should be moved to recovery");

    let cleared = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: Vec::new(),
        tool_state: None,
    };
    save_snapshot(&cleared, &options).expect("empty clear should save");

    assert!(
        !options.recovery_file_path().exists(),
        "intentional empty clear must remove stale recovery-only board data"
    );
    assert!(
        options.clear_marker_file_path().exists(),
        "intentional empty clear should leave a marker that suppresses stale artifacts"
    );
    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("cleared session should load as empty");
    let LoadSnapshotOutcome::Empty = outcome else {
        panic!("expected cleared session to stay empty, got {outcome:?}");
    };
}

#[test]
fn load_snapshot_ignores_backup_older_than_clear_marker() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let older = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(20);

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "marker-backup");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::rename(options.session_file_path(), options.backup_file_path())
        .expect("primary should be moved to backup");
    std::fs::write(options.clear_marker_file_path(), b"cleared").expect("clear marker");
    set_modified(&options.backup_file_path(), older);
    set_modified(&options.clear_marker_file_path(), newer);

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("stale backup should be suppressed");
    let LoadSnapshotOutcome::Empty = outcome else {
        panic!("expected clear marker to suppress stale backup, got {outcome:?}");
    };
}

#[test]
fn load_snapshot_ignores_primary_older_than_clear_marker() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let older = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(20);

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "marker-primary");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::write(options.clear_marker_file_path(), b"cleared").expect("clear marker");
    set_modified(&options.session_file_path(), older);
    set_modified(&options.clear_marker_file_path(), newer);

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("stale primary should be suppressed");
    let LoadSnapshotOutcome::Empty = outcome else {
        panic!("expected clear marker to suppress stale primary, got {outcome:?}");
    };
}

#[test]
fn load_snapshot_preserves_backup_when_suppressed_primary_is_corrupt() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let primary_time = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let marker_time = SystemTime::UNIX_EPOCH + Duration::from_secs(20);
    let backup_time = SystemTime::UNIX_EPOCH + Duration::from_secs(30);

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "marker-corrupt-primary");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");

    let primary_path = options.session_file_path();
    let backup_path = options.backup_file_path();
    let clear_marker_path = options.clear_marker_file_path();
    let backup_bytes = std::fs::read(&primary_path).expect("saved primary bytes");
    std::fs::write(&backup_path, &backup_bytes).expect("backup write");
    std::fs::write(&primary_path, b"{not valid json").expect("corrupt primary write");
    std::fs::write(&clear_marker_path, b"cleared").expect("clear marker");
    set_modified(&primary_path, primary_time);
    set_modified(&clear_marker_path, marker_time);
    set_modified(&backup_path, backup_time);

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("valid backup should survive suppressed corrupt primary probe");
    let LoadSnapshotOutcome::LoadedFromBackup(snapshot) = outcome else {
        panic!("expected backup restore, got {outcome:?}");
    };
    assert_sample_snapshot(&snapshot);
    assert_eq!(
        std::fs::read(&backup_path).expect("backup should remain readable"),
        backup_bytes,
        "suppressed corrupt primary probe must not overwrite the valid backup"
    );
    assert!(
        primary_path.exists(),
        "suppressed corrupt primary should be ignored without quarantine side effects"
    );
}

#[test]
fn load_snapshot_ignores_recovery_older_than_clear_marker() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let older = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(20);

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "marker-recovery");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::rename(options.session_file_path(), options.recovery_file_path())
        .expect("primary should be moved to recovery");
    std::fs::write(options.clear_marker_file_path(), b"cleared").expect("clear marker");
    set_modified(&options.recovery_file_path(), older);
    set_modified(&options.clear_marker_file_path(), newer);

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("stale recovery should be suppressed");
    let LoadSnapshotOutcome::Empty = outcome else {
        panic!("expected clear marker to suppress stale recovery, got {outcome:?}");
    };
}

#[test]
fn load_snapshot_restores_backup_newer_than_clear_marker() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let older = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(20);

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "marker-newer-backup");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::rename(options.session_file_path(), options.backup_file_path())
        .expect("primary should be moved to backup");
    std::fs::write(options.clear_marker_file_path(), b"cleared").expect("clear marker");
    set_modified(&options.clear_marker_file_path(), older);
    set_modified(&options.backup_file_path(), newer);

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("newer backup should still recover");
    let LoadSnapshotOutcome::LoadedFromBackup(snapshot) = outcome else {
        panic!("expected newer backup restore, got {outcome:?}");
    };
    assert_sample_snapshot(&snapshot);
}

#[test]
fn save_contentless_tool_state_snapshot_marks_clear_boundary_before_backup_rotation() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let older = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(20);

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "tool-clear-marker");
    options.persist_transparent = true;
    options.restore_tool_state = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    let stale_backup_bytes = std::fs::read(options.session_file_path()).expect("primary bytes");

    let contentless = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: Vec::new(),
        tool_state: Some(sample_tool_state()),
    };
    save_snapshot_with_report_and_clear_boundary(&contentless, &options, true)
        .expect("contentless tool-state session should save");

    assert!(
        options.clear_marker_file_path().exists(),
        "contentless tool-state save after board data must mark a clear boundary"
    );
    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("contentless clear primary should still load");
    let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
        panic!("expected contentless primary to load, got {outcome:?}");
    };
    assert!(
        !snapshot.has_board_data(),
        "contentless primary must remain authoritative after a clear"
    );
    assert!(
        snapshot.tool_state.is_some(),
        "clear marker must not suppress the freshly saved tool state"
    );

    std::fs::remove_file(options.session_file_path()).expect("remove primary to simulate crash");
    std::fs::write(options.backup_file_path(), stale_backup_bytes).expect("stale backup write");
    set_modified(&options.backup_file_path(), older);
    set_modified(&options.clear_marker_file_path(), newer);

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("stale backup after contentless clear should be suppressed");
    let LoadSnapshotOutcome::Empty = outcome else {
        panic!("expected clear marker to suppress stale backup, got {outcome:?}");
    };
}

#[test]
fn contentful_save_after_clear_marker_removes_stale_backup_before_marker() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();
    let older = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(20);

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "contentful-after-clear");
    options.persist_transparent = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    std::fs::rename(options.session_file_path(), options.backup_file_path())
        .expect("primary should be moved to backup");
    std::fs::write(options.clear_marker_file_path(), b"cleared").expect("clear marker");
    set_modified(&options.backup_file_path(), older);
    set_modified(&options.clear_marker_file_path(), newer);

    save_snapshot(&snapshot, &options).expect("contentful save should succeed");

    assert!(
        options.session_file_path().exists(),
        "contentful primary should be saved"
    );
    assert!(
        !options.backup_file_path().exists(),
        "contentful save must remove a stale backup before dropping the clear marker"
    );
    assert!(
        !options.clear_marker_file_path().exists(),
        "clear marker can be removed after stale backup cleanup"
    );
}

#[test]
fn contentless_save_without_loaded_board_data_preserves_rotated_backup() {
    let temp = tempdir().unwrap();
    let snapshot = sample_snapshot();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "tool-state-protected");
    options.persist_transparent = true;
    options.restore_tool_state = true;
    save_snapshot(&snapshot, &options).expect("normal session should save");
    let primary_before = std::fs::read(options.session_file_path()).expect("primary bytes");

    let contentless = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: Vec::new(),
        tool_state: Some(sample_tool_state()),
    };
    save_snapshot_with_report_and_clear_boundary(&contentless, &options, false)
        .expect("dirty tool-state-only session should save");

    assert!(
        options.session_file_path().exists(),
        "contentless primary should be saved"
    );
    assert_eq!(
        std::fs::read(options.backup_file_path()).expect("backup should remain"),
        primary_before,
        "contentless save without loaded board data must preserve the rotated recoverable primary"
    );
    assert!(
        options.backup_recovery_marker_file_path().exists(),
        "non-clear contentless save must mark the preserved backup as recoverable"
    );
    assert!(
        !options.clear_marker_file_path().exists(),
        "caller did not mark this contentless save as an intentional clear"
    );

    let outcome = load_snapshot_with_expanded_limit(&options, 64 * 1024)
        .expect("recoverable backup should load over non-clear blank primary");
    let LoadSnapshotOutcome::LoadedFromBackup(snapshot) = outcome else {
        panic!("expected preserved backup restore, got {outcome:?}");
    };
    assert_sample_snapshot(&snapshot);

    save_snapshot_with_report_and_clear_boundary(&contentless, &options, false)
        .expect("second dirty tool-state-only session should save");
    assert_eq!(
        std::fs::read(options.backup_file_path()).expect("backup should remain after second save"),
        primary_before,
        "a later non-clear contentless save must not replace the recoverable backup with a blank primary"
    );
}

#[test]
fn contentless_save_twice_without_loaded_board_data_preserves_preserved_recovery() {
    let temp = tempdir().unwrap();

    let mut options = SessionOptions::new(temp.path().to_path_buf(), "preserved-recovery-twice");
    options.persist_transparent = true;
    options.restore_tool_state = true;
    let preserved_recovery = options
        .recovery_file_path()
        .with_extension("recovery.empty");
    std::fs::write(&preserved_recovery, b"unloadable recovery").expect("preserved recovery write");

    let contentless = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: Vec::new(),
        tool_state: Some(sample_tool_state()),
    };
    save_snapshot_with_report_and_clear_boundary(&contentless, &options, false)
        .expect("first contentless save should succeed");
    save_snapshot_with_report_and_clear_boundary(&contentless, &options, false)
        .expect("second contentless save should succeed");

    assert!(
        preserved_recovery.exists(),
        "contentless saves without loaded board data must not delete preserved recovery artifacts"
    );
    assert!(
        !options.clear_marker_file_path().exists(),
        "caller did not mark either contentless save as an intentional clear"
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
    assert_sample_snapshot(&snapshot);
}

fn assert_sample_snapshot(snapshot: &SessionSnapshot) {
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
fn save_snapshot_serializes_compound_undo_history() {
    let temp = tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "compound-history");
    options.persist_transparent = true;
    options.persist_history = true;

    let mut frame = Frame::new();
    let first_id = frame.add_shape(Shape::Line {
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
    let second_id = frame.add_shape(Shape::Line {
        x1: 20,
        y1: 20,
        x2: 30,
        y2: 30,
        color: Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 2.0,
    });

    let before_first = snapshot_for_shape(&frame, first_id);
    let before_second = snapshot_for_shape(&frame, second_id);
    translate_line(&mut frame, first_id, 5, 0);
    translate_line(&mut frame, second_id, 5, 0);
    let after_first = snapshot_for_shape(&frame, first_id);
    let after_second = snapshot_for_shape(&frame, second_id);
    frame.push_undo_action(
        UndoAction::Compound {
            actions: vec![
                UndoAction::Modify {
                    shape_id: first_id,
                    before: before_first,
                    after: after_first,
                },
                UndoAction::Modify {
                    shape_id: second_id,
                    before: before_second,
                    after: after_second,
                },
            ],
        },
        usize::MAX,
    );

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

    save_snapshot(&snapshot, &options).expect("save_snapshot should serialize compound history");

    let raw = std::fs::read_to_string(options.session_file_path()).expect("read session json");
    assert!(raw.contains("\"kind\": \"compound\""));
    assert!(raw.contains("\"actions\""));

    let loaded = load_snapshot(&options)
        .expect("load_snapshot should succeed")
        .expect("snapshot should be present");
    let page = &loaded.boards[0].pages.pages[0];
    assert_eq!(page.undo_stack_len(), 1);
}

fn snapshot_for_shape(frame: &Frame, shape_id: crate::draw::ShapeId) -> ShapeSnapshot {
    let drawn = frame.shape(shape_id).expect("shape exists");
    ShapeSnapshot {
        shape: drawn.shape.clone(),
        locked: drawn.locked,
    }
}

fn translate_line(frame: &mut Frame, shape_id: crate::draw::ShapeId, dx: i32, dy: i32) {
    let shape = frame.shape_mut(shape_id).expect("shape exists");
    if let Shape::Line { x1, y1, x2, y2, .. } = &mut shape.shape {
        *x1 += dx;
        *y1 += dy;
        *x2 += dx;
        *y2 += dy;
    }
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
