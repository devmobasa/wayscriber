//! Background setup workflow; callback identity and feedback policy live together.
use super::effects::Effect;
use crate::models::{DaemonAction, DaemonActionResult, DaemonRuntimeStatus, DesktopEnvironment};

#[derive(Debug)]
pub(crate) enum DaemonFeedback {
    Status(String),
    Action(String),
}
impl DaemonFeedback {
    pub(crate) fn text(&self) -> &str {
        match self {
            Self::Status(text) | Self::Action(text) => text,
        }
    }
}

#[derive(Debug)]
pub(crate) struct DaemonWorkflow {
    pub(crate) status: Option<DaemonRuntimeStatus>,
    pub(crate) shortcut_input: String,
    pub(crate) feedback: Option<DaemonFeedback>,
    active_action: Option<DaemonAction>,
    pub(crate) next_status_request_id: u64,
    pub(crate) latest_status_request_id: u64,
    pub(crate) preserve_feedback_status_request_id: Option<u64>,
}
impl DaemonWorkflow {
    pub(crate) fn is_busy(&self) -> bool {
        self.active_action.is_some()
    }
    pub(crate) fn new(desktop: DesktopEnvironment) -> Self {
        Self {
            status: None,
            shortcut_input: desktop.default_shortcut_input().to_string(),
            feedback: Some(DaemonFeedback::Status(
                "Detecting background mode setup status...".to_string(),
            )),
            active_action: None,
            next_status_request_id: 2,
            latest_status_request_id: 1,
            preserve_feedback_status_request_id: None,
        }
    }
}

impl DaemonWorkflow {
    pub(crate) fn handle_daemon_status_loaded(
        &mut self,
        request_id: u64,
        result: Result<DaemonRuntimeStatus, String>,
    ) -> Vec<Effect> {
        if request_id != self.latest_status_request_id {
            return Vec::new();
        }
        let preserve_feedback = self.preserve_feedback_status_request_id == Some(request_id);
        if preserve_feedback {
            self.preserve_feedback_status_request_id = None;
        }
        match result {
            Ok(status) => {
                self.apply_daemon_status(status);
                if should_update_feedback_after_status_load(
                    preserve_feedback,
                    self.is_busy(),
                    self.feedback.as_ref(),
                ) {
                    self.feedback = Some(DaemonFeedback::Status(
                        "Background mode status loaded.".to_string(),
                    ));
                }
            }
            Err(err) => {
                if preserve_feedback && !self.is_busy() {
                    let previous_feedback = self
                        .feedback
                        .as_ref()
                        .map(DaemonFeedback::text)
                        .unwrap_or("Background setup action failed.");
                    self.feedback = Some(DaemonFeedback::Action(format!(
                        "{previous_feedback}\nStatus refresh failed: {err}"
                    )));
                } else if !self.is_busy() {
                    self.feedback = Some(DaemonFeedback::Status(format!(
                        "Failed to load background setup status: {err}"
                    )));
                }
            }
        }
        Vec::new()
    }

    pub(crate) fn handle_daemon_shortcut_input_changed(&mut self, value: String) -> Vec<Effect> {
        self.shortcut_input = value;
        Vec::new()
    }

    pub(crate) fn handle_daemon_action_requested(&mut self, action: DaemonAction) -> Vec<Effect> {
        if self.is_busy() {
            return Vec::new();
        }
        self.invalidate_pending_daemon_status_requests();
        self.active_action = Some(action);
        self.feedback = Some(DaemonFeedback::Action(action_pending_message(action)));
        let shortcut_input = self.shortcut_input.clone();
        vec![Effect::PerformDaemonAction {
            action,
            shortcut_input,
        }]
    }

    pub(crate) fn handle_daemon_action_completed(
        &mut self,
        result: Result<DaemonActionResult, String>,
    ) -> Vec<Effect> {
        self.active_action = None;
        match result {
            Ok(output) => {
                self.apply_daemon_status(output.status);
                self.feedback = Some(DaemonFeedback::Action(output.message));
                Vec::new()
            }
            Err(err) => {
                self.feedback = Some(DaemonFeedback::Action(format!(
                    "Background setup action failed: {err}"
                )));
                self.schedule_daemon_status_reload(true)
            }
        }
    }

    fn apply_daemon_status(&mut self, status: DaemonRuntimeStatus) {
        if let Some(configured_shortcut) = status.configured_shortcut.clone() {
            self.shortcut_input = configured_shortcut;
        } else if self.shortcut_input.trim().is_empty() {
            self.shortcut_input = status.desktop.default_shortcut_input().to_string();
        }
        self.status = Some(status);
    }

    fn schedule_daemon_status_reload(&mut self, preserve_feedback: bool) -> Vec<Effect> {
        let request_id = self.next_status_request_id;
        self.next_status_request_id = self.next_status_request_id.saturating_add(1);
        self.latest_status_request_id = request_id;
        if preserve_feedback {
            self.preserve_feedback_status_request_id = Some(request_id);
        }
        vec![Effect::LoadDaemonStatus { request_id }]
    }

    fn invalidate_pending_daemon_status_requests(&mut self) {
        let invalidation_id = self.next_status_request_id;
        self.next_status_request_id = self.next_status_request_id.saturating_add(1);
        self.latest_status_request_id = invalidation_id;
        self.preserve_feedback_status_request_id = None;
    }
}

fn should_update_feedback_after_status_load(
    preserve_feedback: bool,
    busy: bool,
    feedback: Option<&DaemonFeedback>,
) -> bool {
    !preserve_feedback && !busy && matches!(feedback, None | Some(DaemonFeedback::Status(_)))
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

    #[test]
    fn refresh_policy_depends_on_feedback_kind_not_its_wording() {
        let translated_status = DaemonFeedback::Status("Status geladen".into());
        assert!(should_update_feedback_after_status_load(
            false,
            false,
            Some(&translated_status)
        ));
        let action_using_status_words =
            DaemonFeedback::Action("Background mode status loaded.".into());
        assert!(!should_update_feedback_after_status_load(
            false,
            false,
            Some(&action_using_status_words)
        ));
        assert!(!should_update_feedback_after_status_load(
            true,
            false,
            Some(&translated_status)
        ));
        assert!(!should_update_feedback_after_status_load(
            false,
            true,
            Some(&translated_status)
        ));
    }
}
