use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use wayscriber::config::ConfigDocument;

use crate::messages::Message;
use crate::models::ConfigDraft;

use super::super::io::{load_config_from_disk, save_config_to_disk};
use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_config_loaded(
        &mut self,
        result: Result<(Arc<ConfigDocument>, Option<String>), String>,
    ) -> Task<Message> {
        self.is_loading = false;
        match result {
            Ok((document, repair_warning)) => {
                let draft = ConfigDraft::from_config(document.config());
                self.draft = draft.clone();
                self.baseline = draft;
                self.base_document = Some(document.clone());
                self.override_mode = self.draft.ui_toolbar_layout_mode;
                self.boards_collapsed = vec![false; self.draft.boards.items.len()];
                self.color_picker_open = None;
                self.color_picker_advanced.clear();
                self.color_picker_hex.clear();
                self.sync_all_color_picker_hex();
                self.is_dirty = false;
                self.defaults_reset_pending = false;
                self.status = repair_warning.map_or_else(
                    || config_document_status(&document, "Configuration loaded from disk."),
                    |warning| {
                        StatusMessage::warning(format!(
                            "The configuration could not be parsed, so built-in defaults were loaded for repair. Saving will create a backup before replacing the unreadable configuration with this draft. Unknown settings are retained only when the TOML structure is parseable and they can be separated safely; malformed TOML content remains only in the backup.\n{warning}"
                        ))
                    },
                );
            }
            Err(err) => {
                self.status =
                    StatusMessage::error(format!("Failed to load config from disk: {err}"));
            }
        }

        self.handle_startup_search_focus_config_fallback()
    }

    pub(super) fn handle_reload_requested(&mut self) -> Task<Message> {
        if !self.is_loading && !self.is_saving {
            self.is_loading = true;
            self.defaults_reset_pending = false;
            self.status = StatusMessage::info("Reloading configuration...");
            return Task::perform(load_config_from_disk(), Message::ConfigLoaded);
        }

        Task::none()
    }

    pub(super) fn handle_reset_to_defaults_requested(&mut self) -> Task<Message> {
        if !self.is_loading && !self.is_saving {
            self.defaults_reset_pending = true;
            self.status = StatusMessage::warning(
                "Defaults will replace the current draft with built-in defaults. Press Confirm Defaults to continue.",
            );
        }

        Task::none()
    }

    pub(super) fn handle_reset_to_defaults_canceled(&mut self) -> Task<Message> {
        self.defaults_reset_pending = false;
        self.status = StatusMessage::idle();
        Task::none()
    }

    pub(super) fn handle_reset_to_defaults_confirmed(&mut self) -> Task<Message> {
        if self.defaults_reset_pending && !self.is_loading && !self.is_saving {
            self.draft = self.defaults.clone();
            self.override_mode = self.draft.ui_toolbar_layout_mode;
            self.boards_collapsed = vec![false; self.draft.boards.items.len()];
            self.color_picker_open = None;
            self.color_picker_advanced.clear();
            self.color_picker_hex.clear();
            self.sync_all_color_picker_hex();
            self.defaults_reset_pending = false;
            self.status = StatusMessage::info("Loaded default configuration (not saved).");
            self.refresh_dirty_flag();
        }

        Task::none()
    }

    pub(super) fn handle_save_requested(&mut self) -> Task<Message> {
        if self.is_saving {
            return Task::none();
        }
        self.defaults_reset_pending = false;
        let Some(document) = self.base_document.clone() else {
            self.status = StatusMessage::error(
                "Configuration has not loaded successfully. Reload before saving.",
            );
            return Task::none();
        };

        match self.draft.to_config(document.config()) {
            Ok(mut config) => {
                config.validate_and_clamp();
                self.is_saving = true;
                self.status = StatusMessage::info("Saving configuration...");
                Task::perform(save_config_to_disk(document, config), Message::ConfigSaved)
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
                Task::none()
            }
        }
    }

    pub(super) fn handle_config_saved(
        &mut self,
        result: Result<(Option<PathBuf>, Arc<ConfigDocument>), String>,
    ) -> Task<Message> {
        self.is_saving = false;
        match result {
            Ok((backup, saved_document)) => {
                let draft = ConfigDraft::from_config(saved_document.config());
                self.last_backup_path = backup.clone();
                self.draft = draft.clone();
                self.baseline = draft;
                self.base_document = Some(saved_document.clone());
                self.boards_collapsed = vec![false; self.draft.boards.items.len()];
                self.color_picker_open = None;
                self.color_picker_advanced.clear();
                self.color_picker_hex.clear();
                self.sync_all_color_picker_hex();
                self.is_dirty = false;
                self.defaults_reset_pending = false;
                let mut msg = "Configuration saved successfully.".to_string();
                if let Some(path) = backup {
                    msg.push_str(&format!("\nBackup created at {}", path.display()));
                }
                self.status = config_document_status(&saved_document, &msg);
            }
            Err(err) => {
                self.status = StatusMessage::error(format!("Failed to save configuration: {err}"));
            }
        }

        Task::none()
    }
}

fn config_document_status(document: &ConfigDocument, success: &str) -> StatusMessage {
    if document.diagnostics().is_empty() {
        return StatusMessage::success(success);
    }

    let shown = document
        .diagnostics()
        .iter()
        .take(8)
        .map(|diagnostic| diagnostic.path())
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = document.diagnostics().len().saturating_sub(8);
    let suffix = if remaining == 0 {
        String::new()
    } else {
        format!(", and {remaining} more")
    };
    StatusMessage::warning(format!(
        "{success}\nUnrecognized settings were preserved: {shown}{suffix}."
    ))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::models::{ColorPickerId, ToggleField};

    fn status_contains(status: &StatusMessage, needle: &str) -> bool {
        match status {
            StatusMessage::Info(text)
            | StatusMessage::Success(text)
            | StatusMessage::Error(text)
            | StatusMessage::Warning(text) => text.contains(needle),
            StatusMessage::Idle => false,
        }
    }

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temp_config_document(name: &str, contents: &str) -> (PathBuf, Arc<ConfigDocument>) {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "wayscriber-configurator-update-config-{}-{sequence}-{name}.toml",
            std::process::id(),
        ));
        std::fs::write(&path, contents).expect("write test config");
        let document = ConfigDocument::load_from_path(&path).expect("load test config document");
        (path, Arc::new(document))
    }

    #[test]
    fn handle_config_loaded_success_resets_loading_and_dirty_state() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        app.color_picker_open = Some(ColorPickerId::StatusBarBg);
        app.is_dirty = true;

        let (path, document) = temp_config_document("loaded", "");
        let _ = app.handle_config_loaded(Ok((document, None)));

        assert!(!app.is_loading);
        assert!(!app.is_dirty);
        assert!(app.color_picker_open.is_none());
        assert_eq!(app.boards_collapsed.len(), app.draft.boards.items.len());
        assert!(status_contains(
            &app.status,
            "Configuration loaded from disk."
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn handle_config_loaded_uses_startup_search_focus_fallback_once() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();

        let (first_path, first) = temp_config_document("focus-first", "");
        let _ = app.handle_config_loaded(Ok((first, None)));

        assert!(app.search_input_focus_hint);
        assert!(!app.startup_search_focus_pending);

        app.search_input_focus_hint = false;
        let (second_path, second) = temp_config_document("focus-second", "");
        let _ = app.handle_config_loaded(Ok((second, None)));

        assert!(!app.search_input_focus_hint);
        let _ = std::fs::remove_file(first_path);
        let _ = std::fs::remove_file(second_path);
    }

    #[test]
    fn handle_config_loaded_error_preserves_the_last_good_document_and_draft() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (path, document) = temp_config_document("before-reload-error", "");
        let _ = app.handle_config_loaded(Ok((document.clone(), None)));
        app.draft.capture_enabled = !app.draft.capture_enabled;
        let draft = app.draft.clone();

        let _ = app.handle_config_loaded(Err("broken".to_string()));

        assert!(!app.is_loading);
        assert!(Arc::ptr_eq(
            app.base_document.as_ref().expect("last good document"),
            &document
        ));
        assert_eq!(app.draft, draft);
        assert!(status_contains(
            &app.status,
            "Failed to load config from disk: broken"
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn handle_config_loaded_repair_document_allows_saving() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (path, document) = temp_config_document("repair", "");

        let _ = app.handle_config_loaded(Ok((
            document,
            Some("invalid type: string, expected u32".to_string()),
        )));

        assert!(app.base_document.is_some());
        assert!(matches!(app.status, StatusMessage::Warning(_)));
        assert!(status_contains(&app.status, "loaded for repair"));
        assert!(status_contains(
            &app.status,
            "malformed TOML content remains only in the backup"
        ));
        let _ = app.handle_save_requested();
        assert!(app.is_saving);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn handle_config_loaded_surfaces_preserved_unknown_settings() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (path, document) =
            temp_config_document("unknown", "future_configurator_option = true\n");

        let _ = app.handle_config_loaded(Ok((document, None)));

        assert!(matches!(app.status, StatusMessage::Warning(_)));
        assert!(status_contains(&app.status, "future_configurator_option"));
        assert!(status_contains(&app.status, "were preserved"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn handle_save_requested_blocks_without_loaded_document() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();

        let _ = app.handle_save_requested();

        assert!(!app.is_saving);
        assert!(status_contains(
            &app.status,
            "Configuration has not loaded successfully"
        ));
    }

    #[test]
    fn handle_save_requested_sets_saving_for_valid_draft() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        app.is_saving = false;
        let (path, document) = temp_config_document("save-request", "");
        let _ = app.handle_config_loaded(Ok((document, None)));

        let _ = app.handle_save_requested();

        assert!(app.is_saving);
        assert!(status_contains(&app.status, "Saving configuration..."));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reset_to_defaults_requires_confirmation() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        app.is_loading = false;
        app.draft.capture_enabled = !app.defaults.capture_enabled;
        let changed_draft = app.draft.clone();

        let _ = app.handle_reset_to_defaults_requested();

        assert!(app.defaults_reset_pending);
        assert_eq!(app.draft, changed_draft);
        assert!(status_contains(&app.status, "Confirm Defaults"));

        let _ = app.handle_reset_to_defaults_confirmed();

        assert!(!app.defaults_reset_pending);
        assert_eq!(app.draft, app.defaults);
        assert!(status_contains(&app.status, "Loaded default configuration"));
    }

    #[test]
    fn reset_to_defaults_confirmation_is_canceled_by_draft_edit() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        app.is_loading = false;

        let _ = app.handle_reset_to_defaults_requested();
        let _ = app.handle_toggle_changed(ToggleField::CaptureEnabled, !app.draft.capture_enabled);
        let edited_draft = app.draft.clone();

        assert!(!app.defaults_reset_pending);
        assert!(matches!(app.status, StatusMessage::Idle));

        let _ = app.handle_reset_to_defaults_confirmed();

        assert_eq!(app.draft, edited_draft);
        assert_ne!(app.draft, app.defaults);
    }

    #[test]
    fn handle_config_saved_success_clears_dirty_and_records_backup() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        app.is_saving = true;
        app.is_dirty = true;
        app.draft.capture_enabled = !app.draft.capture_enabled;
        let backup = PathBuf::from("/tmp/wayscriber-config.bak");
        let (path, document) = temp_config_document("saved", "");

        let _ = app.handle_config_saved(Ok((Some(backup.clone()), document)));

        assert!(!app.is_saving);
        assert!(!app.is_dirty);
        assert_eq!(app.last_backup_path, Some(backup));
        assert_eq!(app.draft, app.baseline);
        assert!(status_contains(
            &app.status,
            "Configuration saved successfully."
        ));
        let _ = std::fs::remove_file(path);
    }
}
