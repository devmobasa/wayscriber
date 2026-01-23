use std::path::PathBuf;
use std::sync::Arc;

use iced::Command;
use wayscriber::config::Config;

use crate::messages::Message;
use crate::models::ConfigDraft;

use super::super::io::{load_config_from_disk, load_config_mtime, save_config_to_disk};
use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_config_loaded(
        &mut self,
        result: Result<Arc<Config>, String>,
    ) -> Command<Message> {
        self.is_loading = false;
        match result {
            Ok(config) => {
                let draft = ConfigDraft::from_config(config.as_ref());
                self.draft = draft.clone();
                self.baseline = draft;
                self.base_config = config.clone();
                self.override_mode = self.draft.ui_toolbar_layout_mode;
                self.boards_collapsed = vec![false; self.draft.boards.items.len()];
                self.color_picker_open = None;
                self.color_picker_advanced.clear();
                self.color_picker_hex.clear();
                self.sync_all_color_picker_hex();
                self.config_mtime = load_config_mtime(&self.config_path);
                self.is_dirty = false;
                self.status = StatusMessage::success("Configuration loaded from disk.");
            }
            Err(err) => {
                self.status =
                    StatusMessage::error(format!("Failed to load config from disk: {err}"));
            }
        }

        Command::none()
    }

    pub(super) fn handle_reload_requested(&mut self) -> Command<Message> {
        if !self.is_loading && !self.is_saving {
            self.is_loading = true;
            self.status = StatusMessage::info("Reloading configuration...");
            return Command::perform(load_config_from_disk(), Message::ConfigLoaded);
        }

        Command::none()
    }

    pub(super) fn handle_reset_to_defaults(&mut self) -> Command<Message> {
        if !self.is_loading {
            self.draft = self.defaults.clone();
            self.override_mode = self.draft.ui_toolbar_layout_mode;
            self.boards_collapsed = vec![false; self.draft.boards.items.len()];
            self.color_picker_open = None;
            self.color_picker_advanced.clear();
            self.color_picker_hex.clear();
            self.sync_all_color_picker_hex();
            self.status = StatusMessage::info("Loaded default configuration (not saved).");
            self.refresh_dirty_flag();
        }

        Command::none()
    }

    pub(super) fn handle_save_requested(&mut self) -> Command<Message> {
        if self.is_saving {
            return Command::none();
        }
        if self.config_changed_on_disk() {
            self.status =
                StatusMessage::error("Configuration changed on disk. Reload before saving.");
            return Command::none();
        }

        match self.draft.to_config(self.base_config.as_ref()) {
            Ok(mut config) => {
                config.validate_and_clamp();
                self.is_saving = true;
                self.status = StatusMessage::info("Saving configuration...");
                Command::perform(save_config_to_disk(config), Message::ConfigSaved)
            }
            Err(errors) => {
                let message = errors
                    .into_iter()
                    .map(|err| format!("{}: {}", err.field, err.message))
                    .collect::<Vec<_>>()
                    .join("\n");
                self.status = StatusMessage::error(format!(
                    "Cannot save due to validation errors:\n{message}"
                ));
                Command::none()
            }
        }
    }

    pub(super) fn handle_config_saved(
        &mut self,
        result: Result<(Option<PathBuf>, Arc<Config>), String>,
    ) -> Command<Message> {
        self.is_saving = false;
        match result {
            Ok((backup, saved_config)) => {
                let draft = ConfigDraft::from_config(saved_config.as_ref());
                self.last_backup_path = backup.clone();
                self.draft = draft.clone();
                self.baseline = draft;
                self.base_config = saved_config.clone();
                self.config_mtime = load_config_mtime(&self.config_path);
                self.boards_collapsed = vec![false; self.draft.boards.items.len()];
                self.color_picker_open = None;
                self.color_picker_advanced.clear();
                self.color_picker_hex.clear();
                self.sync_all_color_picker_hex();
                self.is_dirty = false;
                let mut msg = "Configuration saved successfully.".to_string();
                if let Some(path) = backup {
                    msg.push_str(&format!("\nBackup created at {}", path.display()));
                }
                self.status = StatusMessage::success(msg);
            }
            Err(err) => {
                self.status = StatusMessage::error(format!("Failed to save configuration: {err}"));
            }
        }

        Command::none()
    }
}
