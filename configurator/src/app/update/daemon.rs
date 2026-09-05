use super::super::effects::Effect;
use super::super::state::ConfiguratorApp;
use crate::models::{DaemonAction, DaemonActionResult, DaemonRuntimeStatus};

impl ConfiguratorApp {
    pub(super) fn handle_daemon_status_loaded(
        &mut self,
        request_id: u64,
        result: Result<DaemonRuntimeStatus, String>,
    ) -> Vec<Effect> {
        self.daemon.handle_daemon_status_loaded(request_id, result)
    }
    pub(super) fn handle_daemon_shortcut_input_changed(&mut self, value: String) -> Vec<Effect> {
        self.daemon.handle_daemon_shortcut_input_changed(value)
    }
    pub(super) fn handle_daemon_action_requested(&mut self, action: DaemonAction) -> Vec<Effect> {
        self.daemon.handle_daemon_action_requested(action)
    }
    pub(super) fn handle_daemon_action_completed(
        &mut self,
        result: Result<DaemonActionResult, String>,
    ) -> Vec<Effect> {
        self.daemon.handle_daemon_action_completed(result)
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
        let (mut app, _effects) = ConfiguratorApp::new_app();
        app.daemon.shortcut_input.clear();
        let status = test_status(DesktopEnvironment::Kde, None);

        app.daemon.latest_status_request_id = 7;
        let _ = app.handle_daemon_status_loaded(7, Ok(status));

        assert_eq!(app.daemon.shortcut_input, "Ctrl+Shift+G");
        assert!(app.daemon.status.is_some());
    }

    #[test]
    fn daemon_action_completion_error_sets_feedback() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ = app.handle_daemon_action_completed(Err("boom".to_string()));
        assert!(
            app.daemon
                .feedback
                .as_ref()
                .map(crate::app::daemon_workflow::DaemonFeedback::text)
                .unwrap_or_default()
                .contains("Background setup action failed")
        );
        assert_eq!(
            app.daemon.preserve_feedback_status_request_id,
            Some(app.daemon.latest_status_request_id)
        );
    }

    #[test]
    fn status_loaded_does_not_clear_daemon_busy() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ = app.handle_daemon_action_requested(DaemonAction::RestartService);
        app.daemon.feedback = Some(crate::app::daemon_workflow::DaemonFeedback::Action(
            "Installing/updating background service...".to_string(),
        ));
        let status = test_status(DesktopEnvironment::Kde, None);

        app.daemon.latest_status_request_id = 9;
        let _ = app.handle_daemon_status_loaded(9, Ok(status));

        assert!(app.daemon.is_busy());
        assert_eq!(
            app.daemon
                .feedback
                .as_ref()
                .map(crate::app::daemon_workflow::DaemonFeedback::text),
            Some("Installing/updating background service...")
        );
    }

    #[test]
    fn failed_action_feedback_is_preserved_after_status_reload() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ = app.handle_daemon_action_completed(Err("boom".to_string()));
        let preserved_request_id = app.daemon.latest_status_request_id;
        let status = test_status(DesktopEnvironment::Kde, None);

        let _ = app.handle_daemon_status_loaded(preserved_request_id, Ok(status));

        assert!(
            app.daemon
                .feedback
                .as_ref()
                .map(crate::app::daemon_workflow::DaemonFeedback::text)
                .unwrap_or_default()
                .contains("Background setup action failed: boom")
        );
        assert!(app.daemon.preserve_feedback_status_request_id.is_none());
    }

    #[test]
    fn stale_status_callback_does_not_consume_preserve_flag() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ = app.handle_daemon_action_completed(Err("boom".to_string()));
        let preserved_request_id = app.daemon.latest_status_request_id;
        let stale_request_id = preserved_request_id.saturating_sub(1);
        let stale_status = test_status(DesktopEnvironment::Kde, None);
        let _ = app.handle_daemon_status_loaded(stale_request_id, Ok(stale_status));

        assert_eq!(
            app.daemon.preserve_feedback_status_request_id,
            Some(preserved_request_id)
        );
    }

    #[test]
    fn preserved_error_is_not_applied_while_new_action_is_busy() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let _ = app.handle_daemon_action_completed(Err("boom".to_string()));
        let preserved_request_id = app.daemon.latest_status_request_id;
        let _ = app.handle_daemon_action_requested(DaemonAction::RestartService);
        app.daemon.feedback = Some(crate::app::daemon_workflow::DaemonFeedback::Action(
            "Restarting background service...".to_string(),
        ));

        let _ = app.handle_daemon_status_loaded(
            preserved_request_id,
            Err("portal temporarily unavailable".to_string()),
        );

        assert_eq!(
            app.daemon
                .feedback
                .as_ref()
                .map(crate::app::daemon_workflow::DaemonFeedback::text),
            Some("Restarting background service...")
        );
    }

    #[test]
    fn old_status_callback_after_newer_action_success_is_ignored() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let old_status = test_status(
            DesktopEnvironment::Kde,
            Some("<Ctrl><Shift>old".to_string()),
        );
        let mut new_status = test_status(
            DesktopEnvironment::Kde,
            Some("<Ctrl><Shift>new".to_string()),
        );
        new_status.service_installed = true;
        new_status.service_enabled = true;
        new_status.service_active = true;
        new_status.service_unit_path = Some("/tmp/wayscriber.service".to_string());

        let _ = app.handle_daemon_action_completed(Err("old failure".to_string()));
        let old_request_id = app.daemon.latest_status_request_id;

        let effects = app.handle_daemon_action_requested(DaemonAction::RefreshStatus);
        assert!(matches!(
            effects.as_slice(),
            [Effect::PerformDaemonAction {
                action: DaemonAction::RefreshStatus,
                ..
            }]
        ));
        let _ = app.handle_daemon_action_completed(Ok(DaemonActionResult {
            status: new_status.clone(),
            message: "refresh complete".to_string(),
        }));

        let _ = app.handle_daemon_status_loaded(old_request_id, Ok(old_status));

        assert_eq!(app.daemon.shortcut_input.as_str(), "<Ctrl><Shift>new");
        assert_eq!(
            app.daemon
                .status
                .as_ref()
                .and_then(|status| status.configured_shortcut.as_deref()),
            Some("<Ctrl><Shift>new")
        );
        assert_eq!(
            app.daemon
                .feedback
                .as_ref()
                .map(crate::app::daemon_workflow::DaemonFeedback::text),
            Some("refresh complete")
        );
    }
}
