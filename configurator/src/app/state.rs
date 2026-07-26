use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use iced::Task;
use wayscriber::config::{Config, ConfigDocument, PRESET_SLOTS_MAX};

use crate::messages::Message;
use crate::models::{
    ColorPickerId, ConfigDraft, DaemonRuntimeStatus, DesktopEnvironment, DragMouseButton,
    KeybindingsTabId, SearchQuery, SessionCatalogState, TabId, ToolbarLayoutModeOption, UiTabId,
};

use super::blocking_jobs::{BlockingJobRequest, BlockingJobSubmission, BlockingJobs};

#[derive(Debug)]
pub(crate) struct ConfiguratorApp {
    pub(crate) draft: ConfigDraft,
    pub(crate) baseline: ConfigDraft,
    pub(crate) defaults: ConfigDraft,
    // The source document owns typed config, lossless TOML, and the guarded save revision.
    pub(crate) base_document: BaseDocumentState,
    pub(crate) path_resolver: wayscriber::paths::PathResolver,
    pub(super) blocking_jobs: BlockingJobs,
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
    pub(crate) is_dirty: bool,
    pub(crate) defaults_reset_pending: bool,
    pub(crate) last_backup_path: Option<PathBuf>,
    pub(crate) daemon_status: Option<DaemonRuntimeStatus>,
    pub(crate) daemon_shortcut_input: String,
    pub(crate) daemon_feedback: Option<String>,
    pub(crate) daemon_busy: bool,
    pub(crate) session_catalog: SessionCatalogState,
    pub(crate) search_query: SearchQuery,
    pub(crate) search_input_focus_hint: bool,
    pub(crate) startup_search_focus_pending: bool,
}

#[derive(Debug)]
pub(crate) enum BaseDocumentState {
    Loading {
        retained: Option<Box<ConfigDocument>>,
    },
    Unavailable,
    Ready(Box<ConfigDocument>),
    Saving(Box<ConfigDocumentSave>),
}

#[derive(Debug)]
pub(crate) struct ConfigDocumentSave {
    source_path: PathBuf,
    loaded_legacy_boards: bool,
    submitted_draft: ConfigDraft,
}

impl BaseDocumentState {
    pub(crate) fn document(&self) -> Option<&ConfigDocument> {
        match self {
            Self::Ready(document) => Some(document),
            Self::Loading { .. } | Self::Unavailable | Self::Saving(_) => None,
        }
    }

    pub(crate) fn source_path(&self) -> Option<&Path> {
        match self {
            Self::Ready(document) => Some(document.source_path()),
            Self::Saving(summary) => Some(&summary.source_path),
            Self::Loading {
                retained: Some(document),
            } => Some(document.source_path()),
            Self::Loading { retained: None } => None,
            Self::Unavailable => None,
        }
    }

    pub(crate) fn loaded_legacy_boards(&self) -> bool {
        match self {
            Self::Ready(document) => document.config().boards.is_none(),
            Self::Saving(summary) => summary.loaded_legacy_boards,
            Self::Loading {
                retained: Some(document),
            } => document.config().boards.is_none(),
            Self::Loading { retained: None } => false,
            Self::Unavailable => false,
        }
    }

    pub(crate) fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    pub(crate) fn is_saving(&self) -> bool {
        matches!(self, Self::Saving(_))
    }

    pub(crate) fn begin_load(&mut self) -> bool {
        let current = std::mem::replace(self, Self::Unavailable);
        match current {
            Self::Ready(document) => {
                *self = Self::Loading {
                    retained: Some(document),
                };
                true
            }
            Self::Unavailable => {
                *self = Self::Loading { retained: None };
                true
            }
            other => {
                *self = other;
                false
            }
        }
    }

    pub(crate) fn finish_load(
        &mut self,
        document: Option<Box<ConfigDocument>>,
    ) -> Result<(), Option<Box<ConfigDocument>>> {
        let current = std::mem::replace(self, Self::Unavailable);
        match current {
            Self::Loading { retained } => {
                *self = match document {
                    Some(document) => Self::Ready(document),
                    None => retained.map_or(Self::Unavailable, Self::Ready),
                };
                Ok(())
            }
            other => {
                *self = other;
                Err(document)
            }
        }
    }

    pub(crate) fn begin_save(
        &mut self,
        submitted_draft: ConfigDraft,
    ) -> Option<Box<ConfigDocument>> {
        let current = std::mem::replace(self, Self::Unavailable);
        match current {
            Self::Ready(document) => {
                *self = Self::Saving(Box::new(ConfigDocumentSave {
                    source_path: document.source_path().to_path_buf(),
                    loaded_legacy_boards: document.config().boards.is_none(),
                    submitted_draft,
                }));
                Some(document)
            }
            other => {
                *self = other;
                None
            }
        }
    }

    pub(crate) fn finish_save(
        &mut self,
        document: Box<ConfigDocument>,
    ) -> Result<ConfigDraft, Box<ConfigDocument>> {
        let current = std::mem::replace(self, Self::Unavailable);
        match current {
            Self::Saving(save) => {
                *self = Self::Ready(document);
                Ok(save.submitted_draft)
            }
            other => {
                *self = other;
                Err(document)
            }
        }
    }

    pub(crate) fn fail_save(&mut self) -> bool {
        let current = std::mem::replace(self, Self::Unavailable);
        match current {
            Self::Saving(_) => true,
            other => {
                *self = other;
                false
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
        let path_resolver = wayscriber::paths::PathResolver::from_process_environment();
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
            base_document: BaseDocumentState::Loading { retained: None },
            blocking_jobs: BlockingJobs::new(path_resolver.clone()),
            path_resolver,
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
            is_dirty: false,
            defaults_reset_pending: false,
            last_backup_path: None,
            daemon_status: None,
            daemon_shortcut_input: desktop.default_shortcut_input().to_string(),
            daemon_feedback: Some("Detecting background mode setup status...".to_string()),
            daemon_busy: false,
            session_catalog: SessionCatalogState::loading(),
            search_query: SearchQuery::default(),
            search_input_focus_hint: true,
            startup_search_focus_pending: true,
        };
        app.sync_all_color_picker_hex();

        let command = Task::batch(vec![
            app.submit_blocking_job(BlockingJobRequest::ConfigLoad),
            app.submit_blocking_job(BlockingJobRequest::DaemonStatus {
                preserve_feedback: false,
            }),
            app.submit_blocking_job(BlockingJobRequest::SessionCatalogLoad),
        ]);

        (app, command)
    }

    pub(super) fn refresh_dirty_flag(&mut self) {
        self.defaults_reset_pending = false;
        self.is_dirty = self.draft != self.baseline;
    }

    pub(super) fn submit_blocking_job(&mut self, request: BlockingJobRequest) -> Task<Message> {
        let BlockingJobSubmission {
            id: _job_id,
            started,
            cancellation: _cancellation,
        } = self.blocking_jobs.submit(request);
        started.map(Message::BlockingJobReady)
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
