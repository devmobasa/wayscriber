use std::path::{Path, PathBuf};

use crate::models::{DaemonRuntimeStatus, SessionCatalogActionResult};

use super::{
    CatalogOperation, RuntimeLockKind, RuntimeLockPaths,
    acquire_runtime_lock_for_inactive_operation, load_session_catalog_sync, service_status_blocker,
};

use super::super::daemon_setup::load_daemon_runtime_status;

pub(crate) fn duplicate_session_catalog_entry(
    id: String,
    target: PathBuf,
    paths: &wayscriber::paths::PathResolver,
) -> Result<SessionCatalogActionResult, String> {
    let status = load_daemon_runtime_status(paths)?;
    let catalog = wayscriber::session::catalog::SessionCatalog::from_resolver(paths)
        .map_err(|error| error.to_string())?;
    let runtime_locks = RuntimeLockPaths::prepare(paths)?;
    duplicate_session_catalog_entry_sync(&id, &target, &status, &catalog, &runtime_locks)
}

fn duplicate_session_catalog_entry_sync(
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
    let _daemon_lock = acquire_runtime_lock_for_inactive_operation(
        runtime_locks,
        RuntimeLockKind::Daemon,
        CatalogOperation::Duplicate,
    )?;
    let _overlay_lock = acquire_runtime_lock_for_inactive_operation(
        runtime_locks,
        RuntimeLockKind::Overlay,
        CatalogOperation::Duplicate,
    )?;
    if let Some(blocker) = service_status_blocker(Some(status), CatalogOperation::Duplicate) {
        return Err(blocker);
    }

    let outcome = wayscriber::session::duplicate_named_session_primary(&item.path, target)
        .map_err(|err| err.to_string())?;
    let entry = match catalog.upsert_session_event_with_display_name(
        &outcome.target,
        wayscriber::session::catalog::CatalogEvent::Saved,
        &item.display_name,
    ) {
        Ok(entry) => entry,
        Err(err) => {
            return Ok(SessionCatalogActionResult::warning(
                format!(
                    "Duplicated {} to {}, but failed to update the session catalog: {err}",
                    item.display_name,
                    outcome.target.display()
                ),
                initial_items,
            ));
        }
    };
    let items = match load_session_catalog_sync(catalog) {
        Ok(items) => items,
        Err(err) => {
            return Ok(SessionCatalogActionResult::warning(
                format!(
                    "Duplicated {} to {}, but failed to reload the session catalog: {err}",
                    item.display_name,
                    Path::new(&entry.path).display()
                ),
                initial_items,
            ));
        }
    };
    Ok(SessionCatalogActionResult::success(
        format!(
            "Duplicated {} to {}.",
            item.display_name,
            Path::new(&entry.path).display()
        ),
        items,
    ))
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
    fn duplicate_session_catalog_entry_copies_primary_and_catalogs_new_entry() {
        let temp = crate::test_temp::tempdir()
            .expect("the duplicate-session test fixture operation should succeed");
        let (catalog, runtime_locks) = fixture_owners(&temp);
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("lecture-copy.wayscriber-session");
        let source_artifacts = wayscriber::session::named_session_artifact_paths(&source);
        let target_artifacts = wayscriber::session::named_session_artifact_paths(&target);
        std::fs::write(&source_artifacts.primary, b"primary")
            .expect("the duplicate-session test fixture operation should succeed");
        std::fs::write(&source_artifacts.backup, b"backup")
            .expect("the duplicate-session test fixture operation should succeed");
        std::fs::write(&source_artifacts.lock, b"lock")
            .expect("the duplicate-session test fixture operation should succeed");
        let source_entry = catalog
            .upsert_session_event_with_display_name(
                &source,
                wayscriber::session::catalog::CatalogEvent::Saved,
                "Lecture",
            )
            .expect("the duplicate-session test fixture operation should succeed");

        let result = duplicate_session_catalog_entry_sync(
            &source_entry.id,
            &target,
            &inactive_status(),
            &catalog,
            &runtime_locks,
        )
        .expect("the duplicate-session test fixture operation should succeed");

        assert_eq!(
            std::fs::read(&target_artifacts.primary)
                .expect("the duplicate-session test fixture operation should succeed"),
            b"primary"
        );
        assert!(!target_artifacts.backup.exists());
        assert!(!target_artifacts.lock.exists());
        assert!(result.message.contains("Duplicated Lecture"));
        let recents = catalog
            .recent_sessions()
            .expect("the duplicate-session test fixture operation should succeed");
        assert_eq!(recents.len(), 2);
        assert_eq!(
            recents
                .iter()
                .filter(|entry| entry.display_name == "Lecture")
                .count(),
            2
        );
        assert_ne!(recents[0].id, recents[1].id);
    }

    #[test]
    fn duplicate_session_catalog_entry_warns_when_catalog_update_fails_after_copy() {
        let temp = crate::test_temp::tempdir()
            .expect("the duplicate-session test fixture operation should succeed");
        let (catalog, runtime_locks) = fixture_owners(&temp);
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("lecture-copy.wayscriber-session");
        std::fs::write(&source, b"primary")
            .expect("the duplicate-session test fixture operation should succeed");
        let source_entry = catalog
            .upsert_session_event_with_display_name(
                &source,
                wayscriber::session::catalog::CatalogEvent::Saved,
                "Lecture",
            )
            .expect("the duplicate-session test fixture operation should succeed");
        let lock_path = catalog_lock_path(&catalog);
        std::fs::remove_file(&lock_path)
            .expect("the duplicate-session test fixture operation should succeed");
        std::fs::create_dir(&lock_path)
            .expect("the duplicate-session test fixture operation should succeed");

        let result = duplicate_session_catalog_entry_sync(
            &source_entry.id,
            &target,
            &inactive_status(),
            &catalog,
            &runtime_locks,
        )
        .expect("the duplicate-session test fixture operation should succeed");

        std::fs::remove_dir(&lock_path)
            .expect("the duplicate-session test fixture operation should succeed");
        assert!(result.warning);
        assert!(
            result
                .message
                .contains("failed to update the session catalog")
        );
        assert_eq!(
            std::fs::read(&target)
                .expect("the duplicate-session test fixture operation should succeed"),
            b"primary"
        );
        assert_eq!(
            result.items.len(),
            1,
            "warning should keep the pre-copy catalog rows visible"
        );
        assert_eq!(
            catalog
                .recent_sessions()
                .expect("the duplicate-session test fixture operation should succeed")
                .len(),
            1
        );
    }

    fn catalog_lock_path(
        catalog: &wayscriber::session::catalog::SessionCatalog,
    ) -> std::path::PathBuf {
        let mut raw = OsString::from(catalog.path().as_os_str());
        raw.push(".lock");
        raw.into()
    }
}
