use super::*;
use crate::draw::{Color, Frame, Shape};
use crate::session::{BoardPagesSnapshot, BoardSnapshot, SessionOptions, SessionSnapshot};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

struct EnvGuard {
    _guard: MutexGuard<'static, ()>,
    catalog_hooks: Option<std::ffi::OsString>,
    xdg_data_home: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set_xdg_data_home(path: &Path) -> Self {
        let guard = crate::test_env::lock();
        let catalog_hooks = std::env::var_os("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS");
        let xdg_data_home = std::env::var_os("XDG_DATA_HOME");
        unsafe {
            std::env::set_var("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS", path);
            std::env::set_var("XDG_DATA_HOME", path);
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
            Some(value) => unsafe {
                std::env::set_var("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS", value)
            },
            None => unsafe { std::env::remove_var("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS") },
        }
        match self.xdg_data_home.take() {
            Some(value) => unsafe { std::env::set_var("XDG_DATA_HOME", value) },
            None => unsafe { std::env::remove_var("XDG_DATA_HOME") },
        }
    }
}

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

fn named_options(temp: &Path, name: &str) -> SessionOptions {
    let mut options = SessionOptions::new(temp.join("configured"), name);
    options.persist_transparent = true;
    options.set_named_file_target(temp.join(format!("{name}.wayscriber-session")));
    options
}

#[test]
fn catalog_path_honors_xdg_data_home() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());

    assert_eq!(
        catalog_path(),
        temp.path().join("wayscriber").join("sessions.json")
    );
}

#[test]
fn malformed_catalog_is_not_clobbered_by_upsert_failure() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let path = catalog_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"{not valid json").unwrap();

    let session = temp.path().join("session.wayscriber-session");
    fs::write(&session, b"{}").unwrap();
    let err = upsert_session_event(&session, CatalogEvent::Opened)
        .expect_err("malformed catalog should reject mutation");

    assert!(format!("{err:#}").contains("failed to parse session catalog"));
    assert_eq!(fs::read(&path).unwrap(), b"{not valid json");
}

#[test]
fn equivalent_existing_paths_dedupe_after_canonicalization() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let sessions = temp.path().join("sessions");
    fs::create_dir(&sessions).unwrap();
    let session = sessions.join("lecture.wayscriber-session");
    fs::write(&session, b"{}").unwrap();
    let equivalent = sessions
        .join("..")
        .join("sessions")
        .join("lecture.wayscriber-session");

    upsert_session_event(&equivalent, CatalogEvent::Opened).unwrap();
    upsert_session_event(&session, CatalogEvent::Saved).unwrap();

    let recents = recent_sessions().unwrap();
    assert_eq!(recents.len(), 1);
    assert!(recents[0].last_opened_at_millis.is_some());
    assert!(recents[0].last_saved_at_millis.is_some());
}

#[test]
fn missing_target_identity_uses_canonical_parent_plus_filename() {
    let temp = crate::test_temp::tempdir().unwrap();
    let parent = temp.path().join("sessions");
    fs::create_dir(&parent).unwrap();
    let missing = parent.join("new-session.wayscriber-session");

    let identity = session_path_identity(&missing);

    assert_eq!(
        identity.canonical_path.as_deref(),
        Some(
            parent
                .canonicalize()
                .unwrap()
                .join("new-session.wayscriber-session")
                .as_path()
        )
    );
    assert!(
        !missing.exists(),
        "identity calculation must not create target"
    );
}

#[test]
fn duplicate_display_names_are_allowed_for_distinct_paths() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let left = temp.path().join("left");
    let right = temp.path().join("right");
    fs::create_dir(&left).unwrap();
    fs::create_dir(&right).unwrap();
    let left_session = left.join("lecture.wayscriber-session");
    let right_session = right.join("lecture.wayscriber-session");
    fs::write(&left_session, b"{}").unwrap();
    fs::write(&right_session, b"{}").unwrap();

    upsert_session_event(&left_session, CatalogEvent::Opened).unwrap();
    upsert_session_event(&right_session, CatalogEvent::Opened).unwrap();

    let recents = recent_sessions().unwrap();
    assert_eq!(recents.len(), 2);
    assert!(
        recents
            .iter()
            .all(|entry| entry.display_name == "lecture.wayscriber-session")
    );
}

#[test]
fn upsert_with_display_name_creates_distinct_named_entry() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let source = temp.path().join("source.wayscriber-session");
    let duplicate = temp.path().join("duplicate.wayscriber-session");
    fs::write(&source, b"{}").unwrap();
    fs::write(&duplicate, b"{}").unwrap();

    let source_entry =
        upsert_session_event_with_display_name(&source, CatalogEvent::Saved, "Lecture").unwrap();
    let duplicate_entry =
        upsert_session_event_with_display_name(&duplicate, CatalogEvent::Saved, "Lecture").unwrap();

    assert_ne!(source_entry.id, duplicate_entry.id);
    let recents = recent_sessions().unwrap();
    assert_eq!(recents.len(), 2);
    assert!(recents.iter().all(|entry| entry.display_name == "Lecture"));
}

#[test]
fn forget_by_path_removes_metadata_only() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let session = temp.path().join("session.wayscriber-session");
    let backup = {
        let mut raw = std::ffi::OsString::from(session.as_os_str());
        raw.push(".bak");
        PathBuf::from(raw)
    };
    fs::write(&session, b"{}").unwrap();
    fs::write(&backup, b"backup").unwrap();
    upsert_session_event(&session, CatalogEvent::Saved).unwrap();

    assert!(forget_session_by_path(&session).unwrap());

    assert!(recent_sessions().unwrap().is_empty());
    assert!(session.exists());
    assert!(backup.exists());
}

#[test]
fn rename_display_name_changes_metadata_only_and_allows_duplicates() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let left = temp.path().join("left.wayscriber-session");
    let right = temp.path().join("right.wayscriber-session");
    fs::write(&left, b"{}").unwrap();
    fs::write(&right, b"{}").unwrap();
    let left_entry = upsert_session_event(&left, CatalogEvent::Saved).unwrap();
    upsert_session_event(&right, CatalogEvent::Saved).unwrap();

    let renamed =
        rename_session_display_name_by_id(&left_entry.id, "Lecture").expect("rename should work");

    assert_eq!(renamed.expect("renamed entry").display_name, "Lecture");
    let recents = recent_sessions().unwrap();
    assert_eq!(recents.len(), 2);
    assert_eq!(
        recents
            .iter()
            .filter(|entry| entry.display_name == "Lecture")
            .count(),
        1
    );
    assert!(left.exists(), "rename should not touch primary file");
    assert!(right.exists(), "rename should not touch sibling file");
}

#[test]
fn renamed_display_name_survives_later_upsert() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let session = temp.path().join("lecture.wayscriber-session");
    fs::write(&session, b"{}").unwrap();
    let entry = upsert_session_event(&session, CatalogEvent::Saved).unwrap();

    rename_session_display_name_by_id(&entry.id, "Lecture 04").unwrap();
    upsert_session_event(&session, CatalogEvent::Opened).unwrap();

    let recents = recent_sessions().unwrap();
    assert_eq!(recents.len(), 1);
    assert_eq!(recents[0].display_name, "Lecture 04");
    assert!(recents[0].last_opened_at_millis.is_some());
    assert!(recents[0].last_saved_at_millis.is_some());
}

#[test]
fn rename_display_name_rejects_empty_names() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let session = temp.path().join("lecture.wayscriber-session");
    fs::write(&session, b"{}").unwrap();
    let entry = upsert_session_event(&session, CatalogEvent::Saved).unwrap();

    let err =
        rename_session_display_name_by_id(&entry.id, "  ").expect_err("empty rename should fail");

    assert!(format!("{err:#}").contains("display name cannot be empty"));
}

#[test]
fn failed_temp_write_leaves_existing_catalog_intact() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("sessions.json");
    fs::write(&path, br#"{"version":1,"sessions":[]}"#).unwrap();
    let tmp_path = temp.path().join("missing").join("sessions.json.tmp");
    let mut catalog = CatalogFile::default();
    catalog
        .upsert(
            &temp.path().join("session.wayscriber-session"),
            CatalogEvent::Saved,
            None,
        )
        .unwrap();

    let err = save_catalog_atomic_with_temp_path(&path, &tmp_path, &catalog)
        .expect_err("temp write should fail");

    assert!(format!("{err:#}").contains("temporary session catalog"));
    assert_eq!(fs::read(&path).unwrap(), br#"{"version":1,"sessions":[]}"#);
}

#[test]
fn named_backup_fallback_load_records_catalog_open() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let options = named_options(temp.path(), "backup-open");
    crate::session::save_snapshot(&sample_snapshot(), &options).unwrap();
    fs::rename(options.session_file_path(), options.backup_file_path()).unwrap();

    let loaded = crate::session::load_snapshot(&options)
        .unwrap()
        .expect("backup fallback should load");

    assert!(loaded.has_board_data());
    let recents = recent_sessions().unwrap();
    assert_eq!(recents.len(), 1);
    assert!(
        recents[0].last_opened_at_millis.is_some(),
        "backup fallback loads should update recents"
    );
}

#[test]
fn named_recovery_fallback_load_records_catalog_open() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = EnvGuard::set_xdg_data_home(temp.path());
    let options = named_options(temp.path(), "recovery-open");
    crate::session::save_snapshot(&sample_snapshot(), &options).unwrap();
    fs::rename(options.session_file_path(), options.recovery_file_path()).unwrap();

    let loaded = crate::session::load_snapshot(&options)
        .unwrap()
        .expect("recovery fallback should load");

    assert!(loaded.has_board_data());
    let recents = recent_sessions().unwrap();
    assert_eq!(recents.len(), 1);
    assert!(
        recents[0].last_opened_at_millis.is_some(),
        "recovery fallback loads should update recents"
    );
}
