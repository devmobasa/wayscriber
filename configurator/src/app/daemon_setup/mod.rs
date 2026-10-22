mod command;
mod hyprland;
mod service;
mod shortcut;

use wayscriber::i18n::{tr, trf};

use crate::models::{
    DaemonAction, DaemonActionResult, DaemonRuntimeStatus, DesktopEnvironment,
    LightShortcutApplyCapability, ShortcutApplyCapability, ShortcutBackend,
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
        let report = perform_daemon_action_sync(action, shortcut_input.trim())?;
        let status = load_daemon_runtime_status_sync()?;
        Ok(DaemonActionResult {
            status,
            message: report.message,
            partial_failure: report.partial_failure,
        })
    })
    .await
}

/// What a finished setup action reports. Whether a step failed travels next
/// to the message, never inside it: the panel styles the outcome from this
/// flag, and the message is translated.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionReport {
    message: String,
    /// The action did its main work, but a later step failed; the message
    /// says which.
    partial_failure: bool,
}

impl ActionReport {
    fn complete(message: String) -> Self {
        Self {
            message,
            partial_failure: false,
        }
    }

    fn light_controls(result: &hyprland::HyprlandLightControlsInstallResult) -> Self {
        Self {
            message: result.summary(),
            partial_failure: result.reload_failed(),
        }
    }
}

fn perform_daemon_action_sync(
    action: DaemonAction,
    shortcut_input: &str,
) -> Result<ActionReport, String> {
    let message = match action {
        DaemonAction::RefreshStatus => tr("Daemon status refreshed.").to_string(),
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
            trf(
                "Installed/updated user service at {}{}",
                &[
                    &service_path.display(),
                    &if removed_dropin {
                        tr("; removed stale GNOME portal shortcut drop-in")
                    } else {
                        ""
                    },
                ],
            )
        }
        DaemonAction::EnableAndStartService => {
            require_systemctl_available()?;
            run_systemctl_user(&["daemon-reload"])?;
            run_systemctl_user(&["enable", "--now", SERVICE_NAME])?;
            tr("Enabled and started wayscriber.service.").to_string()
        }
        DaemonAction::RestartService => {
            require_systemctl_available()?;
            run_systemctl_user(&["restart", SERVICE_NAME])?;
            tr("Restarted wayscriber.service.").to_string()
        }
        DaemonAction::StopAndDisableService => {
            require_systemctl_available()?;
            run_systemctl_user(&["disable", "--now", SERVICE_NAME])?;
            tr("Stopped and disabled wayscriber.service.").to_string()
        }
        DaemonAction::ApplyShortcut => apply_shortcut(shortcut_input)?,
        DaemonAction::ApplyLightControls => {
            let result = install_hyprland_light_controls()?;
            return Ok(ActionReport::light_controls(&result));
        }
    };

    Ok(ActionReport::complete(message))
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
    let shortcut_apply_capability = ShortcutApplyCapability::from_environment(
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
