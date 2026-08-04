use std::collections::HashMap;
use std::path::PathBuf;

use wayscriber::config::{
    Config, ConfigDocument, ConfigValidationReport, MigrationPreview, PRESET_SLOTS_MAX,
};

use crate::models::{
    ColorPickerId, ConfigDraft, DaemonRuntimeStatus, DesktopEnvironment, DragMouseButton,
    KeybindingsTabId, SearchQuery, SessionCatalogState, StartupRequest, TabId,
    ToolbarLayoutModeOption, UiTabId,
};

use super::effects::Effect;

#[derive(Debug)]
pub(crate) struct ConfiguratorApp {
    pub(crate) draft: ConfigDraft,
    pub(crate) baseline: ConfigDraft,
    pub(crate) defaults: ConfigDraft,
    // The source document owns typed config, lossless TOML, and the guarded
    // save revision. Owned outright and moved into a running save, so it is
    // `None` exactly while a write holds it and for as long as no load has
    // produced one.
    pub(crate) base_document: Option<ConfigDocument>,
    pub(crate) status: StatusMessage,
    pub(crate) active_tab: TabId,
    pub(crate) active_ui_tab: UiTabId,
    pub(crate) active_keybindings_tab: KeybindingsTabId,
    pub(crate) active_drawing_drag_button: Option<DragMouseButton>,
    pub(crate) preset_collapsed: Vec<bool>,
    pub(crate) boards_collapsed: Vec<bool>,
    pub(crate) color_picker_hex: HashMap<ColorPickerId, String>,
    pub(crate) override_mode: ToolbarLayoutModeOption,
    pub(crate) is_loading: bool,
    pub(crate) is_saving: bool,
    pub(crate) is_dirty: bool,
    pub(crate) defaults_reset_pending: bool,
    /// What an accepted migration would change in the loaded configuration.
    /// Held here rather than in `status` so an expired or replaced status
    /// message cannot take the offer away with it.
    pub(crate) migration_preview: Option<MigrationPreview>,
    /// The document whose migration offer the user dismissed, named by the file
    /// the config path resolved to rather than by the path itself. `None` while
    /// no offer has been dismissed.
    pub(crate) migration_dismissed: Option<PathBuf>,
    /// What validating the configuration the running Save is writing had to
    /// change in `[keybindings]`, held until that write reports back.
    ///
    /// The resolution reaches the file, so the reloaded document cannot show
    /// it: this is the only carrier from the moment the config is built to the
    /// status the finished save renders.
    pub(crate) pending_save_validation: ConfigValidationReport,
    pub(crate) last_backup_path: Option<PathBuf>,
    pub(crate) daemon_status: Option<DaemonRuntimeStatus>,
    pub(crate) daemon_shortcut_input: String,
    pub(crate) daemon_feedback: Option<String>,
    pub(crate) daemon_busy: bool,
    pub(crate) daemon_next_status_request_id: u64,
    pub(crate) daemon_latest_status_request_id: u64,
    pub(crate) daemon_preserve_feedback_status_request_id: Option<u64>,
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
}

#[derive(Debug, Clone)]
pub(crate) enum StatusMessage {
    Idle,
    Info(String),
    Success(String),
    Error(String),
    Warning(String),
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
            base_document: None,
            status: StatusMessage::info("Loading configuration..."),
            active_tab: TabId::Daemon,
            active_ui_tab: UiTabId::Toolbar,
            active_keybindings_tab: KeybindingsTabId::General,
            active_drawing_drag_button: None,
            preset_collapsed: vec![false; PRESET_SLOTS_MAX],
            boards_collapsed: vec![false; boards_len],
            color_picker_hex: HashMap::new(),
            override_mode,
            is_loading: true,
            is_saving: false,
            is_dirty: false,
            defaults_reset_pending: false,
            migration_preview: None,
            migration_dismissed: None,
            pending_save_validation: ConfigValidationReport::default(),
            last_backup_path: None,
            daemon_status: None,
            daemon_shortcut_input: desktop.default_shortcut_input().to_string(),
            daemon_feedback: Some("Detecting background mode setup status...".to_string()),
            daemon_busy: false,
            daemon_next_status_request_id: 2,
            daemon_latest_status_request_id: 1,
            daemon_preserve_feedback_status_request_id: None,
            session_catalog: SessionCatalogState::loading(),
            search_query: SearchQuery::default(),
            search_focus_serial: 0,
            startup_search_focus_pending: true,
            startup_request: startup,
        };
        app.sync_all_color_picker_hex();

        let initial_status_request_id = app.daemon_latest_status_request_id;
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
        self.defaults_reset_pending = false;
        self.is_dirty = self.draft != self.baseline;
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
        if self.migration_dismissed.is_some() {
            return None;
        }
        self.migration_preview.as_ref()
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
