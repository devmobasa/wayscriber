use super::*;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::MutexGuard;

use crate::models::{
    DesktopEnvironment, LightShortcutApplyCapability, ShortcutApplyCapability, ShortcutBackend,
};
use wayscriber::env_vars::XDG_RUNTIME_DIR_ENV;

struct RuntimeEnvGuard {
    previous: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl RuntimeEnvGuard {
    fn set_xdg_runtime_dir(path: &Path) -> Self {
        let guard = crate::test_env::lock();
        let previous = std::env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            std::env::set_var(XDG_RUNTIME_DIR_ENV, path);
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
            Some(value) => unsafe { std::env::set_var(XDG_RUNTIME_DIR_ENV, value) },
            None => unsafe { std::env::remove_var(XDG_RUNTIME_DIR_ENV) },
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
fn session_clear_cached_status_blocker_blocks_running_daemon() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
    let status = daemon_status(true);

    let blocker =
        session_clear_cached_status_blocker(Some(&status)).expect("daemon should block clear");

    assert!(blocker.contains("background service"));
}

#[test]
fn session_clear_cached_status_blocker_allows_inactive_daemon() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
    let status = daemon_status(false);

    let blocker = session_clear_cached_status_blocker(Some(&status));

    assert!(blocker.is_none(), "{blocker:?}");
}

#[test]
fn session_duplicate_cached_status_blocker_uses_duplicate_status_message() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
    let blocker = session_duplicate_cached_status_blocker(None)
        .expect("unknown status should block duplicate");

    assert!(blocker.contains("Duplicate Session"));
}

#[test]
fn session_move_cached_status_blocker_uses_move_status_message() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
    let blocker =
        session_move_cached_status_blocker(None).expect("unknown status should block move");

    assert!(blocker.contains("Move Session"));
}

#[test]
fn session_clear_cached_status_blocker_blocks_unknown_daemon_status() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
    let blocker =
        session_clear_cached_status_blocker(None).expect("unknown status should block clear");

    assert!(blocker.contains("status finishes loading"));
}

#[test]
fn session_clear_tool_state_cached_status_blocker_uses_tool_state_status_message() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
    let blocker = session_clear_tool_state_cached_status_blocker(None)
        .expect("unknown status should block tool reset");

    assert!(blocker.contains("Clear saved tool state"));
    assert!(blocker.contains("status finishes loading"));
}

#[test]
fn session_clear_transaction_guard_blocks_manual_daemon_lock() {
    let temp = crate::test_temp::tempdir().unwrap();
    let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
    let _daemon_lock = acquire_runtime_lock_for_clear(RuntimeLockKind::Daemon).unwrap();
    let error = acquire_runtime_lock_for_inactive_operation(
        RuntimeLockKind::Daemon,
        CatalogOperation::Clear,
    )
    .expect_err("held daemon lock should block the transaction");

    assert!(error.contains("manually started daemon"));
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
    let blocker = session_move_cached_status_blocker(Some(&status));

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
