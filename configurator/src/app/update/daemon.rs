use iced::Command;

use crate::messages::Message;
use crate::models::{DaemonAction, DaemonActionResult, DaemonRuntimeStatus};

use super::super::daemon_setup::{load_daemon_runtime_status, perform_daemon_action};
use super::super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn handle_daemon_status_loaded(
        &mut self,
        request_id: u64,
        result: Result<DaemonRuntimeStatus, String>,
    ) -> Command<Message> {
        if request_id != self.daemon_latest_status_request_id {
            return Command::none();
        }
        let preserve_feedback = self.daemon_preserve_feedback_status_request_id == Some(request_id);
        if preserve_feedback {
            self.daemon_preserve_feedback_status_request_id = None;
        }
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
        Command::none()
    }

    pub(super) fn handle_daemon_shortcut_input_changed(
        &mut self,
        value: String,
    ) -> Command<Message> {
        self.daemon_shortcut_input = value;
        Command::none()
    }

    pub(super) fn handle_daemon_action_requested(
        &mut self,
        action: DaemonAction,
    ) -> Command<Message> {
        if self.daemon_busy {
            return Command::none();
        }
        self.invalidate_pending_daemon_status_requests();
        self.daemon_busy = true;
        self.daemon_feedback = Some(action_pending_message(action));
        let shortcut_input = self.daemon_shortcut_input.clone();
        Command::perform(
            perform_daemon_action(action, shortcut_input),
            Message::DaemonActionCompleted,
        )
    }

    pub(super) fn handle_daemon_action_completed(
        &mut self,
        result: Result<DaemonActionResult, String>,
    ) -> Command<Message> {
        self.daemon_busy = false;
        match result {
            Ok(output) => {
                self.apply_daemon_status(output.status);
                self.daemon_feedback = Some(output.message);
                Command::none()
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

    fn schedule_daemon_status_reload(&mut self, preserve_feedback: bool) -> Command<Message> {
        let request_id = self.daemon_next_status_request_id;
        self.daemon_next_status_request_id = self.daemon_next_status_request_id.saturating_add(1);
        self.daemon_latest_status_request_id = request_id;
        if preserve_feedback {
            self.daemon_preserve_feedback_status_request_id = Some(request_id);
        }
        Command::perform(load_daemon_runtime_status(), move |result| {
            Message::DaemonStatusLoaded(request_id, result)
        })
    }

    fn invalidate_pending_daemon_status_requests(&mut self) {
        let invalidation_id = self.daemon_next_status_request_id;
        self.daemon_next_status_request_id = self.daemon_next_status_request_id.saturating_add(1);
        self.daemon_latest_status_request_id = invalidation_id;
        self.daemon_preserve_feedback_status_request_id = None;
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DesktopEnvironment, ShortcutBackend};

    #[test]
    fn daemon_status_loaded_sets_default_shortcut_when_missing() {
        let (mut app, _command) = ConfiguratorApp::new_app();
        app.daemon_shortcut_input.clear();
        let status = DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Kde,
            shortcut_backend: ShortcutBackend::PortalServiceDropIn,
            systemctl_available: true,
            gsettings_available: false,
            service_installed: false,
            service_enabled: false,
            service_active: false,
            service_unit_path: None,
            configured_shortcut: None,
        };

        app.daemon_latest_status_request_id = 7;
        let _ = app.handle_daemon_status_loaded(7, Ok(status));

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
        assert_eq!(
            app.daemon_preserve_feedback_status_request_id,
            Some(app.daemon_latest_status_request_id)
        );
    }

    #[test]
    fn status_loaded_does_not_clear_daemon_busy() {
        let (mut app, _command) = ConfiguratorApp::new_app();
        app.daemon_busy = true;
        app.daemon_feedback = Some("Installing/updating background service...".to_string());
        let status = DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Kde,
            shortcut_backend: ShortcutBackend::PortalServiceDropIn,
            systemctl_available: true,
            gsettings_available: false,
            service_installed: false,
            service_enabled: false,
            service_active: false,
            service_unit_path: None,
            configured_shortcut: None,
        };

        app.daemon_latest_status_request_id = 9;
        let _ = app.handle_daemon_status_loaded(9, Ok(status));

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
        let preserved_request_id = app.daemon_latest_status_request_id;
        let status = DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Kde,
            shortcut_backend: ShortcutBackend::PortalServiceDropIn,
            systemctl_available: true,
            gsettings_available: false,
            service_installed: false,
            service_enabled: false,
            service_active: false,
            service_unit_path: None,
            configured_shortcut: None,
        };

        let _ = app.handle_daemon_status_loaded(preserved_request_id, Ok(status));

        assert!(
            app.daemon_feedback
                .as_deref()
                .unwrap_or_default()
                .contains("Background setup action failed: boom")
        );
        assert!(app.daemon_preserve_feedback_status_request_id.is_none());
    }

    #[test]
    fn stale_status_callback_does_not_consume_preserve_flag() {
        let (mut app, _command) = ConfiguratorApp::new_app();
        let _ = app.handle_daemon_action_completed(Err("boom".to_string()));
        let preserved_request_id = app.daemon_latest_status_request_id;
        let stale_request_id = preserved_request_id.saturating_sub(1);
        let stale_status = DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Kde,
            shortcut_backend: ShortcutBackend::PortalServiceDropIn,
            systemctl_available: true,
            gsettings_available: false,
            service_installed: false,
            service_enabled: false,
            service_active: false,
            service_unit_path: None,
            configured_shortcut: None,
        };
        let _ = app.handle_daemon_status_loaded(stale_request_id, Ok(stale_status));

        assert_eq!(
            app.daemon_preserve_feedback_status_request_id,
            Some(preserved_request_id)
        );
    }

    #[test]
    fn preserved_error_is_not_applied_while_new_action_is_busy() {
        let (mut app, _command) = ConfiguratorApp::new_app();
        let _ = app.handle_daemon_action_completed(Err("boom".to_string()));
        let preserved_request_id = app.daemon_latest_status_request_id;
        app.daemon_busy = true;
        app.daemon_feedback = Some("Restarting background service...".to_string());

        let _ = app.handle_daemon_status_loaded(
            preserved_request_id,
            Err("portal temporarily unavailable".to_string()),
        );

        assert_eq!(
            app.daemon_feedback.as_deref(),
            Some("Restarting background service...")
        );
    }

    #[test]
    fn old_status_callback_after_newer_action_success_is_ignored() {
        let (mut app, _command) = ConfiguratorApp::new_app();
        let old_status = DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Kde,
            shortcut_backend: ShortcutBackend::PortalServiceDropIn,
            systemctl_available: true,
            gsettings_available: false,
            service_installed: false,
            service_enabled: false,
            service_active: false,
            service_unit_path: None,
            configured_shortcut: Some("<Ctrl><Shift>old".to_string()),
        };
        let new_status = DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Kde,
            shortcut_backend: ShortcutBackend::PortalServiceDropIn,
            systemctl_available: true,
            gsettings_available: false,
            service_installed: true,
            service_enabled: true,
            service_active: true,
            service_unit_path: Some("/tmp/wayscriber.service".to_string()),
            configured_shortcut: Some("<Ctrl><Shift>new".to_string()),
        };

        let _ = app.handle_daemon_action_completed(Err("old failure".to_string()));
        let old_request_id = app.daemon_latest_status_request_id;

        let _ = app.handle_daemon_action_requested(DaemonAction::RefreshStatus);
        let _ = app.handle_daemon_action_completed(Ok(DaemonActionResult {
            status: new_status.clone(),
            message: "refresh complete".to_string(),
        }));

        let _ = app.handle_daemon_status_loaded(old_request_id, Ok(old_status));

        assert_eq!(app.daemon_shortcut_input.as_str(), "<Ctrl><Shift>new");
        assert_eq!(
            app.daemon_status
                .as_ref()
                .and_then(|status| status.configured_shortcut.as_deref()),
            Some("<Ctrl><Shift>new")
        );
        assert_eq!(app.daemon_feedback.as_deref(), Some("refresh complete"));
    }
}
