use super::*;

#[test]
fn session_catalog_state_replaces_items_and_inputs() {
    let mut state = SessionCatalogState::loading();
    let item = SessionCatalogItem {
        id: "s-1".to_string(),
        display_name: "Lecture".to_string(),
        path: PathBuf::from("/tmp/lecture.wayscriber-session"),
        path_label: "/tmp/lecture.wayscriber-session".to_string(),
        canonical_path_label: None,
        created_label: "now".to_string(),
        last_opened_label: "Never".to_string(),
        last_saved_label: "Never".to_string(),
        artifacts: SessionArtifactSummary {
            primary_exists: false,
            backup_exists: false,
            recovery_exists: false,
            clear_marker_exists: false,
            lock_exists: false,
            non_lock_size_bytes: 0,
        },
    };

    state.replace_items(vec![item]);

    assert!(!state.is_loading);
    assert_eq!(state.rename_value("s-1", ""), "Lecture");
    assert_eq!(
        state.duplicate_value("s-1", Path::new("unused")),
        "/tmp/lecture copy.wayscriber-session"
    );
    assert_eq!(
        state.move_value("s-1", Path::new("unused")),
        "/tmp/lecture moved.wayscriber-session"
    );
    assert!(state.item("s-1").is_some());
}

#[test]
fn default_session_target_paths_handle_nonstandard_names() {
    assert_eq!(
        default_duplicate_target_path(Path::new("/tmp/lecture.session")),
        PathBuf::from("/tmp/lecture.session copy")
    );
    assert_eq!(
        default_duplicate_target_path(Path::new("lecture.wayscriber-session")),
        PathBuf::from("lecture copy.wayscriber-session")
    );
    assert_eq!(
        default_move_target_path(Path::new("/tmp/lecture.session")),
        PathBuf::from("/tmp/lecture.session moved")
    );
    assert_eq!(
        default_move_target_path(Path::new("lecture.wayscriber-session")),
        PathBuf::from("lecture moved.wayscriber-session")
    );
}

#[test]
fn artifact_summary_reports_non_lock_artifacts_and_size() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("lecture.wayscriber-session");
    let artifacts = wayscriber::session::named_session_artifact_paths(&path);
    std::fs::write(&artifacts.primary, b"primary").unwrap();
    std::fs::write(&artifacts.backup, b"backup").unwrap();
    std::fs::write(&artifacts.recovery, b"recovery").unwrap();
    std::fs::write(&artifacts.clear_marker, b"cleared").unwrap();
    std::fs::write(&artifacts.lock, b"lock").unwrap();

    let summary = SessionArtifactSummary::from_primary_path(&path).unwrap();

    assert!(summary.primary_exists);
    assert!(summary.backup_exists);
    assert!(summary.recovery_exists);
    assert!(summary.clear_marker_exists);
    assert!(summary.lock_exists);
    assert_eq!(
        summary.non_lock_size_bytes,
        "primary".len() as u64
            + "backup".len() as u64
            + "recovery".len() as u64
            + "cleared".len() as u64
    );
}

#[test]
fn format_byte_count_uses_compact_units() {
    assert_eq!(format_byte_count(14), "14 B");
    assert_eq!(format_byte_count(4096), "4.0 KiB");
    assert_eq!(format_byte_count(2 * 1024 * 1024), "2.0 MiB");
}
