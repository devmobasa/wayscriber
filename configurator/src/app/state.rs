use std::collections::{HashMap, HashSet};
use std::{path::PathBuf, sync::Arc, time::SystemTime};

use iced::Command;
use wayscriber::config::{Config, PRESET_SLOTS_MAX};

use crate::messages::Message;
use crate::models::{
    ColorPickerId, ConfigDraft, KeybindingsTabId, TabId, ToolbarLayoutModeOption, UiTabId,
};

use super::io::load_config_from_disk;

#[derive(Debug)]
pub(crate) struct ConfiguratorApp {
    pub(crate) draft: ConfigDraft,
    pub(crate) baseline: ConfigDraft,
    pub(crate) defaults: ConfigDraft,
    // Base config loaded from disk to preserve unknown fields when saving.
    pub(crate) base_config: Arc<Config>,
    pub(crate) status: StatusMessage,
    pub(crate) active_tab: TabId,
    pub(crate) active_ui_tab: UiTabId,
    pub(crate) active_keybindings_tab: KeybindingsTabId,
    pub(crate) preset_collapsed: Vec<bool>,
    pub(crate) boards_collapsed: Vec<bool>,
    pub(crate) color_picker_open: Option<ColorPickerId>,
    pub(crate) color_picker_advanced: HashSet<ColorPickerId>,
    pub(crate) color_picker_hex: HashMap<ColorPickerId, String>,
    pub(crate) override_mode: ToolbarLayoutModeOption,
    pub(crate) is_loading: bool,
    pub(crate) is_saving: bool,
    pub(crate) is_dirty: bool,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) config_mtime: Option<SystemTime>,
    pub(crate) last_backup_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub(crate) enum StatusMessage {
    Idle,
    Info(String),
    Success(String),
    Error(String),
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
}

impl ConfiguratorApp {
    pub(crate) fn new_app() -> (Self, Command<Message>) {
        let default_config = Config::default();
        let defaults = ConfigDraft::from_config(&default_config);
        let baseline = defaults.clone();
        let override_mode = defaults.ui_toolbar_layout_mode;
        let boards_len = defaults.boards.items.len();
        let config_path = Config::get_config_path().ok();
        let base_config = Arc::new(default_config.clone());

        let mut app = Self {
            draft: baseline.clone(),
            baseline,
            defaults,
            base_config,
            status: StatusMessage::info("Loading configuration..."),
            active_tab: TabId::Drawing,
            active_ui_tab: UiTabId::Toolbar,
            active_keybindings_tab: KeybindingsTabId::General,
            preset_collapsed: vec![false; PRESET_SLOTS_MAX],
            boards_collapsed: vec![false; boards_len],
            color_picker_open: None,
            color_picker_advanced: HashSet::new(),
            color_picker_hex: HashMap::new(),
            override_mode,
            is_loading: true,
            is_saving: false,
            is_dirty: false,
            config_path,
            config_mtime: None,
            last_backup_path: None,
        };
        app.sync_all_color_picker_hex();

        let command = Command::batch(vec![Command::perform(
            load_config_from_disk(),
            Message::ConfigLoaded,
        )]);

        (app, command)
    }

    pub(super) fn config_changed_on_disk(&self) -> bool {
        let Some(last_modified) = self.config_mtime else {
            return false;
        };
        let Some(path) = self.config_path.as_ref() else {
            return false;
        };

        match std::fs::metadata(path).and_then(|meta| meta.modified()) {
            Ok(modified) => modified > last_modified,
            Err(_) => false,
        }
    }

    pub(super) fn refresh_dirty_flag(&mut self) {
        self.is_dirty = self.draft != self.baseline;
    }
}
