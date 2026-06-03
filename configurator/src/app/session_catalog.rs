use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use wayscriber::session::try_lock_exclusive;

use crate::models::{DaemonRuntimeStatus, SessionCatalogActionResult, SessionCatalogItem};

use super::daemon_setup::load_daemon_runtime_status;
mod duplicate;

pub(super) use duplicate::duplicate_session_catalog_entry;

pub(super) async fn load_session_catalog() -> Result<Vec<SessionCatalogItem>, String> {
    load_session_catalog_sync()
}

pub(super) async fn forget_session_catalog_entry(
    id: String,
) -> Result<SessionCatalogActionResult, String> {
    let removed =
        wayscriber::session::catalog::forget_session_by_id(&id).map_err(|err| err.to_string())?;
    let items = load_session_catalog_sync()?;
    Ok(SessionCatalogActionResult::success(
        if removed {
            "Forgot session metadata.".to_string()
        } else {
            "Session was already absent from the catalog.".to_string()
        },
        items,
    ))
}

pub(super) async fn rename_session_catalog_entry(
    id: String,
    display_name: String,
) -> Result<SessionCatalogActionResult, String> {
    let renamed =
        wayscriber::session::catalog::rename_session_display_name_by_id(&id, display_name.trim())
            .map_err(|err| err.to_string())?;
    let items = load_session_catalog_sync()?;
    Ok(SessionCatalogActionResult::success(
        if let Some(entry) = renamed {
            format!("Renamed session to {}.", entry.display_name)
        } else {
            "Session was already absent from the catalog.".to_string()
        },
        items,
    ))
}

pub(super) async fn reveal_session_catalog_entry(
    id: String,
) -> Result<SessionCatalogActionResult, String> {
    let item = find_session_catalog_item(&id)?;
    reveal_path_parent(&item.path)?;
    let items = load_session_catalog_sync()?;
    Ok(SessionCatalogActionResult::success(
        format!("Opened folder for {}.", item.display_name),
        items,
    ))
}

pub(super) async fn clear_session_catalog_entry(
    id: String,
) -> Result<SessionCatalogActionResult, String> {
    let status = load_daemon_runtime_status().await?;
    let item = find_session_catalog_item(&id)?;
    let _daemon_lock = acquire_runtime_lock_for_inactive_operation(
        RuntimeLockKind::Daemon,
        CatalogOperation::Clear,
    )?;
    let _overlay_lock = acquire_runtime_lock_for_inactive_operation(
        RuntimeLockKind::Overlay,
        CatalogOperation::Clear,
    )?;
    if let Some(blocker) = service_status_blocker(Some(&status), CatalogOperation::Clear) {
        return Err(blocker);
    }

    let outcome = wayscriber::session::clear_named_session_non_lock_artifacts(&item.path)
        .map_err(|err| err.to_string())?;
    let items = load_session_catalog_sync()?;
    Ok(SessionCatalogActionResult::success(
        if outcome.removed_any() {
            format!("Cleared saved data for {}.", item.display_name)
        } else {
            format!("No saved data found for {}.", item.display_name)
        },
        items,
    ))
}

pub(super) fn session_clear_blocker(status: Option<&DaemonRuntimeStatus>) -> Option<String> {
    inactive_operation_blocker(status, CatalogOperation::Clear)
}

pub(super) fn session_duplicate_blocker(status: Option<&DaemonRuntimeStatus>) -> Option<String> {
    inactive_operation_blocker(status, CatalogOperation::Duplicate)
}

fn inactive_operation_blocker(
    status: Option<&DaemonRuntimeStatus>,
    operation: CatalogOperation,
) -> Option<String> {
    match runtime_lock_active(RuntimeLockKind::Overlay, operation) {
        Ok(true) => {
            return Some(
                operation
                    .running_message(RuntimeLockKind::Overlay)
                    .to_string(),
            );
        }
        Ok(false) => {}
        Err(err) => return Some(err),
    }

    match runtime_lock_active(RuntimeLockKind::Daemon, operation) {
        Ok(true) => {
            return Some(
                operation
                    .running_message(RuntimeLockKind::Daemon)
                    .to_string(),
            );
        }
        Ok(false) => {}
        Err(err) => return Some(err),
    }

    service_status_blocker(status, operation)
}

pub(super) fn session_clear_cached_status_blocker(
    status: Option<&DaemonRuntimeStatus>,
) -> Option<String> {
    service_status_blocker(status, CatalogOperation::Clear)
}

pub(super) fn session_duplicate_cached_status_blocker(
    status: Option<&DaemonRuntimeStatus>,
) -> Option<String> {
    service_status_blocker(status, CatalogOperation::Duplicate)
}

fn service_status_blocker(
    status: Option<&DaemonRuntimeStatus>,
    operation: CatalogOperation,
) -> Option<String> {
    match status {
        Some(status) if status.service_active => Some(
            operation
                .running_message(RuntimeLockKind::Daemon)
                .to_string(),
        ),
        Some(_) => None,
        None => Some(operation.waiting_for_status_message().to_string()),
    }
}

fn load_session_catalog_sync() -> Result<Vec<SessionCatalogItem>, String> {
    wayscriber::session::catalog::recent_sessions()
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(SessionCatalogItem::from_entry)
        .collect()
}

fn find_session_catalog_item(id: &str) -> Result<SessionCatalogItem, String> {
    load_session_catalog_sync()?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "Session is no longer in the catalog.".to_string())
}

fn reveal_path_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        return Err(format!(
            "Session folder does not exist: {}",
            parent.display()
        ));
    }

    Command::new("xdg-open")
        .arg(parent)
        .spawn()
        .map_err(|err| format!("failed to launch xdg-open: {err}"))?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeLockKind {
    Daemon,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CatalogOperation {
    Clear,
    Duplicate,
}

impl RuntimeLockKind {
    fn path(self) -> PathBuf {
        match self {
            Self::Daemon => wayscriber::paths::daemon_lock_file(),
            Self::Overlay => wayscriber::paths::overlay_lock_file(),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Daemon => "daemon",
            Self::Overlay => "overlay",
        }
    }
}

impl CatalogOperation {
    fn running_message(self, kind: RuntimeLockKind) -> &'static str {
        match (self, kind) {
            (Self::Clear, RuntimeLockKind::Daemon) => {
                "Clear saved data is disabled while the background service or a manually started daemon is running. Stop it first or clear from the overlay."
            }
            (Self::Clear, RuntimeLockKind::Overlay) => {
                "Clear saved data is disabled while an overlay is running. Use the overlay Clear action for the active session."
            }
            (Self::Duplicate, RuntimeLockKind::Daemon) => {
                "Duplicate Session is disabled while the background service or a manually started daemon is running. Stop it first or duplicate from the overlay after opening the session."
            }
            (Self::Duplicate, RuntimeLockKind::Overlay) => {
                "Duplicate Session is disabled while an overlay is running. Use Save As from the overlay for the active session."
            }
        }
    }

    fn waiting_for_status_message(self) -> &'static str {
        match self {
            Self::Clear => {
                "Clear saved data is disabled until background service status finishes loading."
            }
            Self::Duplicate => {
                "Duplicate Session is disabled until background service status finishes loading."
            }
        }
    }
}

fn runtime_lock_active(kind: RuntimeLockKind, operation: CatalogOperation) -> Result<bool, String> {
    let path = kind.path();
    let file = match OpenOptions::new().read(true).write(true).open(&path) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(format!(
                "{} is disabled because the {} lock could not be inspected: {} ({err})",
                operation.label(),
                kind.label(),
                path.display()
            ));
        }
    };

    match try_lock_exclusive(&file) {
        Ok(()) => {
            drop_lock(file);
            Ok(false)
        }
        Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(true),
        Err(err) => Err(format!(
            "{} is disabled because the {} lock could not be checked: {} ({err})",
            operation.label(),
            kind.label(),
            path.display()
        )),
    }
}

#[cfg(test)]
fn acquire_runtime_lock_for_clear(kind: RuntimeLockKind) -> Result<File, String> {
    acquire_runtime_lock_for_inactive_operation(kind, CatalogOperation::Clear)
}

fn acquire_runtime_lock_for_inactive_operation(
    kind: RuntimeLockKind,
    operation: CatalogOperation,
) -> Result<File, String> {
    let path = kind.path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "{} is disabled because the {} lock directory could not be created: {} ({err})",
                operation.label(),
                kind.label(),
                parent.display()
            )
        })?;
    }

    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|err| {
            format!(
                "{} is disabled because the {} lock could not be opened: {} ({err})",
                operation.label(),
                kind.label(),
                path.display()
            )
        })?;

    match try_lock_exclusive(&file) {
        Ok(()) => Ok(file),
        Err(err) if err.kind() == ErrorKind::WouldBlock => {
            Err(operation.running_message(kind).to_string())
        }
        Err(err) => Err(format!(
            "{} is disabled because the {} lock could not be reserved: {} ({err})",
            operation.label(),
            kind.label(),
            path.display()
        )),
    }
}

impl CatalogOperation {
    fn label(self) -> &'static str {
        match self {
            Self::Clear => "Clear saved data",
            Self::Duplicate => "Duplicate Session",
        }
    }
}

fn drop_lock(file: File) {
    drop(file);
}

pub(super) fn session_artifact_status_label(item: &SessionCatalogItem) -> String {
    item.artifacts.status_label()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::sync::MutexGuard;

    use crate::models::{
        DesktopEnvironment, LightShortcutApplyCapability, ShortcutApplyCapability, ShortcutBackend,
    };

    struct RuntimeEnvGuard {
        previous: Option<OsString>,
        _guard: MutexGuard<'static, ()>,
    }

    impl RuntimeEnvGuard {
        fn set_xdg_runtime_dir(path: &Path) -> Self {
            let guard = crate::test_env::lock();
            let previous = std::env::var_os("XDG_RUNTIME_DIR");
            unsafe {
                std::env::set_var("XDG_RUNTIME_DIR", path);
            }
            Self {
                previous,
                _guard: guard,
            }
        }
    }

    impl Drop for RuntimeEnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => unsafe { std::env::set_var("XDG_RUNTIME_DIR", value) },
                None => unsafe { std::env::remove_var("XDG_RUNTIME_DIR") },
            }
        }
    }

    fn daemon_status(active: bool) -> DaemonRuntimeStatus {
        DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Unknown,
            shortcut_backend: ShortcutBackend::Manual,
            shortcut_apply_capability: ShortcutApplyCapability::Manual,
            light_shortcut_apply_capability: LightShortcutApplyCapability::Manual,
            systemctl_available: false,
            gsettings_available: false,
            service_installed: active,
            service_enabled: active,
            service_active: active,
            service_unit_path: None,
            configured_shortcut: None,
            light_controls_configured: false,
            light_controls_config_path: None,
        }
    }

    #[test]
    fn session_clear_blocker_blocks_running_daemon() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let status = daemon_status(true);

        let blocker = session_clear_blocker(Some(&status)).expect("daemon should block clear");

        assert!(blocker.contains("background service"));
    }

    #[test]
    fn session_clear_blocker_allows_inactive_daemon_when_overlay_lock_is_free() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let status = daemon_status(false);

        let blocker = session_clear_blocker(Some(&status));

        assert!(blocker.is_none(), "{blocker:?}");
    }

    #[test]
    fn session_duplicate_blocker_uses_duplicate_status_message() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let blocker =
            session_duplicate_blocker(None).expect("unknown status should block duplicate");

        assert!(blocker.contains("Duplicate Session"));
    }

    #[test]
    fn session_clear_blocker_blocks_unknown_daemon_status() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let blocker = session_clear_blocker(None).expect("unknown status should block clear");

        assert!(blocker.contains("status finishes loading"));
    }

    #[test]
    fn session_clear_blocker_blocks_manual_daemon_lock() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let _daemon_lock = acquire_runtime_lock_for_clear(RuntimeLockKind::Daemon).unwrap();
        let status = daemon_status(false);

        let blocker = session_clear_blocker(Some(&status)).expect("daemon lock should block clear");

        assert!(blocker.contains("manually started daemon"));
    }

    #[test]
    fn cached_status_blocker_does_not_probe_runtime_locks() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let _daemon_lock = acquire_runtime_lock_for_clear(RuntimeLockKind::Daemon).unwrap();
        let _overlay_lock = acquire_runtime_lock_for_clear(RuntimeLockKind::Overlay).unwrap();
        let status = daemon_status(false);

        let blocker = session_clear_cached_status_blocker(Some(&status));

        assert!(blocker.is_none(), "{blocker:?}");
    }

    #[test]
    fn clear_runtime_guards_hold_daemon_and_overlay_locks() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let _daemon_lock = acquire_runtime_lock_for_clear(RuntimeLockKind::Daemon).unwrap();
        let _overlay_lock = acquire_runtime_lock_for_clear(RuntimeLockKind::Overlay).unwrap();

        assert!(matches!(
            runtime_lock_active(RuntimeLockKind::Daemon, CatalogOperation::Clear),
            Ok(true)
        ));
        assert!(matches!(
            runtime_lock_active(RuntimeLockKind::Overlay, CatalogOperation::Clear),
            Ok(true)
        ));
    }

    #[test]
    fn session_artifact_status_label_reports_size_when_present() {
        let item = SessionCatalogItem {
            id: "s-1".to_string(),
            display_name: "Lecture".to_string(),
            path: PathBuf::from("/tmp/lecture.wayscriber-session"),
            path_label: "/tmp/lecture.wayscriber-session".to_string(),
            canonical_path_label: None,
            created_label: "now".to_string(),
            last_opened_label: "Never".to_string(),
            last_saved_label: "Never".to_string(),
            artifacts: crate::models::session::SessionArtifactSummary {
                primary_exists: true,
                backup_exists: false,
                recovery_exists: false,
                clear_marker_exists: false,
                lock_exists: false,
                non_lock_size_bytes: 4096,
            },
        };

        assert_eq!(session_artifact_status_label(&item), "primary · 4.0 KiB");
    }
}
