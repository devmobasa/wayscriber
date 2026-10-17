//! Background setup workflow; callback identity and feedback policy live together.
use super::effects::Effect;
use crate::models::{DaemonAction, DaemonActionResult, DaemonRuntimeStatus, DesktopEnvironment};
use wayscriber::i18n::{tr, trf};

/// What the background setup panel reports, kept as what happened rather than
/// as text: it renders in the current UI language, and its tone comes from the
/// outcome, never from reading the words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DaemonFeedback {
    /// Reported by the panel itself; the next status load replaces it.
    Status(StatusFeedback),
    /// The outcome of something the user asked for; status loads keep it.
    Action(ActionFeedback),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StatusFeedback {
    Detecting,
    Loaded,
    LoadFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActionFeedback {
    Pending(DaemonAction),
    /// The setup step's own report. It was written in the language active
    /// when the step ran, like any other event message.
    Completed(String),
    /// A report whose main work succeeded but whose follow-up step failed,
    /// as the step itself said.
    PartlyFailed(String),
    Failed(String),
    /// A status refresh failed after `previous`, which it keeps visible.
    RefreshFailed {
        previous: Option<Box<DaemonFeedback>>,
        error: String,
    },
}

impl DaemonFeedback {
    pub(crate) fn text(&self) -> String {
        match self {
            Self::Status(StatusFeedback::Detecting) => {
                tr("Detecting background mode setup status...").to_string()
            }
            Self::Status(StatusFeedback::Loaded) => {
                tr("Background mode status loaded.").to_string()
            }
            Self::Status(StatusFeedback::LoadFailed(error)) => {
                trf("Failed to load background setup status: {}", &[error])
            }
            Self::Action(ActionFeedback::Pending(action)) => {
                action_pending_message(*action).to_string()
            }
            Self::Action(
                ActionFeedback::Completed(message) | ActionFeedback::PartlyFailed(message),
            ) => message.clone(),
            Self::Action(ActionFeedback::Failed(error)) => {
                trf("Background setup action failed: {}", &[error])
            }
            Self::Action(ActionFeedback::RefreshFailed { previous, error }) => {
                let previous = previous
                    .as_deref()
                    .map(DaemonFeedback::text)
                    .unwrap_or_else(|| tr("Background setup action failed.").to_string());
                trf("{}\nStatus refresh failed: {}", &[&previous, error])
            }
        }
    }

    /// Whether this reports a failure, for the panel's error styling.
    pub(crate) fn is_failure(&self) -> bool {
        matches!(
            self,
            Self::Status(StatusFeedback::LoadFailed(_))
                | Self::Action(
                    ActionFeedback::PartlyFailed(_)
                        | ActionFeedback::Failed(_)
                        | ActionFeedback::RefreshFailed { .. }
                )
        )
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
            feedback: Some(DaemonFeedback::Status(StatusFeedback::Detecting)),
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
                    self.feedback = Some(DaemonFeedback::Status(StatusFeedback::Loaded));
                }
            }
            Err(error) => {
                if preserve_feedback && !self.is_busy() {
                    let previous = self.feedback.take().map(Box::new);
                    self.feedback = Some(DaemonFeedback::Action(ActionFeedback::RefreshFailed {
                        previous,
                        error,
                    }));
                } else if !self.is_busy() {
                    self.feedback = Some(DaemonFeedback::Status(StatusFeedback::LoadFailed(error)));
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
        self.feedback = Some(DaemonFeedback::Action(ActionFeedback::Pending(action)));
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
                let report = if output.partial_failure {
                    ActionFeedback::PartlyFailed(output.message)
                } else {
                    ActionFeedback::Completed(output.message)
                };
                self.feedback = Some(DaemonFeedback::Action(report));
                Vec::new()
            }
            Err(error) => {
                self.feedback = Some(DaemonFeedback::Action(ActionFeedback::Failed(error)));
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

fn action_pending_message(action: DaemonAction) -> &'static str {
    match action {
        DaemonAction::RefreshStatus => tr("Refreshing background setup status..."),
        DaemonAction::InstallOrUpdateService => tr("Installing/updating background service..."),
        DaemonAction::EnableAndStartService => tr("Enabling and starting background mode..."),
        DaemonAction::RestartService => tr("Restarting background service..."),
        DaemonAction::StopAndDisableService => tr("Stopping and disabling background mode..."),
        DaemonAction::ApplyShortcut => tr("Applying desktop shortcut setup..."),
        DaemonAction::ApplyLightControls => tr("Applying light passthrough controls setup..."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wayscriber::domain::Language;

    #[test]
    fn refresh_policy_depends_on_feedback_kind_not_its_wording() {
        let status = DaemonFeedback::Status(StatusFeedback::Loaded);
        assert!(should_update_feedback_after_status_load(
            false,
            false,
            Some(&status)
        ));
        let action_using_status_words = DaemonFeedback::Action(ActionFeedback::Completed(
            "Background mode status loaded.".into(),
        ));
        assert!(!should_update_feedback_after_status_load(
            false,
            false,
            Some(&action_using_status_words)
        ));
        assert!(!should_update_feedback_after_status_load(
            true,
            false,
            Some(&status)
        ));
        assert!(!should_update_feedback_after_status_load(
            false,
            true,
            Some(&status)
        ));
    }

    #[test]
    fn failures_are_typed_whatever_the_words_say() {
        let failures = [
            DaemonFeedback::Status(StatusFeedback::LoadFailed("portal".into())),
            DaemonFeedback::Action(ActionFeedback::Failed("boom".into())),
            DaemonFeedback::Action(ActionFeedback::PartlyFailed(
                "Wrote Hyprland light controls; hyprctl reload failed: portal".into(),
            )),
            DaemonFeedback::Action(ActionFeedback::RefreshFailed {
                previous: None,
                error: "portal".into(),
            }),
        ];
        let successes = [
            DaemonFeedback::Status(StatusFeedback::Detecting),
            DaemonFeedback::Status(StatusFeedback::Loaded),
            DaemonFeedback::Action(ActionFeedback::Pending(DaemonAction::RestartService)),
            // A report that happens to contain the word is still a success.
            DaemonFeedback::Action(ActionFeedback::Completed("error handler installed".into())),
        ];

        assert!(failures.iter().all(DaemonFeedback::is_failure));
        assert!(!successes.iter().any(DaemonFeedback::is_failure));
    }

    #[test]
    fn feedback_renders_in_the_language_active_when_it_is_drawn() {
        crate::test_env::in_language_child(
            concat!(
                module_path!(),
                "::feedback_renders_in_the_language_active_when_it_is_drawn"
            ),
            || {
                let failed = DaemonFeedback::Action(ActionFeedback::RefreshFailed {
                    previous: Some(Box::new(DaemonFeedback::Action(ActionFeedback::Failed(
                        "boom".into(),
                    )))),
                    error: "portal".into(),
                });
                let detecting = DaemonFeedback::Status(StatusFeedback::Detecting);
                let english = (failed.text(), detecting.text());

                wayscriber::i18n::set_language(Language::SimplifiedChinese);

                assert_eq!(
                    english.0,
                    "Background setup action failed: boom\nStatus refresh failed: portal"
                );
                assert_eq!(english.1, "Detecting background mode setup status...");
                assert_eq!(
                    failed.text(),
                    "后台设置操作失败：boom\n状态刷新失败：portal"
                );
                assert_eq!(detecting.text(), "正在检测后台模式设置状态...");
                // The failure keeps its tone in Chinese, where neither "failed" nor
                // "error" appears.
                assert!(failed.is_failure());
            },
        );
    }
}
