use std::path::{Path, PathBuf};

use crate::models::{DaemonRuntimeStatus, SessionCatalogActionResult};

use super::{
    CatalogOperation, RuntimeLockKind, acquire_runtime_lock_for_inactive_operation,
    load_session_catalog_sync, service_status_blocker,
};

use super::super::daemon_setup::load_daemon_runtime_status;

pub(crate) async fn duplicate_session_catalog_entry(
    id: String,
    target: PathBuf,
) -> Result<SessionCatalogActionResult, String> {
    let status = load_daemon_runtime_status().await?;
    duplicate_session_catalog_entry_sync(&id, &target, &status)
}

fn duplicate_session_catalog_entry_sync(
    id: &str,
    target: &Path,
    status: &DaemonRuntimeStatus,
) -> Result<SessionCatalogActionResult, String> {
    let initial_items = load_session_catalog_sync()?;
    let item = initial_items
        .iter()
        .find(|item| item.id == id)
        .cloned()
        .ok_or_else(|| "Session is no longer in the catalog.".to_string())?;
    let _daemon_lock = acquire_runtime_lock_for_inactive_operation(
        RuntimeLockKind::Daemon,
        CatalogOperation::Duplicate,
    )?;
    let _overlay_lock = acquire_runtime_lock_for_inactive_operation(
        RuntimeLockKind::Overlay,
        CatalogOperation::Duplicate,
    )?;
    if let Some(blocker) = service_status_blocker(Some(status), CatalogOperation::Duplicate) {
        return Err(blocker);
    }

    let outcome = wayscriber::session::duplicate_named_session_primary(&item.path, target)
        .map_err(|err| err.to_string())?;
    let entry = match wayscriber::session::catalog::upsert_session_event_with_display_name(
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
    let items = match load_session_catalog_sync() {
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
    use std::path::Path;
    use std::sync::MutexGuard;

    use crate::models::{
        DaemonRuntimeStatus, DesktopEnvironment, LightShortcutApplyCapability,
        ShortcutApplyCapability, ShortcutBackend,
    };

    use super::*;

    struct EnvGuard {
        catalog_hooks: Option<OsString>,
        xdg_data_home: Option<OsString>,
        xdg_runtime_dir: Option<OsString>,
        _guard: MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn set_roots(path: &Path) -> Self {
            let guard = crate::test_env::lock();
            let catalog_hooks = std::env::var_os("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS");
            let xdg_data_home = std::env::var_os("XDG_DATA_HOME");
            let xdg_runtime_dir = std::env::var_os("XDG_RUNTIME_DIR");
            unsafe {
                std::env::set_var("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS", path);
                std::env::set_var("XDG_DATA_HOME", path);
                std::env::set_var("XDG_RUNTIME_DIR", path);
            }
            Self {
                catalog_hooks,
                xdg_data_home,
                xdg_runtime_dir,
                _guard: guard,
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
            match self.xdg_runtime_dir.take() {
                Some(value) => unsafe { std::env::set_var("XDG_RUNTIME_DIR", value) },
                None => unsafe { std::env::remove_var("XDG_RUNTIME_DIR") },
            }
        }
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
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = EnvGuard::set_roots(temp.path());
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("lecture-copy.wayscriber-session");
        let source_artifacts = wayscriber::session::named_session_artifact_paths(&source);
        let target_artifacts = wayscriber::session::named_session_artifact_paths(&target);
        std::fs::write(&source_artifacts.primary, b"primary").unwrap();
        std::fs::write(&source_artifacts.backup, b"backup").unwrap();
        std::fs::write(&source_artifacts.lock, b"lock").unwrap();
        let source_entry = wayscriber::session::catalog::upsert_session_event_with_display_name(
            &source,
            wayscriber::session::catalog::CatalogEvent::Saved,
            "Lecture",
        )
        .unwrap();

        let result =
            duplicate_session_catalog_entry_sync(&source_entry.id, &target, &inactive_status())
                .unwrap();

        assert_eq!(
            std::fs::read(&target_artifacts.primary).unwrap(),
            b"primary"
        );
        assert!(!target_artifacts.backup.exists());
        assert!(!target_artifacts.lock.exists());
        assert!(result.message.contains("Duplicated Lecture"));
        let recents = wayscriber::session::catalog::recent_sessions().unwrap();
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
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = EnvGuard::set_roots(temp.path());
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("lecture-copy.wayscriber-session");
        std::fs::write(&source, b"primary").unwrap();
        let source_entry = wayscriber::session::catalog::upsert_session_event_with_display_name(
            &source,
            wayscriber::session::catalog::CatalogEvent::Saved,
            "Lecture",
        )
        .unwrap();
        let lock_path = catalog_lock_path();
        std::fs::remove_file(&lock_path).unwrap();
        std::fs::create_dir(&lock_path).unwrap();

        let result =
            duplicate_session_catalog_entry_sync(&source_entry.id, &target, &inactive_status())
                .unwrap();

        std::fs::remove_dir(&lock_path).unwrap();
        assert!(result.warning);
        assert!(
            result
                .message
                .contains("failed to update the session catalog")
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"primary");
        assert_eq!(
            result.items.len(),
            1,
            "warning should keep the pre-copy catalog rows visible"
        );
        assert_eq!(
            wayscriber::session::catalog::recent_sessions()
                .unwrap()
                .len(),
            1
        );
    }

    fn catalog_lock_path() -> std::path::PathBuf {
        let catalog_path = wayscriber::session::catalog::catalog_path();
        let mut raw = OsString::from(catalog_path.as_os_str());
        raw.push(".lock");
        raw.into()
    }
}
