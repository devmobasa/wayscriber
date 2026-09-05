use std::collections::HashMap;

use wayscriber::config::{Config, MigrationPreview, PRESET_SLOTS_MAX};

use crate::models::{
    ColorPickerId, ConfigDraft, DesktopEnvironment, DragMouseButton, KeybindingField,
    KeybindingsTabId, SearchQuery, SessionCatalogState, ShortcutManagerFilter, ShortcutManagerSort,
    StartupRequest, TabId, ToolbarLayoutModeOption, UiTabId,
};

use super::effects::Effect;

#[derive(Debug)]
pub(crate) struct ConfiguratorApp {
    pub(crate) draft: ConfigDraft,
    pub(crate) baseline: ConfigDraft,
    pub(crate) defaults: ConfigDraft,
    pub(crate) document: super::document_workflow::DocumentWorkflow,
    pub(crate) status: StatusMessage,
    pub(crate) active_tab: TabId,
    pub(crate) active_ui_tab: UiTabId,
    pub(crate) active_keybindings_tab: KeybindingsTabId,
    pub(crate) keybindings_show_all: bool,
    pub(crate) shortcut_filter: ShortcutManagerFilter,
    pub(crate) shortcut_sort: ShortcutManagerSort,
    pub(crate) selected_keybinding: Option<KeybindingField>,
    pub(crate) keybinding_focus_serial: u64,
    pub(crate) active_drawing_drag_button: Option<DragMouseButton>,
    pub(crate) preset_collapsed: Vec<bool>,
    pub(crate) boards_collapsed: Vec<bool>,
    pub(crate) color_picker_hex: HashMap<ColorPickerId, String>,
    pub(crate) override_mode: ToolbarLayoutModeOption,
    pub(crate) is_dirty: bool,
    /// The destructive question the user can currently answer.
    ///
    /// One typed identity owns both confirmation surfaces so opening one
    /// replaces the other instead of leaving two independently armed actions
    /// on screen.
    pub(crate) pending_confirmation: Option<PendingConfirmation>,
    pub(crate) migration: super::migration_workflow::MigrationWorkflow,
    pub(crate) daemon: super::daemon_workflow::DaemonWorkflow,
    pub(crate) session_catalog: SessionCatalogState,
    pub(crate) search_query: SearchQuery,
    /// Bumped once per request to put the caret in the search box. The shell
    /// grabs focus when the serial it last honored falls behind, so a request
    /// focuses once instead of on every view refresh.
    pub(crate) search_focus_serial: u64,
    pub(crate) startup_search_focus_pending: bool,
    /// What the launching process asked to open, taken by the first config
    /// load and empty from then on.
    pub(crate) startup_request: StartupRequest,
    pub(crate) shortcuts: super::shortcut_workflow::ShortcutWorkflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfirmationPrompt {
    DefaultsReset,
    SessionClear,
    ShortcutResetVisible,
    ShortcutResetAll,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingConfirmation {
    DefaultsReset,
    SessionClear(String),
    ShortcutResetVisible(Vec<KeybindingField>),
    ShortcutResetAll,
}

impl PendingConfirmation {
    pub(crate) fn prompt(&self) -> ConfirmationPrompt {
        match self {
            PendingConfirmation::DefaultsReset => ConfirmationPrompt::DefaultsReset,
            PendingConfirmation::SessionClear(_) => ConfirmationPrompt::SessionClear,
            PendingConfirmation::ShortcutResetVisible(_) => {
                ConfirmationPrompt::ShortcutResetVisible
            }
            PendingConfirmation::ShortcutResetAll => ConfirmationPrompt::ShortcutResetAll,
        }
    }
}

impl ConfirmationPrompt {
    pub(crate) fn message(self) -> &'static str {
        match self {
            ConfirmationPrompt::DefaultsReset => {
                "Defaults will replace the current draft with built-in defaults. Press \"Confirm Defaults\" to continue."
            }
            ConfirmationPrompt::SessionClear => {
                "Clear saved data removes the selected session primary and non-lock sidecars. Press Confirm Clear to continue."
            }
            ConfirmationPrompt::ShortcutResetVisible => {
                "Reset Visible restores the currently listed keybindings to their defaults. Press \"Confirm Reset Visible\" to continue. Nothing is written until you Save."
            }
            ConfirmationPrompt::ShortcutResetAll => {
                "Reset All restores every keybinding to its built-in default. Press \"Confirm Reset All\" to continue. Nothing is written until you Save."
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum StatusMessage {
    Idle,
    Info(String),
    Success(String),
    Error(String),
    Warning(String),
    Confirmation(ConfirmationPrompt),
}

impl StatusMessage {
    pub(crate) fn idle() -> Self {
        StatusMessage::Idle
    }

    pub(crate) fn info(message: impl Into<String>) -> Self {
        StatusMessage::Info(message.into())
    }

    pub(crate) fn success(message: impl Into<String>) -> Self {
        StatusMessage::Success(message.into())
    }

    pub(crate) fn error(message: impl Into<String>) -> Self {
        StatusMessage::Error(message.into())
    }

    pub(crate) fn warning(message: impl Into<String>) -> Self {
        StatusMessage::Warning(message.into())
    }

    pub(crate) fn confirmation(prompt: ConfirmationPrompt) -> Self {
        StatusMessage::Confirmation(prompt)
    }

    pub(crate) fn is_confirmation(&self, prompt: ConfirmationPrompt) -> bool {
        matches!(self, StatusMessage::Confirmation(current) if *current == prompt)
    }

    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            StatusMessage::Idle => None,
            StatusMessage::Info(text)
            | StatusMessage::Success(text)
            | StatusMessage::Error(text)
            | StatusMessage::Warning(text) => Some(text.as_str()),
            StatusMessage::Confirmation(prompt) => Some(prompt.message()),
        }
    }

    /// Adds a sentence without discarding what is already there.
    ///
    /// The load status can be carrying this file's diagnostics, and a note
    /// about a startup argument is not worth losing them over.
    pub(crate) fn with_note(self, note: &str) -> Self {
        match self {
            StatusMessage::Idle => StatusMessage::warning(note),
            StatusMessage::Info(text)
            | StatusMessage::Success(text)
            | StatusMessage::Warning(text) => StatusMessage::warning(format!("{text}\n{note}")),
            StatusMessage::Confirmation(prompt) => {
                StatusMessage::warning(format!("{}\n{note}", prompt.message()))
            }
            // A failed load is the more urgent of the two; keep its styling.
            StatusMessage::Error(text) => StatusMessage::error(format!("{text}\n{note}")),
        }
    }
}

impl ConfiguratorApp {
    /// The app as a launch with no destination.
    ///
    /// Test-only: the binary always has a parsed launch request to pass, even
    /// when it is the empty one.
    #[cfg(test)]
    pub(crate) fn new_app() -> (Self, Vec<Effect>) {
        Self::new_app_with_startup(StartupRequest::default())
    }

    pub(crate) fn new_app_with_startup(startup: StartupRequest) -> (Self, Vec<Effect>) {
        let default_config = Config::default();
        let defaults = ConfigDraft::from_config(&default_config);
        let baseline = defaults.clone();
        let override_mode = defaults.ui_toolbar_layout_mode;
        let boards_len = defaults.boards.items.len();
        let desktop = DesktopEnvironment::detect_current();

        let mut app = Self {
            draft: baseline.clone(),
            baseline,
            defaults,
            document: super::document_workflow::DocumentWorkflow::loading(),
            status: StatusMessage::info("Loading configuration..."),
            active_tab: TabId::Daemon,
            active_ui_tab: UiTabId::Toolbar,
            active_keybindings_tab: KeybindingsTabId::General,
            keybindings_show_all: true,
            shortcut_filter: ShortcutManagerFilter::All,
            shortcut_sort: ShortcutManagerSort::Category,
            selected_keybinding: None,
            keybinding_focus_serial: 0,
            active_drawing_drag_button: None,
            preset_collapsed: vec![false; PRESET_SLOTS_MAX],
            boards_collapsed: vec![false; boards_len],
            color_picker_hex: HashMap::new(),
            override_mode,
            is_dirty: false,
            pending_confirmation: None,
            migration: super::migration_workflow::MigrationWorkflow::default(),
            daemon: super::daemon_workflow::DaemonWorkflow::new(desktop),
            session_catalog: SessionCatalogState::loading(),
            search_query: SearchQuery::default(),
            search_focus_serial: 0,
            startup_search_focus_pending: true,
            startup_request: startup,
            shortcuts: super::shortcut_workflow::ShortcutWorkflow::default(),
        };
        app.sync_all_color_picker_hex();

        let initial_status_request_id = app.daemon.latest_status_request_id;
        let effects = vec![
            Effect::LoadConfig,
            Effect::LoadDaemonStatus {
                request_id: initial_status_request_id,
            },
            Effect::LoadSessionCatalog,
        ];

        (app, effects)
    }

    pub(super) fn refresh_dirty_flag(&mut self) {
        self.clear_defaults_confirmation();
        self.clear_keybinding_reset_confirmation();
        self.is_dirty = self.draft != self.baseline;
    }

    pub(crate) fn defaults_reset_pending(&self) -> bool {
        matches!(
            self.pending_confirmation,
            Some(PendingConfirmation::DefaultsReset)
        )
    }

    pub(crate) fn pending_session_clear_id(&self) -> Option<&str> {
        match self.pending_confirmation.as_ref() {
            Some(PendingConfirmation::SessionClear(id)) => Some(id.as_str()),
            _ => None,
        }
    }

    pub(crate) fn shortcut_reset_visible_pending(&self) -> bool {
        matches!(
            self.pending_confirmation,
            Some(PendingConfirmation::ShortcutResetVisible(_))
        )
    }

    pub(crate) fn shortcut_reset_all_pending(&self) -> bool {
        matches!(
            self.pending_confirmation,
            Some(PendingConfirmation::ShortcutResetAll)
        )
    }

    pub(crate) fn pending_shortcut_reset_visible_fields(&self) -> Option<&[KeybindingField]> {
        match self.pending_confirmation.as_ref() {
            Some(PendingConfirmation::ShortcutResetVisible(fields)) => Some(fields.as_slice()),
            _ => None,
        }
    }

    pub(super) fn clear_defaults_confirmation(&mut self) {
        if self.defaults_reset_pending() {
            self.pending_confirmation = None;
        }
    }

    pub(super) fn clear_session_confirmation(&mut self) {
        if self.pending_session_clear_id().is_some() {
            self.pending_confirmation = None;
        }
    }

    pub(super) fn clear_keybinding_reset_confirmation(&mut self) {
        if self.shortcut_reset_visible_pending() || self.shortcut_reset_all_pending() {
            self.pending_confirmation = None;
        }
    }

    /// The migration offer to show, if there is one to show.
    ///
    /// Dismissing hides the offer for the rest of this app run, including
    /// across reloads of the same file: the user answered the question about
    /// this configuration, and pressing Reload is not them asking it again. The
    /// next launch offers it afresh, because the file still has the old
    /// revision.
    ///
    /// The answer is about the file, not the path that reached it. A reload
    /// that lands on a different file — `config.toml` retargeted to another
    /// profile between the two — is a configuration the user has not been asked
    /// about, so refreshing the preview clears the dismissal and its offer
    /// shows.
    pub(crate) fn pending_migration(&self) -> Option<&MigrationPreview> {
        self.migration.pending()
    }

    pub(crate) fn shortcut_recorder_active(&self) -> bool {
        self.shortcuts.recorder().is_some()
    }

    pub(super) fn clear_shortcut_editing(&mut self) {
        self.shortcuts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_dirty_flag_tracks_draft_vs_baseline() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        app.refresh_dirty_flag();
        assert!(!app.is_dirty);

        app.draft.capture_enabled = !app.draft.capture_enabled;
        app.refresh_dirty_flag();
        assert!(app.is_dirty);
    }

    #[test]
    fn new_app_starts_with_the_startup_focus_offer_open() {
        let (app, effects) = ConfiguratorApp::new_app();

        assert_eq!(app.search_focus_serial, 0);
        assert!(app.startup_search_focus_pending);
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::LoadConfig,
                Effect::LoadDaemonStatus { .. },
                Effect::LoadSessionCatalog
            ]
        ));
    }
}
