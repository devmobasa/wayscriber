use wayscriber::i18n::{tr, trf};
use wayscriber::session;
use wayscriber::session::{NamedSessionMoveOutcome, catalog};

use std::path::{Path, PathBuf};

use crate::models::{
    DaemonRuntimeStatus, SessionCatalogActionResult, SessionCatalogItem, SessionCatalogOperation,
};

use super::super::blocking_jobs::{BlockingJobKind, run_blocking};
use super::super::daemon_setup::load_daemon_runtime_status_sync;
use super::{
    load_session_catalog_sync, reserve_inactive_session_operation, service_status_blocker,
};

pub(crate) async fn move_session_catalog_entry(
    id: String,
    target: PathBuf,
) -> Result<SessionCatalogActionResult, String> {
    run_blocking(BlockingJobKind::SessionCatalogMutation, move || {
        let status = load_daemon_runtime_status_sync()?;
        move_session_catalog_entry_sync(&id, &target, &status)
    })
    .await
}

fn move_session_catalog_entry_sync(
    id: &str,
    target: &Path,
    status: &DaemonRuntimeStatus,
) -> Result<SessionCatalogActionResult, String> {
    let initial_items = load_session_catalog_sync()?;
    let item = initial_items
        .iter()
        .find(|item| item.id == id)
        .cloned()
        .ok_or_else(|| tr("Session is no longer in the catalog.").to_string())?;
    reject_catalog_target_collision(&initial_items, id, target)?;

    let _reservation = reserve_inactive_session_operation(SessionCatalogOperation::Move)?;
    if let Some(blocker) = service_status_blocker(Some(status), SessionCatalogOperation::Move) {
        return Err(blocker);
    }

    let outcome = session::move_named_session_non_lock_artifacts(&item.path, target)
        .map_err(|err| err.to_string())?;
    let entry = match catalog::move_session_path_by_id(&item.id, &outcome.target) {
        Ok(Some(entry)) => entry,
        Ok(None) => {
            return rollback_moved_artifacts_after_catalog_failure(
                &item,
                &outcome,
                initial_items,
                tr("the original catalog entry disappeared before metadata could be updated"),
            );
        }
        Err(err) => {
            return rollback_moved_artifacts_after_catalog_failure(
                &item,
                &outcome,
                initial_items,
                &format!("failed to update the session catalog: {err}"),
            );
        }
    };
    let items = match load_session_catalog_sync() {
        Ok(items) => items,
        Err(err) => {
            return Ok(SessionCatalogActionResult::warning(
                trf(
                    "Moved {} to {}, but failed to reload the session catalog: {}",
                    &[&item.display_name, &Path::new(&entry.path).display(), &err],
                ),
                initial_items,
            ));
        }
    };

    Ok(SessionCatalogActionResult::success(
        trf(
            "Moved {} to {}.",
            &[&item.display_name, &Path::new(&entry.path).display()],
        ),
        items,
    ))
}

fn rollback_moved_artifacts_after_catalog_failure(
    item: &SessionCatalogItem,
    outcome: &NamedSessionMoveOutcome,
    initial_items: Vec<SessionCatalogItem>,
    reason: &str,
) -> Result<SessionCatalogActionResult, String> {
    match session::rollback_named_session_non_lock_artifacts_move(outcome) {
        Ok(_) => Err(trf(
            "Move Session failed because {}; rolled back moved files for {}.",
            &[&reason, &item.display_name],
        )),
        Err(rollback_err) => Ok(SessionCatalogActionResult::warning(
            trf(
                "Moved {} to {}, but {}; rollback to {} also failed: {}",
                &[
                    &item.display_name,
                    &outcome.target.display(),
                    &reason,
                    &outcome.source.display(),
                    &rollback_err,
                ],
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
    if let Some(existing) = items
        .iter()
        .find(|item| item.id != source_id && catalog::session_paths_match(&item.path, target))
    {
        return Err(trf(
            "Move Session target is already in the catalog as {}.",
            &[&existing.display_name],
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;

    use wayscriber::session::catalog::CatalogEvent;

    use crate::models::{
        DaemonRuntimeStatus, DesktopEnvironment, LightShortcutApplyCapability,
        ShortcutApplyCapability, ShortcutBackend,
    };
    use crate::test_env::EnvGuard;
    use crate::test_temp;
    use wayscriber::env_vars::{CATALOG_HOOKS_TEST_ENV, XDG_DATA_HOME_ENV, XDG_RUNTIME_DIR_ENV};

    use super::*;

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
        let temp = test_temp::tempdir().unwrap();
        let _env = EnvGuard::set(&[
            (CATALOG_HOOKS_TEST_ENV, temp.path().as_os_str()),
            (XDG_DATA_HOME_ENV, temp.path().as_os_str()),
            (XDG_RUNTIME_DIR_ENV, temp.path().as_os_str()),
        ]);
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("archive.wayscriber-session");
        let source_artifacts = session::named_session_artifact_paths(&source);
        let target_artifacts = session::named_session_artifact_paths(&target);
        fs::write(&source_artifacts.primary, b"primary").unwrap();
        fs::write(&source_artifacts.backup, b"backup").unwrap();
        fs::write(&source_artifacts.lock, b"source lock").unwrap();
        let source_entry = catalog::upsert_session_event_with_display_name(
            &source,
            CatalogEvent::Saved,
            "Lecture",
        )
        .unwrap();

        let result =
            move_session_catalog_entry_sync(&source_entry.id, &target, &inactive_status()).unwrap();

        assert!(!result.warning);
        assert_eq!(fs::read(&target_artifacts.primary).unwrap(), b"primary");
        assert_eq!(fs::read(&target_artifacts.backup).unwrap(), b"backup");
        assert_eq!(fs::read(&source_artifacts.lock).unwrap(), b"source lock");
        assert!(!source_artifacts.primary.exists());
        assert!(!source_artifacts.backup.exists());
        let recents = catalog::recent_sessions().unwrap();
        assert_eq!(recents.len(), 1);
        assert_eq!(recents[0].id, source_entry.id);
        assert_eq!(Path::new(&recents[0].path), target.as_path());
    }

    #[test]
    fn move_session_catalog_entry_failure_keeps_catalog_and_source_artifacts() {
        let temp = test_temp::tempdir().unwrap();
        let _env = EnvGuard::set(&[
            (CATALOG_HOOKS_TEST_ENV, temp.path().as_os_str()),
            (XDG_DATA_HOME_ENV, temp.path().as_os_str()),
            (XDG_RUNTIME_DIR_ENV, temp.path().as_os_str()),
        ]);
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("archive.wayscriber-session");
        let source_artifacts = session::named_session_artifact_paths(&source);
        let target_artifacts = session::named_session_artifact_paths(&target);
        fs::write(&source_artifacts.primary, b"primary").unwrap();
        fs::write(&source_artifacts.backup, b"backup").unwrap();
        fs::write(&target_artifacts.clear_marker, b"cleared").unwrap();
        let source_entry = catalog::upsert_session_event_with_display_name(
            &source,
            CatalogEvent::Saved,
            "Lecture",
        )
        .unwrap();

        let err = move_session_catalog_entry_sync(&source_entry.id, &target, &inactive_status())
            .expect_err("target artifact should block move");

        assert!(err.contains("already has session artifacts"));
        assert_eq!(fs::read(&source_artifacts.primary).unwrap(), b"primary");
        assert_eq!(fs::read(&source_artifacts.backup).unwrap(), b"backup");
        assert_eq!(
            fs::read(&target_artifacts.clear_marker).unwrap(),
            b"cleared"
        );
        let recents = catalog::recent_sessions().unwrap();
        assert_eq!(recents.len(), 1);
        assert_eq!(recents[0].id, source_entry.id);
        assert_eq!(Path::new(&recents[0].path), source.as_path());
    }

    #[test]
    fn move_session_catalog_entry_rejects_catalog_target_collision_before_disk_move() {
        let temp = test_temp::tempdir().unwrap();
        let _env = EnvGuard::set(&[
            (CATALOG_HOOKS_TEST_ENV, temp.path().as_os_str()),
            (XDG_DATA_HOME_ENV, temp.path().as_os_str()),
            (XDG_RUNTIME_DIR_ENV, temp.path().as_os_str()),
        ]);
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("archive.wayscriber-session");
        fs::write(&source, b"primary").unwrap();
        let source_entry = catalog::upsert_session_event_with_display_name(
            &source,
            CatalogEvent::Saved,
            "Lecture",
        )
        .unwrap();
        catalog::upsert_session_event_with_display_name(&target, CatalogEvent::Saved, "Archive")
            .unwrap();

        let err = move_session_catalog_entry_sync(&source_entry.id, &target, &inactive_status())
            .expect_err("target catalog entry should block move");

        assert!(err.contains("already in the catalog"));
        assert_eq!(fs::read(&source).unwrap(), b"primary");
        assert!(!target.exists());
    }

    #[test]
    fn move_session_catalog_entry_rolls_back_when_catalog_update_fails_after_move() {
        let temp = test_temp::tempdir().unwrap();
        let _env = EnvGuard::set(&[
            (CATALOG_HOOKS_TEST_ENV, temp.path().as_os_str()),
            (XDG_DATA_HOME_ENV, temp.path().as_os_str()),
            (XDG_RUNTIME_DIR_ENV, temp.path().as_os_str()),
        ]);
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("archive.wayscriber-session");
        let source_artifacts = session::named_session_artifact_paths(&source);
        fs::write(&source_artifacts.primary, b"primary").unwrap();
        fs::create_dir(&source_artifacts.backup).unwrap();
        let source_entry = catalog::upsert_session_event_with_display_name(
            &source,
            CatalogEvent::Saved,
            "Lecture",
        )
        .unwrap();
        let lock_path = catalog_lock_path();
        fs::remove_file(&lock_path).unwrap();
        fs::create_dir(&lock_path).unwrap();

        let err = move_session_catalog_entry_sync(&source_entry.id, &target, &inactive_status())
            .expect_err("catalog update failure should roll back disk move");

        fs::remove_dir(&lock_path).unwrap();
        assert!(err.contains("failed to update the session catalog"));
        assert!(err.contains("rolled back moved files"));
        assert_eq!(fs::read(&source).unwrap(), b"primary");
        assert!(source_artifacts.backup.is_dir());
        assert!(!target.exists());
        let recents = catalog::recent_sessions().unwrap();
        assert_eq!(recents.len(), 1);
        assert_eq!(Path::new(&recents[0].path), source.as_path());
    }

    fn catalog_lock_path() -> PathBuf {
        let catalog_path = catalog::catalog_path();
        let mut raw = OsString::from(catalog_path.as_os_str());
        raw.push(".lock");
        raw.into()
    }
}
