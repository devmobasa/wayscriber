use super::*;
use crate::draw::{Color, Frame, Shape};
use crate::session::{BoardPagesSnapshot, BoardSnapshot, SessionOptions, SessionSnapshot};
use std::fs;
use std::path::{Path, PathBuf};

fn catalog_for(root: &Path) -> SessionCatalog {
    SessionCatalog::at_path(root.join("wayscriber").join("sessions.json"))
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
fn generated_catalog_id_extends_past_existing_timestamp_collisions() {
    let now = 0x2a;
    let base = format!("s-{:x}-{now:x}-0", std::process::id());
    let mut catalog = CatalogFile {
        version: CATALOG_VERSION,
        sessions: vec![
            CatalogEntry {
                id: base.clone(),
                display_name: "first fixture".to_string(),
                path: "/fixture/first".to_string(),
                canonical_path: None,
                created_at_millis: now,
                last_opened_at_millis: None,
                last_saved_at_millis: None,
            },
            CatalogEntry {
                id: format!("{base}f"),
                display_name: "second fixture".to_string(),
                path: "/fixture/second".to_string(),
                canonical_path: None,
                created_at_millis: now,
                last_opened_at_millis: None,
                last_saved_at_millis: None,
            },
        ],
    };

    let generated = catalog.generated_catalog_id(now);

    assert_eq!(generated, format!("{base}ff"));
    catalog.sessions.push(CatalogEntry {
        id: generated,
        display_name: "generated fixture".to_string(),
        path: "/fixture/generated".to_string(),
        canonical_path: None,
        created_at_millis: now,
        last_opened_at_millis: None,
        last_saved_at_millis: None,
    });
    assert_eq!(catalog.generated_catalog_id(now), format!("{base}fff"));
}

#[test]
fn catalog_path_honors_xdg_data_home() {
    let temp = crate::test_temp::tempdir().unwrap();
    let catalog = catalog_for(temp.path());

    assert_eq!(
        catalog.path().to_path_buf(),
        temp.path().join("wayscriber").join("sessions.json")
    );
}

#[test]
fn malformed_catalog_is_not_clobbered_by_upsert_failure() {
    let temp = crate::test_temp::tempdir().unwrap();
    let catalog = catalog_for(temp.path());
    let path = catalog.path().to_path_buf();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"{not valid json").unwrap();

    let session = temp.path().join("session.wayscriber-session");
    fs::write(&session, b"{}").unwrap();
    let err = catalog
        .upsert_session_event(&session, CatalogEvent::Opened)
        .expect_err("malformed catalog should reject mutation");

    assert!(format!("{err:#}").contains("failed to parse session catalog"));
    assert_eq!(fs::read(&path).unwrap(), b"{not valid json");
}

#[test]
fn equivalent_existing_paths_dedupe_after_canonicalization() {
    let temp = crate::test_temp::tempdir().unwrap();
    let catalog = catalog_for(temp.path());
    let sessions = temp.path().join("sessions");
    fs::create_dir(&sessions).unwrap();
    let session = sessions.join("lecture.wayscriber-session");
    fs::write(&session, b"{}").unwrap();
    let equivalent = sessions
        .join("..")
        .join("sessions")
        .join("lecture.wayscriber-session");

    catalog
        .upsert_session_event(&equivalent, CatalogEvent::Opened)
        .unwrap();
    catalog
        .upsert_session_event(&session, CatalogEvent::Saved)
        .unwrap();

    let recents = catalog.recent_sessions().unwrap();
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
    let catalog = catalog_for(temp.path());
    let left = temp.path().join("left");
    let right = temp.path().join("right");
    fs::create_dir(&left).unwrap();
    fs::create_dir(&right).unwrap();
    let left_session = left.join("lecture.wayscriber-session");
    let right_session = right.join("lecture.wayscriber-session");
    fs::write(&left_session, b"{}").unwrap();
    fs::write(&right_session, b"{}").unwrap();

    catalog
        .upsert_session_event(&left_session, CatalogEvent::Opened)
        .unwrap();
    catalog
        .upsert_session_event(&right_session, CatalogEvent::Opened)
        .unwrap();

    let recents = catalog.recent_sessions().unwrap();
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
    let catalog = catalog_for(temp.path());
    let source = temp.path().join("source.wayscriber-session");
    let duplicate = temp.path().join("duplicate.wayscriber-session");
    fs::write(&source, b"{}").unwrap();
    fs::write(&duplicate, b"{}").unwrap();

    let source_entry = catalog
        .upsert_session_event_with_display_name(&source, CatalogEvent::Saved, "Lecture")
        .unwrap();
    let duplicate_entry = catalog
        .upsert_session_event_with_display_name(&duplicate, CatalogEvent::Saved, "Lecture")
        .unwrap();

    assert_ne!(source_entry.id, duplicate_entry.id);
    let recents = catalog.recent_sessions().unwrap();
    assert_eq!(recents.len(), 2);
    assert!(recents.iter().all(|entry| entry.display_name == "Lecture"));
}

#[test]
fn forget_by_path_removes_metadata_only() {
    let temp = crate::test_temp::tempdir().unwrap();
    let catalog = catalog_for(temp.path());
    let session = temp.path().join("session.wayscriber-session");
    let backup = {
        let mut raw = std::ffi::OsString::from(session.as_os_str());
        raw.push(".bak");
        PathBuf::from(raw)
    };
    fs::write(&session, b"{}").unwrap();
    fs::write(&backup, b"backup").unwrap();
    catalog
        .upsert_session_event(&session, CatalogEvent::Saved)
        .unwrap();

    assert!(catalog.forget_session_by_path(&session).unwrap());

    assert!(catalog.recent_sessions().unwrap().is_empty());
    assert!(session.exists());
    assert!(backup.exists());
}

#[test]
fn rename_display_name_changes_metadata_only_and_allows_duplicates() {
    let temp = crate::test_temp::tempdir().unwrap();
    let catalog = catalog_for(temp.path());
    let left = temp.path().join("left.wayscriber-session");
    let right = temp.path().join("right.wayscriber-session");
    fs::write(&left, b"{}").unwrap();
    fs::write(&right, b"{}").unwrap();
    let left_entry = catalog
        .upsert_session_event(&left, CatalogEvent::Saved)
        .unwrap();
    catalog
        .upsert_session_event(&right, CatalogEvent::Saved)
        .unwrap();

    let renamed = catalog
        .rename_session_display_name_by_id(&left_entry.id, "Lecture")
        .expect("rename should work");

    assert_eq!(renamed.expect("renamed entry").display_name, "Lecture");
    let recents = catalog.recent_sessions().unwrap();
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
fn move_session_path_by_id_preserves_id_and_display_name() {
    let temp = crate::test_temp::tempdir().unwrap();
    let catalog = catalog_for(temp.path());
    let source = temp.path().join("lecture.wayscriber-session");
    let target = temp.path().join("archive.wayscriber-session");
    fs::write(&source, b"{}").unwrap();
    let entry = catalog
        .upsert_session_event_with_display_name(&source, CatalogEvent::Saved, "Lecture")
        .unwrap();
    fs::rename(&source, &target).unwrap();

    let moved = catalog
        .move_session_path_by_id(&entry.id, &target)
        .unwrap()
        .expect("entry should move");

    assert_eq!(moved.id, entry.id);
    assert_eq!(moved.display_name, "Lecture");
    assert_eq!(
        Path::new(&moved.path),
        session_path_identity(&target).exact_path
    );
    let recents = catalog.recent_sessions().unwrap();
    assert_eq!(recents.len(), 1);
    assert_eq!(recents[0].id, entry.id);
    assert_eq!(
        Path::new(&recents[0].path),
        session_path_identity(&target).exact_path
    );
}

#[test]
fn move_session_path_by_id_rejects_catalog_target_collision() {
    let temp = crate::test_temp::tempdir().unwrap();
    let catalog = catalog_for(temp.path());
    let source = temp.path().join("lecture.wayscriber-session");
    let target = temp.path().join("archive.wayscriber-session");
    fs::write(&source, b"{}").unwrap();
    fs::write(&target, b"{}").unwrap();
    let source_entry = catalog
        .upsert_session_event_with_display_name(&source, CatalogEvent::Saved, "Lecture")
        .unwrap();
    let target_entry = catalog
        .upsert_session_event_with_display_name(&target, CatalogEvent::Saved, "Archive")
        .unwrap();

    let err = catalog
        .move_session_path_by_id(&source_entry.id, &target)
        .expect_err("target catalog entry should block move metadata update");

    assert!(format!("{err:#}").contains("already present in the catalog"));
    let recents = catalog.recent_sessions().unwrap();
    assert_eq!(recents.len(), 2);
    assert!(recents.iter().any(|entry| entry.id == source_entry.id));
    assert!(recents.iter().any(|entry| entry.id == target_entry.id));
}

#[test]
fn renamed_display_name_survives_later_upsert() {
    let temp = crate::test_temp::tempdir().unwrap();
    let catalog = catalog_for(temp.path());
    let session = temp.path().join("lecture.wayscriber-session");
    fs::write(&session, b"{}").unwrap();
    let entry = catalog
        .upsert_session_event(&session, CatalogEvent::Saved)
        .unwrap();

    catalog
        .rename_session_display_name_by_id(&entry.id, "Lecture 04")
        .unwrap();
    catalog
        .upsert_session_event(&session, CatalogEvent::Opened)
        .unwrap();

    let recents = catalog.recent_sessions().unwrap();
    assert_eq!(recents.len(), 1);
    assert_eq!(recents[0].display_name, "Lecture 04");
    assert!(recents[0].last_opened_at_millis.is_some());
    assert!(recents[0].last_saved_at_millis.is_some());
}

#[test]
fn rename_display_name_rejects_empty_names() {
    let temp = crate::test_temp::tempdir().unwrap();
    let catalog = catalog_for(temp.path());
    let session = temp.path().join("lecture.wayscriber-session");
    fs::write(&session, b"{}").unwrap();
    let entry = catalog
        .upsert_session_event(&session, CatalogEvent::Saved)
        .unwrap();

    let err = catalog
        .rename_session_display_name_by_id(&entry.id, "  ")
        .expect_err("empty rename should fail");

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
    let catalog = catalog_for(temp.path());
    let options = named_options(temp.path(), "backup-open");
    crate::session::save_snapshot(&sample_snapshot(), &options).unwrap();
    fs::rename(options.session_file_path(), options.backup_file_path()).unwrap();

    let loaded = crate::session::load_snapshot(&options)
        .unwrap()
        .expect("backup fallback should load");
    catalog.record_named_session_opened(&options);

    assert!(loaded.has_board_data());
    let recents = catalog.recent_sessions().unwrap();
    assert_eq!(recents.len(), 1);
    assert!(
        recents[0].last_opened_at_millis.is_some(),
        "backup fallback loads should update recents"
    );
}

#[test]
fn named_recovery_fallback_load_records_catalog_open() {
    let temp = crate::test_temp::tempdir().unwrap();
    let catalog = catalog_for(temp.path());
    let options = named_options(temp.path(), "recovery-open");
    crate::session::save_snapshot(&sample_snapshot(), &options).unwrap();
    fs::rename(options.session_file_path(), options.recovery_file_path()).unwrap();

    let loaded = crate::session::load_snapshot(&options)
        .unwrap()
        .expect("recovery fallback should load");
    catalog.record_named_session_opened(&options);

    assert!(loaded.has_board_data());
    let recents = catalog.recent_sessions().unwrap();
    assert_eq!(recents.len(), 1);
    assert!(
        recents[0].last_opened_at_millis.is_some(),
        "recovery fallback loads should update recents"
    );
}
