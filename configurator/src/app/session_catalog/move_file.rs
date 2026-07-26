use std::path::{Path, PathBuf};

use crate::models::{DaemonRuntimeStatus, SessionCatalogActionResult, SessionCatalogItem};

use super::super::daemon_setup::load_daemon_runtime_status;
use super::{
    CatalogOperation, RuntimeLockKind, RuntimeLockPaths,
    acquire_runtime_lock_for_inactive_operation, load_session_catalog_sync, service_status_blocker,
};

pub(crate) fn move_session_catalog_entry(
    id: String,
    target: PathBuf,
    paths: &wayscriber::paths::PathResolver,
) -> Result<SessionCatalogActionResult, String> {
    let status = load_daemon_runtime_status(paths)?;
    let catalog = wayscriber::session::catalog::SessionCatalog::from_resolver(paths)
        .map_err(|error| error.to_string())?;
    let runtime_locks = RuntimeLockPaths::prepare(paths)?;
    move_session_catalog_entry_sync(&id, &target, &status, &catalog, &runtime_locks)
}

fn move_session_catalog_entry_sync(
    id: &str,
    target: &Path,
    status: &DaemonRuntimeStatus,
    catalog: &wayscriber::session::catalog::SessionCatalog,
    runtime_locks: &RuntimeLockPaths,
) -> Result<SessionCatalogActionResult, String> {
    let initial_items = load_session_catalog_sync(catalog)?;
    let item = initial_items
        .iter()
        .find(|item| item.id == id)
        .cloned()
        .ok_or_else(|| "Session is no longer in the catalog.".to_string())?;
    reject_catalog_target_collision(&initial_items, id, target)?;

    let _daemon_lock = acquire_runtime_lock_for_inactive_operation(
        runtime_locks,
        RuntimeLockKind::Daemon,
        CatalogOperation::Move,
    )?;
    let _overlay_lock = acquire_runtime_lock_for_inactive_operation(
        runtime_locks,
        RuntimeLockKind::Overlay,
        CatalogOperation::Move,
    )?;
    if let Some(blocker) = service_status_blocker(Some(status), CatalogOperation::Move) {
        return Err(blocker);
    }

    let outcome = wayscriber::session::move_named_session_non_lock_artifacts(&item.path, target)
        .map_err(|err| err.to_string())?;
    let entry = match catalog.move_session_path_by_id(&item.id, &outcome.target) {
        Ok(Some(entry)) => entry,
        Ok(None) => {
            return rollback_moved_artifacts_after_catalog_failure(
                &item,
                &outcome,
                initial_items,
                "the original catalog entry disappeared before metadata could be updated"
                    .to_string(),
            );
        }
        Err(err) => {
            return rollback_moved_artifacts_after_catalog_failure(
                &item,
                &outcome,
                initial_items,
                format!("failed to update the session catalog: {err}"),
            );
        }
    };
    let items = match load_session_catalog_sync(catalog) {
        Ok(items) => items,
        Err(err) => {
            return Ok(SessionCatalogActionResult::warning(
                format!(
                    "Moved {} to {}, but failed to reload the session catalog: {err}",
                    item.display_name,
                    Path::new(&entry.path).display()
                ),
                initial_items,
            ));
        }
    };

    Ok(SessionCatalogActionResult::success(
        format!(
            "Moved {} to {}.",
            item.display_name,
            Path::new(&entry.path).display()
        ),
        items,
    ))
}

fn rollback_moved_artifacts_after_catalog_failure(
    item: &SessionCatalogItem,
    outcome: &wayscriber::session::NamedSessionMoveOutcome,
    initial_items: Vec<SessionCatalogItem>,
    reason: String,
) -> Result<SessionCatalogActionResult, String> {
    match wayscriber::session::rollback_named_session_non_lock_artifacts_move(outcome) {
        Ok(_) => Err(format!(
            "Move Session failed because {reason}; rolled back moved files for {}.",
            item.display_name
        )),
        Err(rollback_err) => Ok(SessionCatalogActionResult::warning(
            format!(
                "Moved {} to {}, but {reason}; rollback to {} also failed: {rollback_err:#}",
                item.display_name,
                outcome.target.display(),
                outcome.source.display()
            ),
            initial_items,
        )),
    }
}

fn reject_catalog_target_collision(
    items: &[SessionCatalogItem],
    source_id: &str,
    target: &Path,
) -> Result<(), String> {
    if let Some(existing) = items.iter().find(|item| {
        item.id != source_id
            && wayscriber::session::catalog::session_paths_match(&item.path, target)
    }) {
        return Err(format!(
            "Move Session target is already in the catalog as {}.",
            existing.display_name
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;
    use crate::models::{
        DaemonRuntimeStatus, DesktopEnvironment, LightShortcutApplyCapability,
        ShortcutApplyCapability, ShortcutBackend,
    };

    fn fixture_owners(
        temp: &crate::test_temp::TempDir,
    ) -> (
        wayscriber::session::catalog::SessionCatalog,
        RuntimeLockPaths,
    ) {
        let paths = wayscriber::paths::PathResolver::from_environment(
            wayscriber::paths::PathEnvironment::from_values(&[
                (wayscriber::env_vars::HOME_ENV, temp.path().as_os_str()),
                (
                    wayscriber::env_vars::XDG_DATA_HOME_ENV,
                    temp.path().as_os_str(),
                ),
                (
                    wayscriber::env_vars::XDG_RUNTIME_DIR_ENV,
                    temp.path().as_os_str(),
                ),
            ]),
        );
        (
            wayscriber::session::catalog::SessionCatalog::from_resolver(&paths)
                .expect("fixture resolves its catalog"),
            RuntimeLockPaths::prepare(&paths).expect("fixture prepares runtime locks"),
        )
    }

    fn inactive_status() -> DaemonRuntimeStatus {
        DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Unknown,
            shortcut_backend: ShortcutBackend::Manual,
            shortcut_apply_capability: ShortcutApplyCapability::Manual,
            light_shortcut_apply_capability: LightShortcutApplyCapability::Manual,
            systemctl_available: false,
            gsettings_available: false,
            service_installed: false,
            service_enabled: false,
            service_active: false,
            service_unit_path: None,
            configured_shortcut: None,
            light_controls_configured: false,
            light_controls_config_path: None,
        }
    }

    #[test]
    fn move_session_catalog_entry_moves_artifacts_and_preserves_catalog_id() {
        let temp = crate::test_temp::tempdir()
            .expect("the move-session test fixture operation should succeed");
        let (catalog, runtime_locks) = fixture_owners(&temp);
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("archive.wayscriber-session");
        let source_artifacts = wayscriber::session::named_session_artifact_paths(&source);
        let target_artifacts = wayscriber::session::named_session_artifact_paths(&target);
        std::fs::write(&source_artifacts.primary, b"primary")
            .expect("the move-session test fixture operation should succeed");
        std::fs::write(&source_artifacts.backup, b"backup")
            .expect("the move-session test fixture operation should succeed");
        std::fs::write(&source_artifacts.lock, b"source lock")
            .expect("the move-session test fixture operation should succeed");
        let source_entry = catalog
            .upsert_session_event_with_display_name(
                &source,
                wayscriber::session::catalog::CatalogEvent::Saved,
                "Lecture",
            )
            .expect("the move-session test fixture operation should succeed");

        let result = move_session_catalog_entry_sync(
            &source_entry.id,
            &target,
            &inactive_status(),
            &catalog,
            &runtime_locks,
        )
        .expect("the move-session test fixture operation should succeed");

        assert!(!result.warning);
        assert_eq!(
            std::fs::read(&target_artifacts.primary)
                .expect("the move-session test fixture operation should succeed"),
            b"primary"
        );
        assert_eq!(
            std::fs::read(&target_artifacts.backup)
                .expect("the move-session test fixture operation should succeed"),
            b"backup"
        );
        assert_eq!(
            std::fs::read(&source_artifacts.lock)
                .expect("the move-session test fixture operation should succeed"),
            b"source lock"
        );
        assert!(!source_artifacts.primary.exists());
        assert!(!source_artifacts.backup.exists());
        let recents = catalog
            .recent_sessions()
            .expect("the move-session test fixture operation should succeed");
        assert_eq!(recents.len(), 1);
        assert_eq!(recents[0].id, source_entry.id);
        assert_eq!(Path::new(&recents[0].path), target.as_path());
    }

    #[test]
    fn move_session_catalog_entry_failure_keeps_catalog_and_source_artifacts() {
        let temp = crate::test_temp::tempdir()
            .expect("the move-session test fixture operation should succeed");
        let (catalog, runtime_locks) = fixture_owners(&temp);
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("archive.wayscriber-session");
        let source_artifacts = wayscriber::session::named_session_artifact_paths(&source);
        let target_artifacts = wayscriber::session::named_session_artifact_paths(&target);
        std::fs::write(&source_artifacts.primary, b"primary")
            .expect("the move-session test fixture operation should succeed");
        std::fs::write(&source_artifacts.backup, b"backup")
            .expect("the move-session test fixture operation should succeed");
        std::fs::write(&target_artifacts.clear_marker, b"cleared")
            .expect("the move-session test fixture operation should succeed");
        let source_entry = catalog
            .upsert_session_event_with_display_name(
                &source,
                wayscriber::session::catalog::CatalogEvent::Saved,
                "Lecture",
            )
            .expect("the move-session test fixture operation should succeed");

        let err = move_session_catalog_entry_sync(
            &source_entry.id,
            &target,
            &inactive_status(),
            &catalog,
            &runtime_locks,
        )
        .expect_err("target artifact should block move");

        assert!(err.contains("already has session artifacts"));
        assert_eq!(
            std::fs::read(&source_artifacts.primary)
                .expect("the move-session test fixture operation should succeed"),
            b"primary"
        );
        assert_eq!(
            std::fs::read(&source_artifacts.backup)
                .expect("the move-session test fixture operation should succeed"),
            b"backup"
        );
        assert_eq!(
            std::fs::read(&target_artifacts.clear_marker)
                .expect("the move-session test fixture operation should succeed"),
            b"cleared"
        );
        let recents = catalog
            .recent_sessions()
            .expect("the move-session test fixture operation should succeed");
        assert_eq!(recents.len(), 1);
        assert_eq!(recents[0].id, source_entry.id);
        assert_eq!(Path::new(&recents[0].path), source.as_path());
    }

    #[test]
    fn move_session_catalog_entry_rejects_catalog_target_collision_before_disk_move() {
        let temp = crate::test_temp::tempdir()
            .expect("the move-session test fixture operation should succeed");
        let (catalog, runtime_locks) = fixture_owners(&temp);
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("archive.wayscriber-session");
        std::fs::write(&source, b"primary")
            .expect("the move-session test fixture operation should succeed");
        let source_entry = catalog
            .upsert_session_event_with_display_name(
                &source,
                wayscriber::session::catalog::CatalogEvent::Saved,
                "Lecture",
            )
            .expect("the move-session test fixture operation should succeed");
        catalog
            .upsert_session_event_with_display_name(
                &target,
                wayscriber::session::catalog::CatalogEvent::Saved,
                "Archive",
            )
            .expect("the move-session test fixture operation should succeed");

        let err = move_session_catalog_entry_sync(
            &source_entry.id,
            &target,
            &inactive_status(),
            &catalog,
            &runtime_locks,
        )
        .expect_err("target catalog entry should block move");

        assert!(err.contains("already in the catalog"));
        assert_eq!(
            std::fs::read(&source).expect("the move-session test fixture operation should succeed"),
            b"primary"
        );
        assert!(!target.exists());
    }

    #[test]
    fn move_session_catalog_entry_rolls_back_when_catalog_update_fails_after_move() {
        let temp = crate::test_temp::tempdir()
            .expect("the move-session test fixture operation should succeed");
        let (catalog, runtime_locks) = fixture_owners(&temp);
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("archive.wayscriber-session");
        let source_artifacts = wayscriber::session::named_session_artifact_paths(&source);
        std::fs::write(&source_artifacts.primary, b"primary")
            .expect("the move-session test fixture operation should succeed");
        std::fs::create_dir(&source_artifacts.backup)
            .expect("the move-session test fixture operation should succeed");
        let source_entry = catalog
            .upsert_session_event_with_display_name(
                &source,
                wayscriber::session::catalog::CatalogEvent::Saved,
                "Lecture",
            )
            .expect("the move-session test fixture operation should succeed");
        let lock_path = catalog_lock_path(&catalog);
        std::fs::remove_file(&lock_path)
            .expect("the move-session test fixture operation should succeed");
        std::fs::create_dir(&lock_path)
            .expect("the move-session test fixture operation should succeed");

        let err = move_session_catalog_entry_sync(
            &source_entry.id,
            &target,
            &inactive_status(),
            &catalog,
            &runtime_locks,
        )
        .expect_err("catalog update failure should roll back disk move");

        std::fs::remove_dir(&lock_path)
            .expect("the move-session test fixture operation should succeed");
        assert!(err.contains("failed to update the session catalog"));
        assert!(err.contains("rolled back moved files"));
        assert_eq!(
            std::fs::read(&source).expect("the move-session test fixture operation should succeed"),
            b"primary"
        );
        assert!(source_artifacts.backup.is_dir());
        assert!(!target.exists());
        let recents = catalog
            .recent_sessions()
            .expect("the move-session test fixture operation should succeed");
        assert_eq!(recents.len(), 1);
        assert_eq!(Path::new(&recents[0].path), source.as_path());
    }

    fn catalog_lock_path(catalog: &wayscriber::session::catalog::SessionCatalog) -> PathBuf {
        let mut raw = OsString::from(catalog.path().as_os_str());
        raw.push(".lock");
        raw.into()
    }
}
