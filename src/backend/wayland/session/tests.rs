use super::*;
use crate::config::{Action, BoardsConfig, KeyBinding, PresenterModeConfig};
use crate::draw::{
    Color, EraserKind, FontDescriptor, Frame, PageDeleteOutcome, REGULAR_POLYGON_DEFAULT_SIDES,
    Shape, ShapeId,
};
use crate::env_vars::{CATALOG_HOOKS_TEST_ENV, XDG_DATA_HOME_ENV};
use crate::input::{
    BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, ClickHighlightSettings, DrawingState, EraserMode,
    Tool,
};
use crate::util::Rect;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink};

struct EnvGuard {
    _guard: MutexGuard<'static, ()>,
    catalog_hooks: Option<std::ffi::OsString>,
    xdg_data_home: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set_xdg_data_home(path: &Path) -> Self {
        let guard = crate::test_env::lock();
        let catalog_hooks = std::env::var_os(CATALOG_HOOKS_TEST_ENV);
        let xdg_data_home = std::env::var_os(XDG_DATA_HOME_ENV);
        unsafe {
            std::env::set_var(CATALOG_HOOKS_TEST_ENV, path);
            std::env::set_var(XDG_DATA_HOME_ENV, path);
        }
        Self {
            _guard: guard,
            catalog_hooks,
            xdg_data_home,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.catalog_hooks.take() {
            Some(value) => unsafe { std::env::set_var(CATALOG_HOOKS_TEST_ENV, value) },
            None => unsafe { std::env::remove_var(CATALOG_HOOKS_TEST_ENV) },
        }
        match self.xdg_data_home.take() {
            Some(value) => unsafe { std::env::set_var(XDG_DATA_HOME_ENV, value) },
            None => unsafe { std::env::remove_var(XDG_DATA_HOME_ENV) },
        }
    }
}

#[cfg(unix)]
struct ReadonlyDirGuard {
    path: PathBuf,
    original_mode: u32,
}

#[cfg(unix)]
impl Drop for ReadonlyDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(
            &self.path,
            std::fs::Permissions::from_mode(self.original_mode),
        );
    }
}

#[cfg(unix)]
fn readonly_dir_guard(path: &Path) -> ReadonlyDirGuard {
    let original_mode = std::fs::metadata(path)
        .expect("parent metadata")
        .permissions()
        .mode();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555))
        .expect("make parent read-only");
    ReadonlyDirGuard {
        path: path.to_path_buf(),
        original_mode,
    }
}

#[cfg(unix)]
fn can_create_probe(parent: &Path) -> bool {
    let probe = parent.join(format!(
        ".wayscriber-runtime-open-manual-access-test-{}",
        std::process::id()
    ));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(file) => {
            drop(file);
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn test_input_state() -> InputState {
    let mut action_map = HashMap::new();
    action_map.insert(KeyBinding::parse("Escape").unwrap(), Action::Exit);
    InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        3.0,
        12.0,
        EraserMode::Brush,
        0.32,
        false,
        32.0,
        FontDescriptor::default(),
        false,
        20.0,
        30.0,
        false,
        true,
        BoardsConfig::default(),
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
        PresenterModeConfig::default(),
    )
}

fn add_line(input: &mut InputState, x2: i32) -> ShapeId {
    input.boards.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2,
        y2: 10,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
        thick: 2.0,
    })
}

fn board_shape_count(input: &InputState, id: &str) -> usize {
    input
        .boards
        .board_states()
        .iter()
        .find(|board| board.spec.id == id)
        .expect("board")
        .pages
        .active_frame()
        .shapes
        .len()
}

fn sample_snapshot() -> stored_session::SessionSnapshot {
    snapshot_for_board("transparent", 42)
}

fn snapshot_for_board(id: &str, x2: i32) -> stored_session::SessionSnapshot {
    stored_session::SessionSnapshot {
        active_board_id: id.to_string(),
        boards: vec![board_snapshot(id, x2)],
        tool_state: None,
    }
}

fn board_snapshot(id: &str, x2: i32) -> stored_session::BoardSnapshot {
    let mut frame = Frame::new();
    frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2,
        y2: 10,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 2.0,
    });
    stored_session::BoardSnapshot {
        id: id.to_string(),
        pages: stored_session::BoardPagesSnapshot {
            pages: vec![frame],
            active: 0,
        },
    }
}

fn sample_tool_state() -> stored_session::ToolStateSnapshot {
    stored_session::ToolStateSnapshot {
        current_color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        current_thickness: 3.0,
        eraser_size: 12.0,
        eraser_kind: EraserKind::Circle,
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
        polygon_sides: REGULAR_POLYGON_DEFAULT_SIDES,
        board_previous_color: None,
        show_status_bar: true,
        tool_settings: None,
    }
}

fn named_options(base: &Path, name: &str) -> SessionOptions {
    let mut options = SessionOptions::new(base.join("configured"), name);
    options.persist_transparent = true;
    options.set_named_file_target(base.join(format!("{name}.wayscriber-session")));
    options
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
            "failed runtime open must not create candidate sidecar {}",
            path.display()
        );
    }
}

fn loaded_line_x2(options: &SessionOptions) -> i32 {
    let outcome = stored_session::load_snapshot_with_outcome(options).expect("load session");
    let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
        panic!("expected loaded snapshot, got {outcome:?}");
    };
    let shape = &snapshot.boards[0].pages.pages[0].shapes[0].shape;
    let Shape::Line { x2, .. } = shape else {
        panic!("expected saved line, got {shape:?}");
    };
    *x2
}

#[test]
fn runtime_save_as_new_path_writes_and_switches_active_target() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-save-as-new");
    let target_options = named_options(temp.path(), "target-save-as-new");

    let mut input = test_input_state();
    add_line(&mut input, 51);
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));

    let report = save_named_session_as_runtime(
        &mut input,
        &mut session_state,
        &target_options.session_file_path(),
        stored_session::SaveAsOverwrite::Deny,
        Instant::now(),
    )
    .expect("runtime save as");

    assert_eq!(report.previous_path, current_options.session_file_path());
    assert_eq!(report.saved_path, target_options.session_file_path());
    assert!(report.switched_target);
    assert!(report.saved);
    assert!(report.saved_board_data);
    assert_eq!(
        report.outcome,
        Some(stored_session::SaveSnapshotOutcome::Full)
    );
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(target_options.session_file_path())
    );
    assert!(!session_state.is_dirty());
    assert!(!input.is_session_dirty());
    assert_eq!(loaded_line_x2(&target_options), 51);

    let recent = stored_session::catalog::recent_sessions().expect("recent sessions");
    let target_entry = recent
        .iter()
        .find(|entry| entry.path == target_options.session_file_path().display().to_string())
        .expect("target catalog entry");
    assert!(
        target_entry.last_saved_at_millis.is_some(),
        "Save As should catalog the target only after commit"
    );
}

#[test]
fn runtime_save_as_rejects_existing_target_without_confirmation() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-save-as-existing");
    let target_options = named_options(temp.path(), "target-save-as-existing");
    stored_session::save_snapshot(&snapshot_for_board("transparent", 17), &target_options)
        .expect("save existing target");
    let before = std::fs::read(target_options.session_file_path()).expect("existing target bytes");

    let mut input = test_input_state();
    add_line(&mut input, 88);
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));

    let err = save_named_session_as_runtime(
        &mut input,
        &mut session_state,
        &target_options.session_file_path(),
        stored_session::SaveAsOverwrite::Deny,
        Instant::now(),
    )
    .expect_err("existing target should require confirmation");

    assert!(
        format!("{err:#}").contains("overwrite confirmation required"),
        "{err:#}"
    );
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(current_options.session_file_path())
    );
    assert!(input.is_session_dirty());
    assert_eq!(
        std::fs::read(target_options.session_file_path()).expect("target unchanged"),
        before
    );
    assert_eq!(loaded_line_x2(&target_options), 17);
}

#[test]
fn runtime_save_as_rejects_existing_sidecar_without_confirmation() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-save-as-sidecar");
    let target_options = named_options(temp.path(), "target-save-as-sidecar");
    std::fs::write(target_options.clear_marker_file_path(), b"stale clear").expect("stale clear");

    let mut input = test_input_state();
    add_line(&mut input, 88);
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));

    let err = save_named_session_as_runtime(
        &mut input,
        &mut session_state,
        &target_options.session_file_path(),
        stored_session::SaveAsOverwrite::Deny,
        Instant::now(),
    )
    .expect_err("existing sidecar should require confirmation");

    assert!(
        format!("{err:#}").contains("overwrite confirmation required"),
        "{err:#}"
    );
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(current_options.session_file_path())
    );
    assert!(input.is_session_dirty());
    assert!(target_options.clear_marker_file_path().exists());
    assert!(!target_options.session_file_path().exists());
}

#[test]
fn runtime_save_as_overwrite_preflight_reports_existing_artifacts() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-save-as-preflight");
    let fresh_options = named_options(temp.path(), "fresh-save-as-preflight");
    let primary_options = named_options(temp.path(), "primary-save-as-preflight");
    let sidecar_options = named_options(temp.path(), "sidecar-save-as-preflight");
    stored_session::save_snapshot(&snapshot_for_board("transparent", 17), &primary_options)
        .expect("save existing target");
    std::fs::write(sidecar_options.clear_marker_file_path(), b"stale clear").expect("stale clear");
    let session_state = SessionState::new(Some(current_options));

    assert!(
        !save_named_session_as_requires_overwrite(
            &session_state,
            &fresh_options.session_file_path()
        )
        .expect("fresh preflight")
    );
    assert!(
        save_named_session_as_requires_overwrite(
            &session_state,
            &primary_options.session_file_path()
        )
        .expect("primary preflight")
    );
    assert!(
        save_named_session_as_requires_overwrite(
            &session_state,
            &sidecar_options.session_file_path()
        )
        .expect("sidecar preflight")
    );
}

#[test]
fn runtime_save_as_overwrite_preflight_skips_current_target() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-save-as-preflight-self");
    std::fs::write(current_options.backup_file_path(), b"stale backup").expect("stale backup");
    let session_state = SessionState::new(Some(current_options.clone()));

    assert!(
        !save_named_session_as_requires_overwrite(
            &session_state,
            &current_options.session_file_path()
        )
        .expect("current target preflight")
    );
}

#[test]
fn runtime_save_as_confirmed_overwrite_removes_stale_sidecars_but_keeps_lock() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-save-as-overwrite");
    let target_options = named_options(temp.path(), "target-save-as-overwrite");
    std::fs::write(target_options.session_file_path(), b"old primary").expect("old primary");
    std::fs::write(target_options.backup_file_path(), b"old backup").expect("old backup");
    std::fs::write(target_options.recovery_file_path(), b"old recovery").expect("old recovery");
    let preserved_recovery = target_options
        .recovery_file_path()
        .with_extension("recovery.preserved");
    std::fs::write(&preserved_recovery, b"old preserved recovery").expect("old preserved recovery");
    std::fs::write(target_options.clear_marker_file_path(), b"old clear").expect("old clear");
    std::fs::write(target_options.lock_file_path(), b"lock").expect("old lock");

    let mut input = test_input_state();
    add_line(&mut input, 63);
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options));

    save_named_session_as_runtime(
        &mut input,
        &mut session_state,
        &target_options.session_file_path(),
        stored_session::SaveAsOverwrite::ConfirmReplace,
        Instant::now(),
    )
    .expect("confirmed overwrite");

    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(target_options.session_file_path())
    );
    assert_eq!(loaded_line_x2(&target_options), 63);
    assert!(!target_options.backup_file_path().exists());
    assert!(!target_options.recovery_file_path().exists());
    assert!(!preserved_recovery.exists());
    assert!(!target_options.clear_marker_file_path().exists());
    assert!(
        target_options.lock_file_path().exists(),
        "Save As must not delete the target lock sidecar"
    );
}

#[test]
fn runtime_save_as_current_target_noop_does_not_require_overwrite_cleanup() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-save-as-self");
    std::fs::write(current_options.backup_file_path(), b"stale backup").expect("stale backup");

    let mut input = test_input_state();
    let mut session_state = SessionState::new(Some(current_options.clone()));
    let report = save_named_session_as_runtime(
        &mut input,
        &mut session_state,
        &current_options.session_file_path(),
        stored_session::SaveAsOverwrite::Deny,
        Instant::now(),
    )
    .expect("current target save as");

    assert!(!report.switched_target);
    assert!(!report.saved);
    assert!(
        current_options.backup_file_path().exists(),
        "current-target Save As must not perform overwrite cleanup"
    );
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(current_options.session_file_path())
    );
}

#[cfg(unix)]
#[test]
fn runtime_save_as_rejects_symlink_before_current_target_shortcut() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-save-as-symlink");
    std::fs::write(current_options.session_file_path(), b"current primary")
        .expect("current primary");
    let symlink_path = temp.path().join("current-save-as-link.wayscriber-session");
    symlink(current_options.session_file_path(), &symlink_path).expect("target symlink");

    let mut input = test_input_state();
    let mut session_state = SessionState::new(Some(current_options.clone()));
    let err = save_named_session_as_runtime(
        &mut input,
        &mut session_state,
        &symlink_path,
        stored_session::SaveAsOverwrite::Deny,
        Instant::now(),
    )
    .expect_err("symlink target should be rejected before identity shortcut");

    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(current_options.session_file_path())
    );
}

#[test]
fn runtime_clear_persists_boundary_then_clears_live_session() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-runtime-clear");
    stored_session::save_snapshot(&snapshot_for_board("transparent", 12), &current_options)
        .expect("seed saved session");
    std::fs::write(current_options.backup_file_path(), b"stale backup").expect("stale backup");
    std::fs::write(current_options.recovery_file_path(), b"stale recovery")
        .expect("stale recovery");

    let mut input = test_input_state();
    add_line(&mut input, 77);
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));
    session_state.mark_loaded(true);
    session_state.record_input_dirty(Instant::now(), true);

    let report = clear_current_session_runtime(&mut input, &mut session_state, Instant::now())
        .expect("runtime clear");

    assert_eq!(report.cleared_path, current_options.session_file_path());
    assert!(report.persisted);
    assert_eq!(input.boards.active_frame().shapes.len(), 0);
    assert!(!input.is_session_dirty());
    assert!(!session_state.is_dirty());
    assert!(session_state.is_loaded());
    assert!(!session_state.has_loaded_board_data());
    assert!(!current_options.session_file_path().exists());
    assert!(current_options.clear_marker_file_path().exists());
    assert!(!current_options.backup_file_path().exists());
    assert!(!current_options.recovery_file_path().exists());
    assert!(matches!(
        stored_session::load_snapshot_with_outcome(&current_options).expect("load after clear"),
        LoadSnapshotOutcome::Empty
    ));
}

#[test]
fn runtime_clear_primary_cleanup_failure_after_marker_still_clears_live_session() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let mut current_options =
        SessionOptions::new(temp.path().to_path_buf(), "runtime-clear-cleanup-fail");
    current_options.persist_transparent = true;
    std::fs::create_dir(current_options.session_file_path())
        .expect("primary cleanup failure fixture");

    let mut input = test_input_state();
    add_line(&mut input, 83);
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));
    session_state.mark_loaded(true);
    session_state.record_input_dirty(Instant::now(), true);

    let report = clear_current_session_runtime(&mut input, &mut session_state, Instant::now())
        .expect("runtime clear should treat durable marker as committed");

    assert_eq!(report.cleared_path, current_options.session_file_path());
    assert_eq!(input.boards.active_frame().shapes.len(), 0);
    assert!(!input.is_session_dirty());
    assert!(!session_state.is_dirty());
    assert!(!session_state.has_loaded_board_data());
    assert!(current_options.clear_marker_file_path().exists());
    assert!(
        current_options.session_file_path().is_dir(),
        "stale primary cleanup can fail after the clear marker commits"
    );
}

#[cfg(unix)]
#[test]
fn runtime_clear_persistence_failure_leaves_live_session_unchanged() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-runtime-clear-fail");
    let current_target = temp.path().join("current-runtime-clear-symlink-target");
    std::fs::write(&current_target, b"preserve current target").expect("write target");
    symlink(&current_target, current_options.session_file_path()).expect("current symlink");

    let mut input = test_input_state();
    add_line(&mut input, 91);
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));
    session_state.mark_loaded(true);
    session_state.record_input_dirty(Instant::now(), true);

    let err = clear_current_session_runtime(&mut input, &mut session_state, Instant::now())
        .expect_err("symlink primary should abort durable clear before memory mutation");

    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    assert_eq!(input.boards.active_frame().shapes.len(), 1);
    assert!(input.is_session_dirty());
    assert!(session_state.is_dirty());
    assert!(session_state.has_loaded_board_data());
    assert_eq!(
        std::fs::read(&current_target).expect("current target bytes"),
        b"preserve current target"
    );
}

#[test]
fn runtime_open_success_commits_target_and_catalog_after_apply() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current");
    let candidate_options = named_options(temp.path(), "candidate");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    let mut session_state = SessionState::new(Some(current_options.clone()));
    let report = open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    assert_eq!(report.previous_path, current_options.session_file_path());
    assert_eq!(report.opened_path, candidate_path);
    assert!(!report.saved_current);
    assert!(report.loaded_board_data);
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(candidate_options.session_file_path())
    );
    assert!(session_state.is_loaded());
    assert!(!session_state.is_dirty());
    assert!(!input.is_session_dirty());
    assert_eq!(input.boards.active_frame().shapes.len(), 1);

    let recent = stored_session::catalog::recent_sessions().expect("recent sessions");
    let candidate_entry = recent
        .iter()
        .find(|entry| entry.path == candidate_options.session_file_path().display().to_string())
        .expect("candidate catalog entry");
    assert!(
        candidate_entry.last_opened_at_millis.is_some(),
        "runtime open should record catalog opened only after commit"
    );
}

#[test]
fn runtime_open_marks_full_damage_after_replacing_boards() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-damage");
    let candidate_options = named_options(temp.path(), "candidate-damage");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    input.update_screen_dimensions(800, 600);
    input
        .dirty_tracker
        .mark_rect(Rect::new(10, 10, 20, 20).expect("small dirty rect"));
    let mut session_state = SessionState::new(Some(current_options));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    assert_eq!(
        input.take_dirty_regions(),
        vec![Rect::new(0, 0, 800, 600).expect("full dirty rect")]
    );
}

#[test]
fn runtime_open_uses_recoverable_backup_without_mutating_candidate_artifacts() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-before-backup-open");
    let source_options = named_options(temp.path(), "source-contentful-backup");
    stored_session::save_snapshot(&sample_snapshot(), &source_options)
        .expect("save source contentful session");
    let backup_bytes =
        std::fs::read(source_options.session_file_path()).expect("source session bytes");

    let candidate_options = named_options(temp.path(), "candidate-recoverable-backup");
    let contentless = stored_session::SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: Vec::new(),
        tool_state: Some(sample_tool_state()),
    };
    stored_session::save_snapshot(&contentless, &candidate_options)
        .expect("save contentless candidate primary");
    std::fs::remove_file(candidate_options.lock_file_path()).expect("remove save-created lock");
    std::fs::write(candidate_options.backup_file_path(), &backup_bytes)
        .expect("seed recoverable backup");
    std::fs::write(
        candidate_options.backup_recovery_marker_file_path(),
        b"recoverable",
    )
    .expect("seed recoverable backup marker");
    let primary_before =
        std::fs::read(candidate_options.session_file_path()).expect("candidate primary bytes");

    let mut input = test_input_state();
    let mut session_state = SessionState::new(Some(current_options));
    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_options.session_file_path(),
        Instant::now(),
    )
    .expect("runtime open");

    assert_eq!(input.boards.active_frame().shapes.len(), 1);
    assert_eq!(
        std::fs::read(candidate_options.session_file_path()).expect("candidate primary unchanged"),
        primary_before
    );
    assert_eq!(
        std::fs::read(candidate_options.backup_file_path()).expect("backup unchanged"),
        backup_bytes
    );
    assert!(
        candidate_options
            .backup_recovery_marker_file_path()
            .exists()
    );
    assert!(
        !candidate_options.lock_file_path().exists(),
        "runtime open must not create a missing candidate lock"
    );
}

#[test]
fn runtime_open_replaces_boards_missing_from_candidate_snapshot() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-with-whiteboard");
    let candidate_options = named_options(temp.path(), "candidate-transparent-only");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    input.switch_board_force(BOARD_ID_WHITEBOARD);
    add_line(&mut input, 77);
    assert_eq!(board_shape_count(&input, BOARD_ID_WHITEBOARD), 1);
    let mut session_state = SessionState::new(Some(current_options));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    assert_eq!(board_shape_count(&input, BOARD_ID_WHITEBOARD), 0);
    assert_eq!(input.board_id(), "transparent");
    assert_eq!(input.boards.active_frame().shapes.len(), 1);
}

#[test]
fn runtime_open_resyncs_canvas_pointer_after_same_active_board_view_offset() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-pointer-cache");
    let mut candidate_options = named_options(temp.path(), "candidate-pointer-cache");
    candidate_options.persist_whiteboard = true;

    let mut candidate_snapshot = snapshot_for_board(BOARD_ID_WHITEBOARD, 77);
    assert!(candidate_snapshot.boards[0].pages.pages[0].set_view_offset(100, 50));
    stored_session::save_snapshot(&candidate_snapshot, &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    input.switch_board_force(BOARD_ID_WHITEBOARD);
    input.update_pointer_position(30, 40);
    assert_eq!(input.canvas_pointer_position(), (30, 40));
    let mut session_state = SessionState::new(Some(current_options));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    assert_eq!(input.board_id(), BOARD_ID_WHITEBOARD);
    assert_eq!(input.boards.active_frame().view_offset(), (100, 50));
    assert_eq!(input.canvas_pointer_position(), (130, 90));
}

#[test]
fn runtime_open_releases_old_board_slots_for_candidate_boards() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-full-board-slots");
    let candidate_options = named_options(temp.path(), "candidate-custom-board");
    let target_board = "session-target-board";
    stored_session::save_snapshot(&snapshot_for_board(target_board, 99), &candidate_options)
        .expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    while input.boards.board_count() < input.boards.max_count() {
        assert!(input.boards.create_board());
    }
    assert_eq!(input.boards.board_count(), input.boards.max_count());
    assert!(!input.boards.has_board(target_board));
    let mut session_state = SessionState::new(Some(current_options));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    assert!(input.boards.has_board(target_board));
    assert_eq!(input.board_id(), target_board);
    assert_eq!(board_shape_count(&input, target_board), 1);
}

#[test]
fn runtime_open_apply_capacity_failure_keeps_current_session_active() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-apply-failure");
    let candidate_options = named_options(temp.path(), "candidate-too-many-boards");

    let input = test_input_state();
    let max_boards = input.boards.max_count();
    let oversized_snapshot = stored_session::SessionSnapshot {
        active_board_id: "candidate-board-0".to_string(),
        boards: (0..=max_boards)
            .map(|index| board_snapshot(&format!("candidate-board-{index}"), index as i32))
            .collect(),
        tool_state: None,
    };
    stored_session::save_snapshot(&oversized_snapshot, &candidate_options)
        .expect("save oversized candidate");
    std::fs::remove_file(candidate_options.lock_file_path()).expect("remove save-created lock");

    let mut input = input;
    add_line(&mut input, 121);
    let mut session_state = SessionState::new(Some(current_options.clone()));

    let err = open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_options.session_file_path(),
        Instant::now(),
    )
    .expect_err("oversized candidate should fail before applying");

    assert!(
        format!("{err:#}").contains("current runtime allows"),
        "{err:#}"
    );
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(current_options.session_file_path())
    );
    assert_eq!(input.board_id(), "transparent");
    assert_eq!(input.boards.active_frame().shapes.len(), 1);
    assert_no_candidate_sidecars(&candidate_options);

    let recent = stored_session::catalog::recent_sessions().expect("recent sessions");
    let candidate_entry = recent
        .iter()
        .find(|entry| entry.path == candidate_options.session_file_path().display().to_string())
        .expect("saved candidate catalog entry");
    assert!(
        candidate_entry.last_opened_at_millis.is_none(),
        "failed apply must not be cataloged as opened"
    );
}

#[test]
fn runtime_open_rejects_full_candidate_snapshot_that_omits_overlay_board() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-overlay-preserved");
    let candidate_options = named_options(temp.path(), "candidate-without-overlay");

    let mut input = test_input_state();
    while input.boards.board_count() < input.boards.max_count() {
        assert!(input.boards.create_board());
    }
    assert_eq!(input.boards.board_count(), input.boards.max_count());
    assert!(input.boards.has_board(BOARD_ID_TRANSPARENT));

    let max_boards = input.boards.max_count();
    let full_non_overlay_snapshot = stored_session::SessionSnapshot {
        active_board_id: "candidate-board-0".to_string(),
        boards: (0..max_boards)
            .map(|index| board_snapshot(&format!("candidate-board-{index}"), index as i32))
            .collect(),
        tool_state: None,
    };
    stored_session::save_snapshot(&full_non_overlay_snapshot, &candidate_options)
        .expect("save full candidate");
    std::fs::remove_file(candidate_options.lock_file_path()).expect("remove save-created lock");

    let mut session_state = SessionState::new(Some(current_options.clone()));
    let err = open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_options.session_file_path(),
        Instant::now(),
    )
    .expect_err("candidate should not evict the overlay board");

    assert!(
        format!("{err:#}").contains("preserving the overlay board"),
        "{err:#}"
    );
    assert!(input.boards.has_board(BOARD_ID_TRANSPARENT));
    assert_eq!(input.boards.board_count(), max_boards);
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(current_options.session_file_path())
    );

    let recent = stored_session::catalog::recent_sessions().expect("recent sessions");
    let candidate_entry = recent
        .iter()
        .find(|entry| entry.path == candidate_options.session_file_path().display().to_string())
        .expect("saved candidate catalog entry");
    assert!(
        candidate_entry.last_opened_at_millis.is_none(),
        "failed overlay-preservation open must not be cataloged as opened"
    );
}

#[test]
fn runtime_open_clears_deleted_page_restore_state_from_previous_session() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-with-deleted-page");
    let candidate_options = named_options(temp.path(), "candidate-after-deleted-page");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    input.boards.active_pages_mut().new_page();
    add_line(&mut input, 88);
    assert_eq!(input.page_delete(), PageDeleteOutcome::Pending);
    assert_eq!(input.page_delete(), PageDeleteOutcome::Removed);
    assert_eq!(input.boards.page_count(), 1);
    let mut session_state = SessionState::new(Some(current_options));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");
    input.restore_deleted_page();

    assert_eq!(input.boards.page_count(), 1);
    assert_eq!(input.boards.active_frame().shapes.len(), 1);
}

#[test]
fn runtime_open_cancels_active_interaction_from_previous_session() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-active-interaction");
    let candidate_options = named_options(temp.path(), "candidate-after-interaction");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    input.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 10,
        start_y: 20,
        points: vec![(10, 20), (30, 40)],
        point_thicknesses: vec![2.0, 2.0],
    };
    assert!(input.has_active_pointer_interaction());
    let mut session_state = SessionState::new(Some(current_options));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    assert!(!input.has_active_pointer_interaction());
    assert!(matches!(input.state, DrawingState::Idle));
    assert_eq!(input.boards.active_frame().shapes.len(), 1);
}

#[test]
fn runtime_open_closes_active_board_picker_drag() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-board-picker-drag");
    let candidate_options = named_options(temp.path(), "candidate-after-board-picker-drag");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    input.open_board_picker();
    assert!(input.board_picker_start_drag(0));
    assert!(input.is_board_picker_open());
    assert!(input.board_picker_is_dragging());
    let mut session_state = SessionState::new(Some(current_options));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    assert!(!input.is_board_picker_open());
    assert!(!input.board_picker_is_dragging());
    assert!(!input.board_picker_is_page_dragging());
    assert_eq!(input.boards.active_frame().shapes.len(), 1);
}

#[test]
fn runtime_open_closes_active_board_picker_page_drag() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-board-picker-page-drag");
    let candidate_options = named_options(temp.path(), "candidate-after-board-picker-page-drag");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    input.open_board_picker();
    assert!(input.board_picker_start_page_drag(0));
    assert!(input.is_board_picker_open());
    assert!(input.board_picker_is_page_dragging());
    let mut session_state = SessionState::new(Some(current_options));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    assert!(!input.is_board_picker_open());
    assert!(!input.board_picker_is_dragging());
    assert!(!input.board_picker_is_page_dragging());
    assert_eq!(input.boards.active_frame().shapes.len(), 1);
}

#[test]
fn runtime_open_saves_current_after_canceling_active_selection_move() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-active-selection-move");
    let candidate_options = named_options(temp.path(), "candidate-after-selection-move");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    let shape_id = add_line(&mut input, 20);
    input.set_selection(vec![shape_id]);
    let snapshots = input.capture_movable_selection_snapshots();
    assert!(input.apply_translation_to_selection(100, 0));
    input.state = DrawingState::MovingSelection {
        last_x: 100,
        last_y: 0,
        snapshots,
        moved: true,
    };
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    let outcome =
        stored_session::load_snapshot_with_outcome(&current_options).expect("load current");
    let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
        panic!("expected saved current session, got {outcome:?}");
    };
    let shape = &snapshot.boards[0].pages.pages[0].shapes[0].shape;
    match shape {
        Shape::Line { x1, y1, x2, y2, .. } => {
            assert_eq!((*x1, *y1, *x2, *y2), (0, 0, 20, 10));
        }
        other => panic!("expected saved line, got {other:?}"),
    }
}

#[test]
fn runtime_open_saves_current_after_canceling_active_text_edit() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-active-text-edit");
    let candidate_options = named_options(temp.path(), "candidate-after-text-edit");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    let shape_id = input.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 80,
        text: "Original".to_string(),
        color: input.current_color,
        size: input.current_font_size,
        font_descriptor: input.font_descriptor.clone(),
        background_enabled: input.text_background_enabled,
        wrap_width: Some(180),
    });
    input.set_selection(vec![shape_id]);
    assert!(input.edit_selected_text());
    let Shape::Text { text, .. } = &input
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("text shape")
        .shape
    else {
        panic!("expected text shape");
    };
    assert!(text.is_empty());
    let DrawingState::TextInput { buffer, .. } = &mut input.state else {
        panic!("expected text input");
    };
    buffer.push_str(" unsaved edit");
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    let outcome =
        stored_session::load_snapshot_with_outcome(&current_options).expect("load current");
    let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
        panic!("expected saved current session, got {outcome:?}");
    };
    let shape = &snapshot.boards[0].pages.pages[0].shapes[0].shape;
    match shape {
        Shape::Text {
            text, wrap_width, ..
        } => {
            assert_eq!(text, "Original");
            assert_eq!(*wrap_width, Some(180));
        }
        other => panic!("expected saved text, got {other:?}"),
    }
}

#[test]
fn runtime_open_saves_current_after_canceling_color_picker_preview() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-color-picker-preview");
    let candidate_options = named_options(temp.path(), "candidate-after-color-picker");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    let original = input.color_for_tool(Tool::Pen);
    input.open_color_picker_popup();
    input.color_picker_popup_set_from_gradient(0.6, 0.1);
    input.color_picker_popup_set_dragging(true);
    assert_ne!(input.color_for_tool(Tool::Pen), original);
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    assert!(!input.is_color_picker_popup_open());
    let outcome =
        stored_session::load_snapshot_with_outcome(&current_options).expect("load current");
    let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
        panic!("expected saved current session, got {outcome:?}");
    };
    let tool_state = snapshot.tool_state.expect("tool state");
    let tool_settings = tool_state.tool_settings.expect("tool settings");
    assert_eq!(tool_state.current_color, original);
    assert_eq!(tool_settings.pen.color, original);
}

#[cfg(unix)]
#[test]
fn runtime_open_current_save_failure_preserves_active_selection_move() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-active-save-fail");
    let current_target = temp.path().join("current-active-symlink-target");
    std::fs::write(&current_target, b"preserve current target").expect("write target");
    symlink(&current_target, current_options.session_file_path()).expect("current symlink");

    let candidate_options = named_options(temp.path(), "candidate-active-save-fail");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");

    let mut input = test_input_state();
    let shape_id = add_line(&mut input, 9);
    input.set_selection(vec![shape_id]);
    let snapshots = input.capture_movable_selection_snapshots();
    assert!(input.apply_translation_to_selection(100, 0));
    input.state = DrawingState::MovingSelection {
        last_x: 100,
        last_y: 0,
        snapshots,
        moved: true,
    };
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));

    let err = open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_options.session_file_path(),
        Instant::now(),
    )
    .expect_err("current save failure should abort open");

    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    assert!(input.has_active_pointer_interaction());
    assert!(matches!(input.state, DrawingState::MovingSelection { .. }));
    let Shape::Line { x1, y1, x2, y2, .. } = &input.boards.active_frame().shapes[0].shape else {
        panic!("expected line");
    };
    assert_eq!((*x1, *y1, *x2, *y2), (100, 0, 109, 10));
    assert!(input.is_session_dirty());
    assert_eq!(
        std::fs::read(&current_target).expect("current target bytes"),
        b"preserve current target"
    );
}

#[cfg(unix)]
#[test]
fn runtime_open_current_save_failure_preserves_spatial_index_for_active_selection_move() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-spatial-save-fail");
    let current_target = temp.path().join("current-spatial-symlink-target");
    std::fs::write(&current_target, b"preserve current target").expect("write target");
    symlink(&current_target, current_options.session_file_path()).expect("current symlink");

    let candidate_options = named_options(temp.path(), "candidate-spatial-save-fail");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");

    let mut input = test_input_state();
    input.set_hit_test_threshold(1);
    let shape_id = add_line(&mut input, 9);
    input.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 1000,
        y: 1000,
        w: 10,
        h: 10,
        fill: false,
        color: input.current_color,
        thick: input.current_thickness,
    });
    input.ensure_spatial_index_for_active_frame();
    assert!(input.has_spatial_index());

    input.set_selection(vec![shape_id]);
    let snapshots = input.capture_movable_selection_snapshots();
    assert!(input.apply_translation_to_selection(200, 0));
    assert!(
        input
            .hit_test_all_for_points(&[(205, 5)], input.hit_test_tolerance)
            .contains(&shape_id)
    );
    input.state = DrawingState::MovingSelection {
        last_x: 200,
        last_y: 0,
        snapshots,
        moved: true,
    };
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));

    let err = open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_options.session_file_path(),
        Instant::now(),
    )
    .expect_err("current save failure should abort open");

    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    assert!(input.has_spatial_index());
    assert!(
        input
            .hit_test_all_for_points(&[(205, 5)], input.hit_test_tolerance)
            .contains(&shape_id),
        "hit testing should use the restored in-progress selection position"
    );
    assert_eq!(
        std::fs::read(&current_target).expect("current target bytes"),
        b"preserve current target"
    );
}

#[test]
fn runtime_open_clears_stale_selection_and_hit_cache() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-selected-shape");
    let candidate_options = named_options(temp.path(), "candidate-reuses-shape-id");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");
    let candidate_path = candidate_options.session_file_path();

    let mut input = test_input_state();
    let old_shape_id = input.boards.active_frame_mut().add_shape(Shape::Line {
        x1: 500,
        y1: 500,
        x2: 550,
        y2: 550,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
        thick: 2.0,
    });
    input.set_selection(vec![old_shape_id]);
    assert!(input.has_selection());
    assert_eq!(input.hit_test_at(510, 510), Some(old_shape_id));
    let mut session_state = SessionState::new(Some(current_options));

    open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_path,
        Instant::now(),
    )
    .expect("runtime open");

    assert!(!input.has_selection());
    assert_eq!(input.hit_test_at(5, 5), Some(1));
}

#[test]
fn runtime_open_candidate_failure_after_current_save_keeps_current_active() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let current_options = named_options(temp.path(), "current-save-first");
    let candidate_options = named_options(temp.path(), "candidate-corrupt");
    std::fs::write(candidate_options.session_file_path(), b"{not valid json")
        .expect("write corrupt candidate");

    let mut input = test_input_state();
    add_line(&mut input, 7);
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));

    let err = open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_options.session_file_path(),
        Instant::now(),
    )
    .expect_err("corrupt candidate should abort after current save");

    assert!(
        format!("{err:#}").contains("failed to parse session json"),
        "{err:#}"
    );
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(current_options.session_file_path())
    );
    assert!(
        current_options.session_file_path().exists(),
        "dirty current session should be saved before candidate load"
    );
    assert!(
        !input.is_session_dirty(),
        "dirty flag may clear after the current session is successfully persisted"
    );
    assert!(!session_state.is_dirty());
    assert_eq!(
        std::fs::read(candidate_options.session_file_path()).expect("candidate bytes"),
        b"{not valid json"
    );
    assert_no_candidate_sidecars(&candidate_options);

    let recent = stored_session::catalog::recent_sessions().expect("recent sessions");
    assert!(
        recent
            .iter()
            .all(|entry| entry.path != candidate_options.session_file_path().display().to_string()),
        "failed candidate must not be cataloged as opened"
    );
}

#[cfg(unix)]
#[test]
fn runtime_open_rejects_readonly_candidate_parent_before_commit() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_dir = temp.path().join("current");
    let candidate_dir = temp.path().join("candidate");
    std::fs::create_dir(&current_dir).expect("create current dir");
    std::fs::create_dir(&candidate_dir).expect("create candidate dir");
    let current_options = named_options(&current_dir, "current");
    let candidate_options = named_options(&candidate_dir, "candidate");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");

    let _guard = readonly_dir_guard(&candidate_dir);
    if can_create_probe(&candidate_dir) {
        return;
    }

    let mut input = test_input_state();
    let mut session_state = SessionState::new(Some(current_options.clone()));
    let err = open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_options.session_file_path(),
        Instant::now(),
    )
    .expect_err("read-only candidate parent should abort runtime open");

    assert!(format!("{err:#}").contains("not writable"), "{err:#}");
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(current_options.session_file_path())
    );
    assert!(!session_state.is_loaded());
    assert!(!input.is_session_dirty());
    assert_eq!(input.boards.active_frame().shapes.len(), 0);
}

#[cfg(unix)]
#[test]
fn runtime_open_current_save_failure_aborts_before_candidate_load() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let current_options = named_options(temp.path(), "current-save-fail");
    let current_target = temp.path().join("current-symlink-target");
    std::fs::write(&current_target, b"preserve current target").expect("write target");
    symlink(&current_target, current_options.session_file_path()).expect("current symlink");

    let candidate_options = named_options(temp.path(), "candidate-valid");
    stored_session::save_snapshot(&sample_snapshot(), &candidate_options).expect("save candidate");

    let mut input = test_input_state();
    add_line(&mut input, 9);
    input.mark_session_dirty();
    let mut session_state = SessionState::new(Some(current_options.clone()));

    let err = open_named_session_runtime(
        &mut input,
        &mut session_state,
        &candidate_options.session_file_path(),
        Instant::now(),
    )
    .expect_err("current save failure should abort open");

    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    assert_eq!(
        session_state
            .options()
            .map(SessionOptions::session_file_path),
        Some(current_options.session_file_path())
    );
    assert!(input.is_session_dirty());
    assert!(!session_state.is_dirty());
    assert_eq!(
        std::fs::read(&current_target).expect("current target bytes"),
        b"preserve current target"
    );
}

#[test]
fn autosave_failure_backoff_delays_retry() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.autosave_enabled = true;
    options.persist_transparent = true;
    options.autosave_idle = Duration::from_millis(1);
    options.autosave_interval = Duration::from_millis(1);
    options.autosave_failure_backoff = Duration::from_millis(50);

    let mut state = SessionState::new(Some(options.clone()));
    let now = Instant::now();
    state.record_input_dirty(now, true);
    state.mark_autosave_failure(now, options.autosave_failure_backoff);

    assert!(!state.autosave_due(now, &options));
    assert_eq!(
        state.autosave_timeout(now, &options),
        Some(options.autosave_failure_backoff)
    );

    let later = now + options.autosave_failure_backoff;
    assert!(state.autosave_due(later, &options));
    assert_eq!(
        state.autosave_timeout(later, &options),
        Some(Duration::from_millis(0))
    );
}

#[test]
fn autosave_deferral_delays_due_without_clearing_dirty() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.autosave_enabled = true;
    options.persist_transparent = true;
    options.autosave_idle = Duration::from_millis(1);
    options.autosave_interval = Duration::from_millis(1);

    let mut state = SessionState::new(Some(options.clone()));
    let now = Instant::now();
    state.record_input_dirty(now, true);
    let due_at = now + Duration::from_millis(2);
    assert!(state.autosave_due(due_at, &options));

    let defer_for = Duration::from_millis(50);
    state.defer_autosave(due_at, defer_for);
    assert!(!state.autosave_due(due_at, &options));
    assert_eq!(state.autosave_timeout(due_at, &options), Some(defer_for));

    let later = due_at + defer_for;
    assert!(state.autosave_due(later, &options));
    assert_eq!(
        state.autosave_timeout(later, &options),
        Some(Duration::ZERO)
    );
}

#[test]
fn mark_saved_clears_autosave_deferral() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.autosave_enabled = true;
    options.persist_transparent = true;

    let mut state = SessionState::new(Some(options));
    let now = Instant::now();
    state.record_input_dirty(now, true);
    state.defer_autosave(now, Duration::from_secs(60));
    state.mark_saved(now, false);

    assert_eq!(state.autosave_deferred_until, None);
}

#[test]
fn protected_session_path_blocks_save_until_session_is_dirty() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.autosave_enabled = true;
    options.persist_transparent = true;
    let path = options.session_file_path();
    let mut state = SessionState::new(Some(options.clone()));

    assert!(!state.should_skip_save_for_protected_path(&path, false));
    state.protect_session_path(path.clone());
    assert!(state.should_skip_save_for_protected_path(&path, false));
    assert!(!state.should_skip_save_for_protected_path(&path, true));

    state.record_input_dirty(Instant::now(), true);
    assert!(!state.should_skip_save_for_protected_path(&path, false));
}

#[test]
fn load_baseline_clears_stale_dirty_before_protected_save_check() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.autosave_enabled = true;
    options.persist_transparent = true;
    let path = options.session_file_path();
    let mut state = SessionState::new(Some(options));

    state.record_input_dirty(Instant::now(), true);
    state.protect_session_path(path.clone());
    assert!(!state.should_skip_save_for_protected_path(&path, false));

    state.mark_clean_after_load();
    assert!(state.should_skip_save_for_protected_path(&path, false));
}

#[test]
fn contentless_save_guard_blocks_clean_unloaded_save_when_primary_exists() {
    assert!(should_skip_unloaded_contentless_save(
        false, false, false, false, true,
    ));
}

#[test]
fn contentless_save_guard_blocks_clean_unloaded_save_when_only_backup_exists() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let options = SessionOptions::new(temp.path().to_path_buf(), "backup-only");
    std::fs::write(options.backup_file_path(), b"backup").expect("backup write");

    assert!(has_session_artifact(&options));
    assert!(should_skip_unloaded_contentless_save(
        false,
        false,
        false,
        false,
        has_session_artifact(&options),
    ));
}

#[test]
fn contentless_save_guard_blocks_clean_unloaded_save_when_only_recovery_exists() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let options = SessionOptions::new(temp.path().to_path_buf(), "recovery-only");
    std::fs::write(options.recovery_file_path(), b"recovery").expect("recovery write");

    assert!(has_session_artifact(&options));
    assert!(should_skip_unloaded_contentless_save(
        false,
        false,
        false,
        false,
        has_session_artifact(&options),
    ));
}

#[test]
fn contentless_save_guard_blocks_clean_loaded_empty_save_when_only_preserved_recovery_exists() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let options = SessionOptions::new(temp.path().to_path_buf(), "preserved-recovery");
    std::fs::write(
        options
            .recovery_file_path()
            .with_extension("recovery.empty"),
        b"recovery",
    )
    .expect("preserved recovery write");

    assert!(has_session_artifact(&options));
    assert!(should_skip_unloaded_contentless_save(
        false,
        false,
        false,
        false,
        has_session_artifact(&options),
    ));
}

#[test]
fn contentless_save_guard_allows_real_board_data_or_dirty_state() {
    assert!(!should_skip_unloaded_contentless_save(
        false, false, false, true, true,
    ));
    assert!(!should_skip_unloaded_contentless_save(
        false, true, false, false, true,
    ));
    assert!(!should_skip_unloaded_contentless_save(
        false, false, true, false, true,
    ));
    assert!(!should_skip_unloaded_contentless_save(
        true, false, false, false, true,
    ));
}

#[test]
fn contentless_save_guard_allows_noop_when_no_session_artifact_exists() {
    assert!(!should_skip_unloaded_contentless_save(
        false, false, false, false, false,
    ));
}
