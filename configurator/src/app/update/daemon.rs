use iced::Task;

use crate::messages::Message;
use crate::models::{DaemonAction, DaemonActionResult, DaemonRuntimeStatus};

use super::super::blocking_jobs::BlockingJobRequest;
use super::super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn handle_daemon_status_loaded(
        &mut self,
        preserve_feedback: bool,
        result: Result<DaemonRuntimeStatus, String>,
    ) -> Task<Message> {
        match result {
            Ok(status) => {
                self.apply_daemon_status(status);
                if should_update_feedback_after_status_load(
                    preserve_feedback,
                    self.daemon_busy,
                    self.daemon_feedback.as_deref(),
                ) {
                    self.daemon_feedback = Some("Background mode status loaded.".to_string());
                }
            }
            Err(err) => {
                if preserve_feedback && !self.daemon_busy {
                    let previous_feedback = self
                        .daemon_feedback
                        .as_deref()
                        .unwrap_or("Background setup action failed.");
                    self.daemon_feedback =
                        Some(format!("{previous_feedback}\nStatus refresh failed: {err}"));
                } else if !self.daemon_busy {
                    self.daemon_feedback =
                        Some(format!("Failed to load background setup status: {err}"));
                }
            }
        }
        Task::none()
    }

    pub(super) fn handle_daemon_shortcut_input_changed(&mut self, value: String) -> Task<Message> {
        self.daemon_shortcut_input = value;
        Task::none()
    }

    pub(super) fn handle_daemon_action_requested(&mut self, action: DaemonAction) -> Task<Message> {
        if self.daemon_busy {
            return Task::none();
        }
        let _cancellation = self.blocking_jobs.cancel_daemon_status();
        self.daemon_busy = true;
        self.daemon_feedback = Some(action_pending_message(action));
        let shortcut_input = self.daemon_shortcut_input.clone();
        self.submit_blocking_job(BlockingJobRequest::DaemonAction {
            action,
            shortcut_input,
        })
    }

    pub(super) fn handle_daemon_action_completed(
        &mut self,
        result: Result<DaemonActionResult, String>,
    ) -> Task<Message> {
        self.daemon_busy = false;
        match result {
            Ok(output) => {
                self.apply_daemon_status(output.status);
                self.daemon_feedback = Some(output.message);
                Task::none()
            }
            Err(err) => {
                self.daemon_feedback = Some(format!("Background setup action failed: {err}"));
                self.schedule_daemon_status_reload(true)
            }
        }
    }

    fn apply_daemon_status(&mut self, status: DaemonRuntimeStatus) {
        if let Some(configured_shortcut) = status.configured_shortcut.clone() {
            self.daemon_shortcut_input = configured_shortcut;
        } else if self.daemon_shortcut_input.trim().is_empty() {
            self.daemon_shortcut_input = status.desktop.default_shortcut_input().to_string();
        }
        self.daemon_status = Some(status);
    }

    fn schedule_daemon_status_reload(&mut self, preserve_feedback: bool) -> Task<Message> {
        self.submit_blocking_job(BlockingJobRequest::DaemonStatus { preserve_feedback })
    }
}

fn should_update_feedback_after_status_load(
    preserve_feedback: bool,
    daemon_busy: bool,
    current_feedback: Option<&str>,
) -> bool {
    if preserve_feedback || daemon_busy {
        return false;
    }
    let Some(feedback) = current_feedback else {
        return true;
    };
    let normalized = feedback.to_ascii_lowercase();
    normalized.contains("detecting background mode setup status")
        || normalized.contains("refreshing background setup status")
        || normalized.contains("detecting daemon setup status")
        || normalized.contains("refreshing daemon status")
        || normalized == "background mode status loaded."
        || normalized == "daemon status loaded."
}

fn action_pending_message(action: DaemonAction) -> String {
    match action {
        DaemonAction::RefreshStatus => "Refreshing background setup status...".to_string(),
        DaemonAction::InstallOrUpdateService => {
            "Installing/updating background service...".to_string()
        }
        DaemonAction::EnableAndStartService => {
            "Enabling and starting background mode...".to_string()
        }
        DaemonAction::RestartService => "Restarting background service...".to_string(),
        DaemonAction::StopAndDisableService => {
            "Stopping and disabling background mode...".to_string()
        }
        DaemonAction::ApplyShortcut => "Applying desktop shortcut setup...".to_string(),
        DaemonAction::ApplyLightControls => {
            "Applying light passthrough controls setup...".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        DesktopEnvironment, LightShortcutApplyCapability, ShortcutApplyCapability, ShortcutBackend,
    };

    fn test_status(desktop: DesktopEnvironment, shortcut: Option<String>) -> DaemonRuntimeStatus {
        DaemonRuntimeStatus {
            desktop,
            shortcut_backend: ShortcutBackend::PortalServiceDropIn,
            shortcut_apply_capability: ShortcutApplyCapability::PortalServiceDropIn,
            light_shortcut_apply_capability: LightShortcutApplyCapability::from_environment(
                desktop,
            ),
            systemctl_available: true,
            gsettings_available: false,
            service_installed: false,
            service_enabled: false,
            service_active: false,
            service_unit_path: None,
            configured_shortcut: shortcut,
            light_controls_configured: false,
            light_controls_config_path: None,
        }
    }

    #[test]
    fn daemon_status_loaded_sets_default_shortcut_when_missing() {
        let (mut app, _command) = ConfiguratorApp::new_app();
        app.daemon_shortcut_input.clear();
        let status = test_status(DesktopEnvironment::Kde, None);

        let _ = app.handle_daemon_status_loaded(false, Ok(status));

        assert_eq!(app.daemon_shortcut_input, "Ctrl+Shift+G");
        assert!(app.daemon_status.is_some());
    }

    #[test]
    fn daemon_action_completion_error_sets_feedback() {
        let (mut app, _command) = ConfiguratorApp::new_app();
        let _ = app.handle_daemon_action_completed(Err("boom".to_string()));
        assert!(
            app.daemon_feedback
                .as_deref()
                .unwrap_or_default()
                .contains("Background setup action failed")
        );
    }

    #[test]
    fn status_loaded_does_not_clear_daemon_busy() {
        let (mut app, _command) = ConfiguratorApp::new_app();
        app.daemon_busy = true;
        app.daemon_feedback = Some("Installing/updating background service...".to_string());
        let status = test_status(DesktopEnvironment::Kde, None);

        let _ = app.handle_daemon_status_loaded(false, Ok(status));

        assert!(app.daemon_busy);
        assert_eq!(
            app.daemon_feedback.as_deref(),
            Some("Installing/updating background service...")
        );
    }

    #[test]
    fn failed_action_feedback_is_preserved_after_status_reload() {
        let (mut app, _command) = ConfiguratorApp::new_app();
        let _ = app.handle_daemon_action_completed(Err("boom".to_string()));
        let status = test_status(DesktopEnvironment::Kde, None);

        let _ = app.handle_daemon_status_loaded(true, Ok(status));

        assert!(
            app.daemon_feedback
                .as_deref()
                .unwrap_or_default()
                .contains("Background setup action failed: boom")
        );
    }

    #[test]
    fn preserved_error_is_not_applied_while_new_action_is_busy() {
        let (mut app, _command) = ConfiguratorApp::new_app();
        let _ = app.handle_daemon_action_completed(Err("boom".to_string()));
        app.daemon_busy = true;
        app.daemon_feedback = Some("Restarting background service...".to_string());

        let _ = app
            .handle_daemon_status_loaded(true, Err("portal temporarily unavailable".to_string()));

        assert_eq!(
            app.daemon_feedback.as_deref(),
            Some("Restarting background service...")
        );
    }
}
