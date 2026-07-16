mod command;
mod hyprland;
mod service;
mod shortcut;

use crate::models::{
    DaemonAction, DaemonActionResult, DaemonRuntimeStatus, DesktopEnvironment,
    LightShortcutApplyCapability, ShortcutBackend,
};

use command::command_available;
use hyprland::{
    install_light_controls as install_hyprland_light_controls,
    read_light_controls_status as read_hyprland_light_controls_status,
};
use service::{
    SERVICE_NAME, detect_managed_daemon_portal_runtime_supported, detect_service_unit_path,
    install_or_update_user_service, query_service_active, query_service_enabled,
    remove_portal_shortcut_dropin_if_gnome, require_systemctl_available, run_systemctl_user,
};
use shortcut::{apply_shortcut, read_configured_shortcut, read_portal_shortcut_dropin_state};

use super::blocking_jobs::{BlockingJobKind, run_blocking};

pub(super) async fn load_daemon_runtime_status() -> Result<DaemonRuntimeStatus, String> {
    run_blocking(
        BlockingJobKind::DaemonStatus,
        load_daemon_runtime_status_sync,
    )
    .await
}

pub(super) async fn perform_daemon_action(
    action: DaemonAction,
    shortcut_input: String,
) -> Result<DaemonActionResult, String> {
    run_blocking(BlockingJobKind::DaemonAction, move || {
        let message = perform_daemon_action_sync(action, shortcut_input.trim())?;
        let status = load_daemon_runtime_status_sync()?;
        Ok(DaemonActionResult { status, message })
    })
    .await
}

fn perform_daemon_action_sync(
    action: DaemonAction,
    shortcut_input: &str,
) -> Result<String, String> {
    match action {
        DaemonAction::RefreshStatus => Ok("Daemon status refreshed.".to_string()),
        DaemonAction::InstallOrUpdateService => {
            let desktop = DesktopEnvironment::detect_current();
            let service_path = install_or_update_user_service()?;
            let removed_dropin = remove_portal_shortcut_dropin_if_gnome(desktop)?;
            if command_available("systemctl") && removed_dropin {
                run_systemctl_user(&["daemon-reload"])?;
                if query_service_active() {
                    run_systemctl_user(&["restart", SERVICE_NAME])?;
                }
            }
            Ok(format!(
                "Installed/updated user service at {}{}",
                service_path.display(),
                if removed_dropin {
                    "; removed stale GNOME portal shortcut drop-in"
                } else {
                    ""
                }
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
        DaemonAction::ApplyLightControls => {
            let result = install_hyprland_light_controls()?;
            Ok(result.summary())
        }
    }
}

pub(super) fn load_daemon_runtime_status_sync() -> Result<DaemonRuntimeStatus, String> {
    let desktop = DesktopEnvironment::detect_current();
    let systemctl_available = command_available("systemctl");
    let gsettings_available = command_available("gsettings");
    let portal_runtime_supported = detect_managed_daemon_portal_runtime_supported();
    let shortcut_backend = ShortcutBackend::from_runtime_inputs(
        desktop,
        read_portal_shortcut_dropin_state(),
        portal_runtime_supported,
    );
    let shortcut_apply_capability = crate::models::ShortcutApplyCapability::from_environment(
        desktop,
        gsettings_available,
        systemctl_available,
        portal_runtime_supported,
    );
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
    let light_shortcut_apply_capability = LightShortcutApplyCapability::from_environment(desktop);
    let light_controls =
        if light_shortcut_apply_capability == LightShortcutApplyCapability::HyprlandNative {
            Some(read_hyprland_light_controls_status())
        } else {
            None
        };

    Ok(DaemonRuntimeStatus {
        desktop,
        shortcut_backend,
        shortcut_apply_capability,
        light_shortcut_apply_capability,
        systemctl_available,
        gsettings_available,
        service_installed,
        service_enabled,
        service_active,
        service_unit_path: service_unit_path.map(|path| path.display().to_string()),
        configured_shortcut,
        light_controls_configured: light_controls
            .as_ref()
            .is_some_and(|status| status.configured()),
        light_controls_config_path: light_controls
            .as_ref()
            .and_then(|status| status.include_path.as_ref())
            .map(|path| path.display().to_string()),
    })
}
