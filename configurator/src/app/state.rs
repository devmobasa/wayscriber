use std::collections::{HashMap, HashSet};
use std::{path::PathBuf, sync::Arc};

use iced::Task;
use wayscriber::config::{Config, ConfigDocument, PRESET_SLOTS_MAX};

use crate::messages::Message;
use crate::models::{
    ColorPickerId, ConfigDraft, DaemonRuntimeStatus, DesktopEnvironment, DragMouseButton,
    KeybindingsTabId, SearchQuery, SessionCatalogState, TabId, ToolbarLayoutModeOption, UiTabId,
};

use super::daemon_setup::load_daemon_runtime_status;
use super::io::load_config_from_disk;
use super::session_catalog::load_session_catalog;

#[derive(Debug)]
pub(crate) struct ConfiguratorApp {
    pub(crate) draft: ConfigDraft,
    pub(crate) baseline: ConfigDraft,
    pub(crate) defaults: ConfigDraft,
    // The source document owns typed config, lossless TOML, and the guarded save revision.
    pub(crate) base_document: Option<Arc<ConfigDocument>>,
    pub(crate) status: StatusMessage,
    pub(crate) active_tab: TabId,
    pub(crate) active_ui_tab: UiTabId,
    pub(crate) active_keybindings_tab: KeybindingsTabId,
    pub(crate) active_drawing_drag_button: Option<DragMouseButton>,
    pub(crate) preset_collapsed: Vec<bool>,
    pub(crate) boards_collapsed: Vec<bool>,
    pub(crate) color_picker_open: Option<ColorPickerId>,
    pub(crate) color_picker_advanced: HashSet<ColorPickerId>,
    pub(crate) color_picker_hex: HashMap<ColorPickerId, String>,
    pub(crate) override_mode: ToolbarLayoutModeOption,
    pub(crate) is_loading: bool,
    pub(crate) is_saving: bool,
    pub(crate) is_dirty: bool,
    pub(crate) defaults_reset_pending: bool,
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
    pub(crate) search_input_focus_hint: bool,
    pub(crate) startup_search_focus_pending: bool,
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
}

impl ConfiguratorApp {
    pub(crate) fn new_app() -> (Self, Task<Message>) {
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
            color_picker_open: None,
            color_picker_advanced: HashSet::new(),
            color_picker_hex: HashMap::new(),
            override_mode,
            is_loading: true,
            is_saving: false,
            is_dirty: false,
            defaults_reset_pending: false,
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
            search_input_focus_hint: true,
            startup_search_focus_pending: true,
        };
        app.sync_all_color_picker_hex();

        let initial_status_request_id = app.daemon_latest_status_request_id;
        let command = Task::batch(vec![
            Task::perform(load_config_from_disk(), Message::ConfigLoaded),
            Task::perform(load_daemon_runtime_status(), move |result| {
                Message::DaemonStatusLoaded(initial_status_request_id, result)
            }),
            Task::perform(load_session_catalog(), Message::SessionCatalogLoaded),
        ]);

        (app, command)
    }

    pub(super) fn refresh_dirty_flag(&mut self) {
        self.defaults_reset_pending = false;
        self.is_dirty = self.draft != self.baseline;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_dirty_flag_tracks_draft_vs_baseline() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        app.refresh_dirty_flag();
        assert!(!app.is_dirty);

        app.draft.capture_enabled = !app.draft.capture_enabled;
        app.refresh_dirty_flag();
        assert!(app.is_dirty);
    }

    #[test]
    fn new_app_starts_with_search_focus_hint() {
        let (app, _cmd) = ConfiguratorApp::new_app();

        assert!(app.search_input_focus_hint);
        assert!(app.startup_search_focus_pending);
    }
}
