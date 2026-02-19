mod command;
mod service;
mod shortcut;

use crate::models::{
    DaemonAction, DaemonActionResult, DaemonRuntimeStatus, DesktopEnvironment, ShortcutBackend,
};

use command::command_available;
use service::{
    SERVICE_NAME, detect_service_unit_path, install_or_update_user_service, query_service_active,
    query_service_enabled, require_systemctl_available, run_systemctl_user,
};
use shortcut::{apply_shortcut, read_configured_shortcut};

pub(super) async fn load_daemon_runtime_status() -> Result<DaemonRuntimeStatus, String> {
    load_daemon_runtime_status_sync()
}

pub(super) async fn perform_daemon_action(
    action: DaemonAction,
    shortcut_input: String,
) -> Result<DaemonActionResult, String> {
    let message = perform_daemon_action_sync(action, shortcut_input.trim())?;
    let status = load_daemon_runtime_status_sync()?;
    Ok(DaemonActionResult { status, message })
}

fn perform_daemon_action_sync(
    action: DaemonAction,
    shortcut_input: &str,
) -> Result<String, String> {
    match action {
        DaemonAction::RefreshStatus => Ok("Daemon status refreshed.".to_string()),
        DaemonAction::InstallOrUpdateService => {
            let service_path = install_or_update_user_service()?;
            Ok(format!(
                "Installed/updated user service at {}",
                service_path.display()
            ))
        }
        DaemonAction::EnableAndStartService => {
            require_systemctl_available()?;
            run_systemctl_user(&["daemon-reload"])?;
            run_systemctl_user(&["enable", "--now", SERVICE_NAME])?;
            Ok("Enabled and started wayscriber.service.".to_string())
        }
        DaemonAction::RestartService => {
            require_systemctl_available()?;
            run_systemctl_user(&["restart", SERVICE_NAME])?;
            Ok("Restarted wayscriber.service.".to_string())
        }
        DaemonAction::StopAndDisableService => {
            require_systemctl_available()?;
            run_systemctl_user(&["disable", "--now", SERVICE_NAME])?;
            Ok("Stopped and disabled wayscriber.service.".to_string())
        }
        DaemonAction::ApplyShortcut => apply_shortcut(shortcut_input),
    }
}

fn load_daemon_runtime_status_sync() -> Result<DaemonRuntimeStatus, String> {
    let desktop = DesktopEnvironment::detect_current();
    let systemctl_available = command_available("systemctl");
    let gsettings_available = command_available("gsettings");
    let shortcut_backend =
        ShortcutBackend::from_environment(desktop, gsettings_available, systemctl_available);
    let service_unit_path = detect_service_unit_path(systemctl_available);
    let service_installed = service_unit_path.is_some();
    let service_enabled = if systemctl_available {
        query_service_enabled()
    } else {
        false
    };
    let service_active = if systemctl_available {
        query_service_active()
    } else {
        false
    };
    let configured_shortcut = read_configured_shortcut(shortcut_backend);

    Ok(DaemonRuntimeStatus {
        desktop,
        shortcut_backend,
        systemctl_available,
        gsettings_available,
        service_installed,
        service_enabled,
        service_active,
        service_unit_path: service_unit_path.map(|path| path.display().to_string()),
        configured_shortcut,
    })
}
