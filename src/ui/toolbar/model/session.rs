use std::path::PathBuf;

use crate::input::ToolbarDrawerTab;

use super::super::{SessionRecentSnapshot, ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};
use super::catalog;

const MAX_RECENT_SESSIONS: usize = 3;
pub(crate) const SESSION_BUTTON_COLUMNS: usize = 3;

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSessionModel {
    pub(crate) active_name: String,
    pub(crate) active_path_label: String,
    pub(crate) buttons: Vec<ToolbarSessionButton>,
    pub(crate) recents: Vec<ToolbarSessionRecent>,
    pub(crate) overwrite_confirmation: Option<ToolbarSessionOverwriteConfirmation>,
}

impl ToolbarSessionModel {
    pub(crate) fn from_snapshot(snapshot: &ToolbarSnapshot) -> Option<Self> {
        if snapshot.side_section_hidden(ToolbarSideSection::Session)
            || !snapshot.drawer_open
            || snapshot.drawer_tab != ToolbarDrawerTab::Session
        {
            return None;
        }

        let active_name = snapshot
            .active_session_name
            .clone()
            .unwrap_or_else(|| "Default session".to_string());
        let active_path_label = snapshot
            .active_session_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "No persisted session target".to_string());
        let target_active = snapshot.active_session_path.is_some();
        let buttons: Vec<_> = vec![
            ToolbarSessionButton::new(ToolbarEvent::OpenSession, "Open", target_active),
            ToolbarSessionButton::new(ToolbarEvent::SaveSessionAs, "Save As", target_active),
            ToolbarSessionButton::new(ToolbarEvent::SessionInfo, "Info", target_active),
            ToolbarSessionButton::new(ToolbarEvent::ClearSession, "Clear", target_active),
            ToolbarSessionButton::new(ToolbarEvent::OpenConfigurator, "Manager", true),
        ]
        .into_iter()
        .filter(|button| session_button_visible(snapshot, &button.event))
        .collect();
        let recents = if target_active {
            snapshot
                .recent_sessions
                .iter()
                .take(MAX_RECENT_SESSIONS)
                .map(ToolbarSessionRecent::from_snapshot)
                .collect()
        } else {
            Vec::new()
        };
        let overwrite_confirmation = snapshot
            .pending_save_as_overwrite_path
            .as_ref()
            .map(|path| ToolbarSessionOverwriteConfirmation {
                label: session_path_label(path),
                path: path.clone(),
            });

        (!buttons.is_empty() || !recents.is_empty()).then_some(Self {
            active_name,
            active_path_label,
            buttons,
            recents,
            overwrite_confirmation,
        })
    }

    pub(crate) fn has_recent_sessions(&self) -> bool {
        !self.recents.is_empty()
    }

    pub(crate) fn button_columns(&self) -> usize {
        self.buttons.len().clamp(1, SESSION_BUTTON_COLUMNS)
    }

    pub(crate) fn button_rows(&self) -> usize {
        self.buttons.len().div_ceil(self.button_columns())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSessionOverwriteConfirmation {
    pub(crate) label: String,
    pub(crate) path: PathBuf,
}

impl ToolbarSessionOverwriteConfirmation {
    pub(crate) fn confirm_event(&self) -> ToolbarEvent {
        ToolbarEvent::SaveSessionAsConfirm(self.path.clone())
    }

    pub(crate) fn cancel_event(&self) -> ToolbarEvent {
        ToolbarEvent::SaveSessionAsCancel
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSessionButton {
    pub(crate) event: ToolbarEvent,
    pub(crate) label: &'static str,
    pub(crate) enabled: bool,
}

impl ToolbarSessionButton {
    fn new(event: ToolbarEvent, label: &'static str, enabled: bool) -> Self {
        Self {
            event,
            label,
            enabled,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSessionRecent {
    pub(crate) label: String,
    pub(crate) path: PathBuf,
}

impl ToolbarSessionRecent {
    fn from_snapshot(snapshot: &SessionRecentSnapshot) -> Self {
        Self {
            label: snapshot.display_name.clone(),
            path: snapshot.path.clone(),
        }
    }

    pub(crate) fn event(&self) -> ToolbarEvent {
        ToolbarEvent::OpenRecentSession(self.path.clone())
    }
}

fn session_path_label(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn session_button_visible(snapshot: &ToolbarSnapshot, event: &ToolbarEvent) -> bool {
    catalog::session_button_item_id(event).is_none_or(|id| !snapshot.toolbar_item_hidden(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarBindingHints;

    fn app_snapshot() -> ToolbarSnapshot {
        let mut state = make_test_input_state();
        state.toolbar_drawer_open = true;
        state.toolbar_drawer_tab = ToolbarDrawerTab::Session;
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
    }

    #[test]
    fn session_model_shows_active_target_controls_and_caps_recents() {
        let mut snapshot = app_snapshot();
        snapshot.active_session_name = Some("lecture.wayscriber-session".to_string());
        snapshot.active_session_path = Some(PathBuf::from("/tmp/lecture.wayscriber-session"));
        snapshot.recent_sessions = (0..5)
            .map(|index| SessionRecentSnapshot {
                display_name: format!("recent-{index}.wayscriber-session"),
                path: PathBuf::from(format!("/tmp/recent-{index}.wayscriber-session")),
            })
            .collect();

        let model = ToolbarSessionModel::from_snapshot(&snapshot).expect("session model");

        assert_eq!(model.active_name, "lecture.wayscriber-session");
        assert!(model.active_path_label.contains("/tmp/lecture"));
        assert_eq!(model.buttons.len(), 5);
        assert_eq!(model.button_columns(), SESSION_BUTTON_COLUMNS);
        assert_eq!(model.button_rows(), 2);
        assert!(model.buttons.iter().all(|button| button.enabled));
        assert_eq!(model.recents.len(), MAX_RECENT_SESSIONS);
        assert!(model.overwrite_confirmation.is_none());
    }

    #[test]
    fn session_model_disables_current_target_actions_without_active_target() {
        let mut snapshot = app_snapshot();
        snapshot.recent_sessions = vec![SessionRecentSnapshot {
            display_name: "recent.wayscriber-session".to_string(),
            path: PathBuf::from("/tmp/recent.wayscriber-session"),
        }];

        let model = ToolbarSessionModel::from_snapshot(&snapshot).expect("session model");

        assert!(!model.buttons[0].enabled);
        assert!(!model.buttons[1].enabled);
        assert!(!model.buttons[2].enabled);
        assert!(!model.buttons[3].enabled);
        assert!(model.buttons[4].enabled);
        assert!(matches!(
            model.buttons[4].event,
            ToolbarEvent::OpenConfigurator
        ));
        assert!(model.recents.is_empty());
        assert_eq!(model.active_path_label, "No persisted session target");
    }

    #[test]
    fn session_model_exposes_pending_save_as_overwrite_confirmation() {
        let mut snapshot = app_snapshot();
        let target = PathBuf::from("/tmp/existing.wayscriber-session");
        snapshot.active_session_path = Some(PathBuf::from("/tmp/current.wayscriber-session"));
        snapshot.pending_save_as_overwrite_path = Some(target.clone());

        let model = ToolbarSessionModel::from_snapshot(&snapshot).expect("session model");
        let confirmation = model
            .overwrite_confirmation
            .as_ref()
            .expect("overwrite confirmation");

        assert_eq!(confirmation.label, "existing.wayscriber-session");
        assert!(matches!(
            confirmation.confirm_event(),
            ToolbarEvent::SaveSessionAsConfirm(path) if path == target
        ));
        assert!(matches!(
            confirmation.cancel_event(),
            ToolbarEvent::SaveSessionAsCancel
        ));
    }

    #[test]
    fn session_model_is_hidden_outside_app_drawer() {
        let mut snapshot = app_snapshot();
        snapshot.drawer_tab = ToolbarDrawerTab::View;
        assert!(ToolbarSessionModel::from_snapshot(&snapshot).is_none());

        snapshot.drawer_tab = ToolbarDrawerTab::App;
        snapshot.drawer_open = false;
        assert!(ToolbarSessionModel::from_snapshot(&snapshot).is_none());
    }
}
