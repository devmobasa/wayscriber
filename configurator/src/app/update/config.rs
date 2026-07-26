use iced::Task;
use wayscriber::config::ConfigDocument;

use crate::messages::Message;
use crate::models::ConfigDraft;

use super::super::blocking_jobs::BlockingJobRequest;
use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_config_loaded(
        &mut self,
        result: Result<(Box<ConfigDocument>, Option<String>), String>,
    ) -> Task<Message> {
        match result {
            Ok((document, repair_warning)) => {
                let draft = ConfigDraft::from_config(document.config());
                let status = repair_warning.map_or_else(
                    || config_document_status(&document, "Configuration loaded from disk."),
                    |warning| {
                        StatusMessage::warning(format!(
                            "The configuration could not be parsed, so built-in defaults were loaded for repair. Saving will create a backup before replacing the unreadable configuration with this draft. Unknown settings are retained only when the TOML structure is parseable and they can be separated safely; malformed TOML content remains only in the backup.\n{warning}"
                        ))
                    },
                );
                if self.base_document.finish_load(Some(document)).is_err() {
                    return Task::none();
                }
                self.draft = draft.clone();
                self.baseline = draft;
                self.override_mode = self.draft.ui_toolbar_layout_mode;
                self.boards_collapsed = vec![false; self.draft.boards.items.len()];
                self.color_picker_open = None;
                self.color_picker_advanced.clear();
                self.color_picker_hex.clear();
                self.sync_all_color_picker_hex();
                self.is_dirty = false;
                self.defaults_reset_pending = false;
                self.status = status;
            }
            Err(err) => {
                if self.base_document.finish_load(None).is_err() {
                    return Task::none();
                }
                self.status =
                    StatusMessage::error(format!("Failed to load config from disk: {err}"));
            }
        }

        self.handle_startup_search_focus_config_fallback()
    }

    pub(super) fn handle_reload_requested(&mut self) -> Task<Message> {
        if self.base_document.begin_load() {
            self.defaults_reset_pending = false;
            self.status = StatusMessage::info("Reloading configuration...");
            return self.submit_blocking_job(BlockingJobRequest::ConfigLoad);
        }

        Task::none()
    }

    pub(super) fn handle_reset_to_defaults_requested(&mut self) -> Task<Message> {
        if !self.base_document.is_loading() && !self.base_document.is_saving() {
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
        if self.defaults_reset_pending
            && !self.base_document.is_loading()
            && !self.base_document.is_saving()
        {
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
        if self.base_document.is_loading() || self.base_document.is_saving() {
            return Task::none();
        }
        self.defaults_reset_pending = false;
        let Some(document) = self.base_document.document() else {
            self.status = StatusMessage::error(
                "Configuration has not loaded successfully. Reload before saving.",
            );
            return Task::none();
        };

        match self.draft.to_config(document.config(), &self.path_resolver) {
            Ok(mut config) => {
                config.validate_and_clamp();
                let Some(document) = self.base_document.begin_save(self.draft.clone()) else {
                    self.status = StatusMessage::error(
                        "Configuration is no longer ready to save. Reload before trying again.",
                    );
                    return Task::none();
                };
                self.status = StatusMessage::info("Saving configuration...");
                self.submit_blocking_job(BlockingJobRequest::ConfigSave {
                    document,
                    config: Box::new(config),
                })
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
        document: Box<ConfigDocument>,
        outcome: Result<Option<std::path::PathBuf>, String>,
    ) -> Task<Message> {
        match outcome {
            Ok(backup) => {
                let saved_draft = ConfigDraft::from_config(document.config());
                let mut msg = "Configuration saved successfully.".to_string();
                if let Some(path) = &backup {
                    msg.push_str(&format!("\nBackup created at {}", path.display()));
                }
                let status = config_document_status(&document, &msg);
                let submitted_draft = match self.base_document.finish_save(document) {
                    Ok(submitted_draft) => submitted_draft,
                    Err(_) => return Task::none(),
                };
                let has_newer_edits = self.draft != submitted_draft;
                self.last_backup_path = backup.clone();
                self.baseline = saved_draft.clone();
                if has_newer_edits {
                    self.is_dirty = self.draft != self.baseline;
                } else {
                    self.draft = saved_draft;
                    self.boards_collapsed = vec![false; self.draft.boards.items.len()];
                    self.color_picker_open = None;
                    self.color_picker_advanced.clear();
                    self.color_picker_hex.clear();
                    self.sync_all_color_picker_hex();
                    self.is_dirty = false;
                }
                self.defaults_reset_pending = false;
                self.status = if has_newer_edits {
                    append_status_detail(status, "Newer edits remain unsaved.")
                } else {
                    status
                };
            }
            Err(err) => {
                if self.base_document.finish_save(document).is_err() {
                    return Task::none();
                }
                self.is_dirty = self.draft != self.baseline;
                self.status = StatusMessage::error(format!("Failed to save configuration: {err}"));
            }
        }

        Task::none()
    }
}

fn append_status_detail(status: StatusMessage, detail: &str) -> StatusMessage {
    match status {
        StatusMessage::Success(message) => StatusMessage::success(format!("{message}\n{detail}")),
        StatusMessage::Warning(message) => StatusMessage::warning(format!("{message}\n{detail}")),
        other => other,
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

    use super::*;
    use crate::models::{ColorPickerId, ToggleField};
    use crate::test_temp::{TempDir, tempdir};

    fn status_contains(status: &StatusMessage, needle: &str) -> bool {
        match status {
            StatusMessage::Info(text)
            | StatusMessage::Success(text)
            | StatusMessage::Error(text)
            | StatusMessage::Warning(text) => text.contains(needle),
            StatusMessage::Idle => false,
        }
    }

    fn temp_config_document(name: &str, contents: &str) -> (TempDir, Box<ConfigDocument>) {
        let temp = tempdir().expect("the owned temporary-directory fixture should be created");
        let path = temp.path().join(format!("{name}.toml"));
        std::fs::write(&path, contents)
            .expect("the fixture path is inside the owned temporary directory");
        let document = ConfigDocument::load_from_path(&path)
            .expect("the just-written TOML fixture should parse");
        (temp, Box::new(document))
    }

    #[test]
    fn handle_config_loaded_success_resets_loading_and_dirty_state() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        app.color_picker_open = Some(ColorPickerId::StatusBarBg);
        app.is_dirty = true;

        let (_temp, document) = temp_config_document("loaded", "");
        let _ = app.handle_config_loaded(Ok((document, None)));

        assert!(!app.base_document.is_loading());
        assert!(!app.is_dirty);
        assert!(app.color_picker_open.is_none());
        assert_eq!(app.boards_collapsed.len(), app.draft.boards.items.len());
        assert!(status_contains(
            &app.status,
            "Configuration loaded from disk."
        ));
    }

    #[test]
    fn handle_config_loaded_uses_startup_search_focus_fallback_once() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();

        let (_first_temp, first) = temp_config_document("focus-first", "");
        let _ = app.handle_config_loaded(Ok((first, None)));

        assert!(app.search_input_focus_hint);
        assert!(!app.startup_search_focus_pending);

        app.search_input_focus_hint = false;
        assert!(app.base_document.begin_load());
        let (_second_temp, second) = temp_config_document("focus-second", "");
        let _ = app.handle_config_loaded(Ok((second, None)));

        assert!(!app.search_input_focus_hint);
    }

    #[test]
    fn handle_config_loaded_error_preserves_the_last_good_document_and_draft() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (_temp, document) = temp_config_document("before-reload-error", "");
        let expected_path = document.source_path().to_path_buf();
        let _ = app.handle_config_loaded(Ok((document, None)));
        app.draft.capture_enabled = !app.draft.capture_enabled;
        let draft = app.draft.clone();

        assert!(app.base_document.begin_load());
        let _ = app.handle_config_loaded(Err("broken".to_string()));

        assert!(!app.base_document.is_loading());
        assert_eq!(
            app.base_document.source_path(),
            Some(expected_path.as_path())
        );
        assert_eq!(app.draft, draft);
        assert!(status_contains(
            &app.status,
            "Failed to load config from disk: broken"
        ));
    }

    #[test]
    fn handle_config_loaded_repair_document_allows_saving() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (_temp, document) = temp_config_document("repair", "");

        let _ = app.handle_config_loaded(Ok((
            document,
            Some("invalid type: string, expected u32".to_string()),
        )));

        assert!(app.base_document.document().is_some());
        assert!(matches!(app.status, StatusMessage::Warning(_)));
        assert!(status_contains(&app.status, "loaded for repair"));
        assert!(status_contains(
            &app.status,
            "malformed TOML content remains only in the backup"
        ));
        let _ = app.handle_save_requested();
        assert!(app.base_document.is_saving());
    }

    #[test]
    fn handle_config_loaded_surfaces_preserved_unknown_settings() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (_temp, document) =
            temp_config_document("unknown", "future_configurator_option = true\n");

        let _ = app.handle_config_loaded(Ok((document, None)));

        assert!(matches!(app.status, StatusMessage::Warning(_)));
        assert!(status_contains(&app.status, "future_configurator_option"));
        assert!(status_contains(&app.status, "were preserved"));
    }

    #[test]
    fn handle_save_requested_blocks_without_loaded_document() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let _ = app.handle_config_loaded(Err("missing".to_string()));

        let _ = app.handle_save_requested();

        assert!(!app.base_document.is_saving());
        assert!(status_contains(
            &app.status,
            "Configuration has not loaded successfully"
        ));
    }

    #[test]
    fn handle_save_requested_sets_saving_for_valid_draft() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (_temp, document) = temp_config_document("save-request", "");
        let _ = app.handle_config_loaded(Ok((document, None)));

        let _ = app.handle_save_requested();

        assert!(app.base_document.is_saving());
        assert!(status_contains(&app.status, "Saving configuration..."));
    }

    #[test]
    fn reset_to_defaults_requires_confirmation() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (_temp, document) = temp_config_document("reset-defaults", "");
        let _ = app.handle_config_loaded(Ok((document, None)));
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
        let (_temp, document) = temp_config_document("cancel-defaults", "");
        let _ = app.handle_config_loaded(Ok((document, None)));

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
        let (_source_temp, source_document) = temp_config_document("before-save", "");
        let _ = app.handle_config_loaded(Ok((source_document, None)));
        app.is_dirty = true;
        app.draft.capture_enabled = !app.draft.capture_enabled;
        let _ = app.handle_save_requested();
        let backup = PathBuf::from("/tmp/wayscriber-config.bak");
        let (_temp, document) = temp_config_document("saved", "");

        let _ = app.handle_config_saved(document, Ok(Some(backup.clone())));

        assert!(!app.base_document.is_saving());
        assert!(!app.is_dirty);
        assert_eq!(app.last_backup_path, Some(backup));
        assert_eq!(app.draft, app.baseline);
        assert!(status_contains(
            &app.status,
            "Configuration saved successfully."
        ));
    }

    #[test]
    fn handle_config_saved_preserves_edits_made_after_submission() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (_source_temp, source_document) = temp_config_document("edit-during-save", "");
        let _ = app.handle_config_loaded(Ok((source_document, None)));
        app.draft.capture_enabled = false;
        let submitted_draft = app.draft.clone();
        let _ = app.handle_save_requested();

        app.draft.capture_copy_to_clipboard = false;
        let newer_draft = app.draft.clone();
        let (_saved_temp, saved_document) =
            temp_config_document("saved-edit-during-save", "[capture]\nenabled = false\n");
        let _ = app.handle_config_saved(saved_document, Ok(None));

        assert_eq!(app.draft, newer_draft);
        assert_ne!(app.draft, submitted_draft);
        assert!(app.is_dirty);
        assert!(status_contains(&app.status, "Newer edits remain unsaved"));
    }

    #[test]
    fn save_request_is_rejected_while_reload_is_in_flight() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (_temp, document) = temp_config_document("reload-blocks-save", "");
        let _ = app.handle_config_loaded(Ok((document, None)));
        let _ = app.handle_reload_requested();

        let _ = app.handle_save_requested();

        assert!(app.base_document.is_loading());
        assert!(!app.base_document.is_saving());
    }
}
