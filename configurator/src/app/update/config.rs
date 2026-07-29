use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use wayscriber::config::{
    Config, ConfigDiagnosticKind, ConfigDocument, ConfigValidationReport, InvalidKeybinding,
    KeybindingConflictResolution, MigrationPreview,
};

use crate::messages::Message;
use crate::models::error::FormError;
use crate::models::{ConfigDraft, KeybindingField};

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
                self.refresh_migration_preview(&document);
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

        // After the status is set, so a note about a startup argument can be
        // added to this file's diagnostics instead of replacing them. This is
        // also the only place a destination is applied: the tabs it chooses
        // are only meaningful once the configuration behind them has loaded.
        self.apply_startup_request()
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

        match self.prepare_config_to_save(&document) {
            Ok(config) => {
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

    /// The configuration a Save writes, with what validating it had to change
    /// in `[keybindings]` kept for the status the completed write reports.
    ///
    /// The draft rebuilds that section from the editor's own fields, so every
    /// list in it is authored and a duplicate the user typed is arbitrated by
    /// traversal order rather than filtered as an unauthored default. The
    /// arbitration edits the configuration on its way to disk, and the saved
    /// file then spells both lists out — leaving nothing for the reloaded
    /// document to rediscover — so this is the only place the loss can be seen.
    fn prepare_config_to_save(
        &mut self,
        document: &ConfigDocument,
    ) -> Result<Config, Vec<FormError>> {
        let mut config = self.draft.to_config(document.config())?;
        self.pending_save_validation = config.validate_and_clamp();
        Ok(config)
    }

    pub(super) fn handle_config_saved(
        &mut self,
        result: Result<(Option<PathBuf>, Arc<ConfigDocument>), String>,
    ) -> Task<Message> {
        self.is_saving = false;
        // Either outcome answers this write; a failed one wrote nothing, so
        // there is no resolution to report for it.
        let validation = std::mem::take(&mut self.pending_save_validation);
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
                // The file just changed, so the offer has to be recomputed
                // against it: an applied migration leaves nothing to propose,
                // and an unrelated save leaves the same proposal standing.
                self.refresh_migration_preview(&saved_document);
                let mut msg = "Configuration saved successfully.".to_string();
                if let Some(path) = backup {
                    msg.push_str(&format!("\nBackup created at {}", path.display()));
                }
                let mut status = config_document_status(&saved_document, &msg);
                if let Some(note) = save_validation_note(&validation) {
                    status = status.with_note(&note);
                }
                self.status = status;
            }
            Err(err) => {
                self.status = StatusMessage::error(format!("Failed to save configuration: {err}"));
            }
        }

        Task::none()
    }

    /// Recomputes what a migration would propose for the document now in hand.
    ///
    /// The authored values are the ones to diff: proposing a change to a
    /// binding that only exists because loading dropped a contested key would
    /// offer the user an edit their file never contained.
    fn refresh_migration_preview(&mut self, document: &ConfigDocument) {
        self.migration_preview = MigrationPreview::for_authored_config(document.authored_config());
    }

    pub(super) fn handle_migration_apply_requested(&mut self) -> Task<Message> {
        if self.is_loading || self.is_saving {
            return Task::none();
        }
        let Some(preview) = self.pending_migration().cloned() else {
            return Task::none();
        };

        let mut applied = 0usize;
        for change in preview.changes() {
            // A key this build has no field for cannot be shown or edited, so
            // it is left alone rather than written blind.
            let Some(field) = KeybindingField::from_field_key(change.config_key()) else {
                continue;
            };
            self.draft.keybindings.set(field, change.after().join(", "));
            applied += 1;
        }
        // Recording the revision is what stops the same proposal from coming
        // back, and it is a draft change in its own right: a proposal whose
        // shortcut text already matches what the draft shows still has to be
        // savable.
        self.draft.config_revision = Some(preview.proposed_revision());
        self.migration_preview = None;
        let label = if applied == 1 {
            "shortcut update"
        } else {
            "shortcut updates"
        };
        self.status = StatusMessage::info(format!(
            "Applied {applied} {label} to the draft. Nothing is written until you press Save."
        ));
        self.refresh_dirty_flag();

        Task::none()
    }

    pub(super) fn handle_migration_dismissed(&mut self) -> Task<Message> {
        // Left silent on purpose: the status banner may be carrying the load
        // diagnostics for this file, and hiding the offer is not worth losing
        // them over.
        self.migration_dismissed = true;

        Task::none()
    }
}

const SHOWN_DIAGNOSTICS: usize = 8;

fn config_document_status(document: &ConfigDocument, success: &str) -> StatusMessage {
    let diagnostics = document.diagnostics();
    if diagnostics.is_empty() {
        return StatusMessage::success(success);
    }

    let mut message = success.to_string();
    let mut unknown = Vec::new();
    let mut conflicts = Vec::new();
    let mut invalid = Vec::new();
    let mut skipped_defaults = Vec::new();
    // Exhaustive on purpose: a kind with no section here would leave the
    // status warning-styled and empty of the very thing it is warning about,
    // so a new variant has to be a compile error rather than a silent drop.
    for diagnostic in diagnostics {
        match diagnostic.kind() {
            ConfigDiagnosticKind::UnknownSetting => unknown.push(diagnostic.path().to_string()),
            // Every keybinding kind is resolved in memory only, so the file
            // the editor is showing still contains them: carry the diagnostic's
            // own wording, which names the actions, instead of just the path.
            ConfigDiagnosticKind::KeybindingConflict => conflicts.push(diagnostic.to_string()),
            ConfigDiagnosticKind::InvalidKeybinding => invalid.push(diagnostic.to_string()),
            ConfigDiagnosticKind::DefaultShortcutSkipped => {
                skipped_defaults.push(diagnostic.to_string());
            }
        }
    }

    if !unknown.is_empty() {
        message.push_str(&format!(
            "\nUnrecognized settings were preserved: {}.",
            list_with_overflow(&borrowed(&unknown), ", ")
        ));
    }
    if !invalid.is_empty() {
        message.push_str(&format!(
            "\nShortcuts that could not be parsed are ignored for the running session; the file still has them: {}.",
            list_with_overflow(&borrowed(&invalid), "; ")
        ));
    }
    if !conflicts.is_empty() {
        message.push_str(&format!(
            "\nConflicting shortcuts were resolved for the running session only; the file still has them: {}.",
            list_with_overflow(&borrowed(&conflicts), "; ")
        ));
    }
    // Its own sentence, and the last one: nothing in the file is wrong here.
    // An action this configuration never mentions was offered a shortcut this
    // build added, and the configuration already spends that key.
    if !skipped_defaults.is_empty() {
        message.push_str(&format!(
            "\nNew default shortcuts stayed inactive because this configuration already uses those keys: {}.",
            list_with_overflow(&borrowed(&skipped_defaults), "; ")
        ));
    }

    StatusMessage::warning(message)
}

/// What validating the saved configuration changed in the shortcuts the user
/// typed, or `None` when it changed nothing.
///
/// The load-time sentences in [`config_document_status`] all end in "the file
/// still has them", because loading resolves in memory only. These are the
/// other case: the draft is the authored text, the resolution is what reached
/// `config.toml`, and the reloaded document no longer contains the collision
/// to report. Naming which action kept the key and which lost it is therefore
/// the only account the user gets of an edit their Save made for them.
///
/// A skipped default cannot appear here: the draft spells every action out
/// (`ConfigDraft::to_config` marks the section explicit), so the omitted-default
/// pass has nothing to offer and reports nothing.
fn save_validation_note(validation: &ConfigValidationReport) -> Option<String> {
    // The summaries, not the full `Display` forms: those say the file keeps
    // the shortcut and the session does without it, which is the load story.
    let invalid = clauses(
        validation
            .invalid_keybindings
            .iter()
            .map(InvalidKeybinding::summary),
    );
    let conflicts = clauses(
        validation
            .keybinding_conflicts
            .iter()
            .map(KeybindingConflictResolution::summary),
    );
    if invalid.is_empty() && conflicts.is_empty() {
        return None;
    }

    let mut note = String::new();
    if !invalid.is_empty() {
        note.push_str(&format!(
            "Shortcuts that could not be parsed were left out of the saved configuration: {}.",
            list_with_overflow(&borrowed(&invalid), "; ")
        ));
    }
    if !conflicts.is_empty() {
        if !note.is_empty() {
            note.push('\n');
        }
        note.push_str(&format!(
            "Shortcuts two actions claimed were settled before saving, and the saved configuration keeps that outcome: {}.",
            list_with_overflow(&borrowed(&conflicts), "; ")
        ));
    }
    Some(note)
}

/// Toast-sized summaries as list items: each is a finished sentence, and the
/// sentence they are listed inside supplies the final stop.
fn clauses(summaries: impl Iterator<Item = String>) -> Vec<String> {
    summaries
        .map(|summary| summary.trim_end_matches('.').to_string())
        .collect()
}

fn borrowed(entries: &[String]) -> Vec<&str> {
    entries.iter().map(String::as_str).collect()
}

fn list_with_overflow(entries: &[&str], separator: &str) -> String {
    let shown = entries
        .iter()
        .take(SHOWN_DIAGNOSTICS)
        .copied()
        .collect::<Vec<_>>()
        .join(separator);
    match entries.len().saturating_sub(SHOWN_DIAGNOSTICS) {
        0 => shown,
        remaining => format!("{shown}{separator}and {remaining} more"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use wayscriber::config::{Action, CURRENT_CONFIG_REVISION};

    use super::*;
    use crate::models::{ColorPickerId, ToggleField};
    use crate::test_temp::TempDir;

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

    /// A resolved shortcut conflict is never written back, so the editor is
    /// where the user has to be able to find it (#293). Both sides here are
    /// spelled out in the file, which is what makes it their conflict.
    #[test]
    fn handle_config_loaded_surfaces_resolved_shortcut_conflicts() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (path, document) = temp_config_document(
            "shortcut-conflict",
            &format!(
                "config_revision = {}\n\n[keybindings]\ntoggle_toolbar = [\"F2\"]\ncycle_toolbar_display = [\"F2\"]\n",
                wayscriber::config::CURRENT_CONFIG_REVISION
            ),
        );

        let _ = app.handle_config_loaded(Ok((document, None)));

        assert!(matches!(app.status, StatusMessage::Warning(_)));
        assert!(status_contains(&app.status, "F2"));
        assert!(status_contains(&app.status, "Toggle Toolbar"));
        assert!(status_contains(&app.status, "Cycle Toolbar Display"));
        assert!(status_contains(&app.status, "running session only"));
        assert!(
            !status_contains(&app.status, "Unrecognized settings"),
            "a conflict is not an unknown setting"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A default this build added and the file never mentions gets its own
    /// sentence: the user's configuration is fine, and the shortcut they read
    /// about in the release notes simply is not theirs (#293).
    #[test]
    fn handle_config_loaded_surfaces_skipped_default_shortcuts() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (path, document) = temp_config_document(
            "skipped-default",
            &format!(
                "config_revision = {}\n\n[keybindings]\ntoggle_toolbar = [\"F2\", \"F9\"]\n",
                wayscriber::config::CURRENT_CONFIG_REVISION
            ),
        );

        let _ = app.handle_config_loaded(Ok((document, None)));

        assert!(matches!(app.status, StatusMessage::Warning(_)));
        assert!(status_contains(&app.status, "F2"));
        assert!(status_contains(&app.status, "Cycle Toolbar Display"));
        assert!(status_contains(
            &app.status,
            "New default shortcuts stayed inactive"
        ));
        assert!(
            !status_contains(&app.status, "Conflicting shortcuts")
                && !status_contains(&app.status, "Unrecognized settings"),
            "a skipped default is neither a conflict nor an unknown setting"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A string the parser rejects is dropped for the session and kept by the
    /// file, so the editor is where the user has to be able to find it. With
    /// nothing else wrong in the file, this section is the entire warning.
    #[test]
    fn handle_config_loaded_surfaces_shortcuts_that_could_not_be_parsed() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (path, document) = temp_config_document(
            "invalid-shortcut",
            &format!(
                "config_revision = {}\n\n[keybindings]\nclear_canvas = [\"Ctrl+Shift\"]\n",
                wayscriber::config::CURRENT_CONFIG_REVISION
            ),
        );

        let _ = app.handle_config_loaded(Ok((document, None)));

        assert!(matches!(app.status, StatusMessage::Warning(_)));
        assert!(status_contains(&app.status, "Ctrl+Shift"));
        assert!(status_contains(&app.status, "Clear Canvas"));
        assert!(status_contains(&app.status, "could not be parsed"));
        assert!(
            !status_contains(&app.status, "Unrecognized settings")
                && !status_contains(&app.status, "Conflicting shortcuts"),
            "an unparseable shortcut is neither an unknown setting nor a conflict"
        );
        let _ = std::fs::remove_file(path);
    }

    /// All three keybinding kinds can land in one file, and each gets its own
    /// sentence: they need different fixes, and one of them needs no fix.
    #[test]
    fn handle_config_loaded_separates_every_keybinding_diagnostic_kind() {
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        let (path, document) = temp_config_document(
            "invalid-and-conflicting",
            &format!(
                "config_revision = {}\n\n[keybindings]\nclear_canvas = [\"Ctrl+Shift\"]\ntoggle_toolbar = [\"F2\", \"F9\"]\nundo = [\"Ctrl+Alt+U\"]\nredo = [\"Ctrl+Alt+U\"]\n",
                wayscriber::config::CURRENT_CONFIG_REVISION
            ),
        );

        let _ = app.handle_config_loaded(Ok((document, None)));

        assert!(matches!(app.status, StatusMessage::Warning(_)));
        assert!(status_contains(&app.status, "could not be parsed"));
        assert!(status_contains(&app.status, "running session only"));
        assert!(status_contains(
            &app.status,
            "New default shortcuts stayed inactive"
        ));
        assert!(status_contains(&app.status, "Ctrl+Shift"));
        assert!(status_contains(&app.status, "Ctrl+Alt+U"));
        assert!(status_contains(&app.status, "F2"));
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

    /// The reviewer's case: the file spells `undo` out and never mentions
    /// `clear_canvas`, and the user then types `undo`'s shortcut into the
    /// Clear canvas field. The draft is the authored text now, so both lists
    /// are explicit and the traversal order settles the collision (core visits
    /// `clear_canvas` before `undo`). Classifying the typed binding as an
    /// omitted default instead would filter it away, save an empty list, and
    /// report success.
    #[test]
    fn a_shortcut_typed_for_an_omitted_action_is_arbitrated_not_filtered() {
        let (mut app, _dir, path) = app_with_config_file(&format!(
            "config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\nundo = [\"Ctrl+Alt+U\"]\n"
        ));
        app.draft
            .keybindings
            .set(KeybindingField::ClearCanvas, "Ctrl+Alt+U".to_string());

        let document = app.base_document.clone().expect("a loaded document");
        let mut config = app
            .draft
            .to_config(document.config())
            .expect("the draft converts to a config");
        let report = config.validate_and_clamp();

        assert!(
            report.skipped_default_shortcuts.is_empty(),
            "the user typed this binding; it is not an offer to filter: {:?}",
            report.skipped_default_shortcuts
        );
        assert_eq!(
            config.keybindings.core.clear_canvas,
            ["Ctrl+Alt+U"],
            "the earlier action in traversal order keeps the key"
        );
        assert!(config.keybindings.core.undo.is_empty());
        assert_eq!(report.keybinding_conflicts.len(), 1);
        assert_eq!(report.keybinding_conflicts[0].kept(), Action::ClearCanvas);
        assert_eq!(report.keybinding_conflicts[0].dropped(), Action::Undo);

        let _ = save_draft(&mut app);

        assert!(
            matches!(app.status, StatusMessage::Warning(_)),
            "a binding the save took away is not a plain success: {:?}",
            app.status
        );
        assert!(status_contains(&app.status, "settled before saving"));
        assert!(
            status_contains(
                &app.status,
                "Ctrl+Alt+U kept for Clear Canvas, dropped from Undo."
            ),
            "the status has to name the key, the winner, and the loser: {:?}",
            app.status
        );

        let contents = read_config(&path);
        assert_eq!(
            config_setting(&contents, "clear_canvas").as_deref(),
            Some("clear_canvas = [\"Ctrl+Alt+U\"]"),
            "the typed binding reaches the file"
        );
        assert_eq!(
            config_setting(&contents, "undo").as_deref(),
            Some("undo = []"),
            "the loser is written out too, so the file and the report agree"
        );
    }

    /// The same collision the other way around: nothing about the draft is
    /// wrong, so a save that resolves nothing says nothing extra.
    #[test]
    fn a_save_without_shortcut_trouble_stays_a_plain_success() {
        let (mut app, _dir, _path) = app_with_config_file(&format!(
            "config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\nundo = [\"Ctrl+Alt+U\"]\n"
        ));
        app.draft.drawing_default_thickness = "6".to_string();

        let _ = save_draft(&mut app);

        assert!(
            matches!(app.status, StatusMessage::Success(_)),
            "unexpected status: {:?}",
            app.status
        );
        assert!(!status_contains(&app.status, "settled before saving"));
    }

    /// A shortcut the editor accepts as text but the parser rejects never
    /// reaches the file either, so the save status is the only place it can be
    /// reported.
    #[test]
    fn a_typed_shortcut_the_parser_rejects_is_reported_by_the_save() {
        let (mut app, _dir, _path) = app_with_config_file(&format!(
            "config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\nundo = [\"Ctrl+Alt+U\"]\n"
        ));
        app.draft
            .keybindings
            .set(KeybindingField::ClearCanvas, "Ctrl+Shift".to_string());

        let _ = save_draft(&mut app);

        assert!(
            matches!(app.status, StatusMessage::Warning(_)),
            "unexpected status: {:?}",
            app.status
        );
        assert!(status_contains(&app.status, "Ctrl+Shift"));
        assert!(status_contains(&app.status, "Clear Canvas"));
        assert!(status_contains(&app.status, "could not be parsed"));
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

    const LEGACY_REVISION_ZERO_CONFIG: &str = "config_revision = 0\n\n[drawing]\ndefault_thickness = 3.0\n\n[keybindings]\ntoggle_command_palette = [\"Ctrl+K\"]\ncapture_full_screen = [\"Ctrl+Shift+P\"]\n";

    /// A config file of its own, in a directory the test owns: an applied
    /// migration saves, and a save drops its `.bak` next to the file.
    fn app_with_config_file(contents: &str) -> (ConfiguratorApp, TempDir, PathBuf) {
        let dir = crate::test_temp::tempdir().expect("temporary test directory");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, contents).expect("write test config");
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        load_config_file(&mut app, &path);
        (app, dir, path)
    }

    fn load_config_file(app: &mut ConfiguratorApp, path: &Path) {
        let document = ConfigDocument::load_from_path(path).expect("load test config document");
        let _ = app.handle_config_loaded(Ok((Arc::new(document), None)));
    }

    /// The Save path with the executor left out: `handle_save_requested`
    /// builds exactly this config, and `save_config_to_disk` performs exactly
    /// this write before handing the outcome back to `handle_config_saved`.
    fn save_draft(app: &mut ConfiguratorApp) -> Option<PathBuf> {
        let document = app.base_document.clone().expect("a loaded document");
        let config = app
            .prepare_config_to_save(&document)
            .expect("the draft converts to a config");
        let (saved, backup) = document
            .save_with_backup(config)
            .expect("the document saves")
            .into_parts();
        let _ = app.handle_config_saved(Ok((backup.clone(), Arc::new(saved))));
        backup
    }

    /// One setting exactly as the saved file spells it, with the line wrapping
    /// the merge may choose for an array folded into single spaces.
    fn config_setting(contents: &str, key: &str) -> Option<String> {
        let mut lines = contents.lines().map(str::trim).skip_while(|line| {
            !(line.starts_with(key) && line[key.len()..].trim_start().starts_with('='))
        });
        let mut setting = lines.next()?.to_string();
        while setting.matches('[').count() > setting.matches(']').count() {
            let Some(continuation) = lines.next() else {
                break;
            };
            setting.push(' ');
            setting.push_str(continuation);
        }
        Some(setting.split_whitespace().collect::<Vec<_>>().join(" "))
    }

    fn read_config(path: &Path) -> String {
        std::fs::read_to_string(path).expect("read the saved config")
    }

    /// The whole point of the review flow: an old file that the user never
    /// migrated keeps both its shortcuts and its revision, however much else
    /// they save.
    #[test]
    fn saving_an_unrelated_field_leaves_old_bindings_and_revision_alone() {
        let (mut app, _dir, path) = app_with_config_file(LEGACY_REVISION_ZERO_CONFIG);
        assert!(app.pending_migration().is_some());

        app.draft.drawing_default_thickness = "6".to_string();
        let _ = save_draft(&mut app);

        let contents = read_config(&path);
        assert_eq!(
            config_setting(&contents, "toggle_command_palette").as_deref(),
            Some("toggle_command_palette = [\"Ctrl+K\"]")
        );
        assert_eq!(
            config_setting(&contents, "capture_full_screen").as_deref(),
            Some("capture_full_screen = [\"Ctrl+Shift+P\"]")
        );
        assert_eq!(
            config_setting(&contents, "config_revision").as_deref(),
            Some("config_revision = 0")
        );
        assert_eq!(
            config_setting(&contents, "default_thickness").as_deref(),
            Some("default_thickness = 6.0"),
            "the field the user did edit still saves"
        );
        assert!(
            app.pending_migration().is_some(),
            "an unrelated save does not answer the migration question"
        );
    }

    #[test]
    fn applying_and_saving_writes_the_reviewed_fields_the_revision_and_a_backup() {
        let (mut app, _dir, path) = app_with_config_file(LEGACY_REVISION_ZERO_CONFIG);

        let _ = app.handle_migration_apply_requested();

        assert!(app.is_dirty);
        assert!(
            app.pending_migration().is_none(),
            "the offer is answered once it is applied"
        );
        assert_eq!(
            app.draft
                .keybindings
                .value_for(KeybindingField::ToggleCommandPalette),
            Some("Ctrl+K, Ctrl+Shift+P")
        );
        assert_eq!(app.draft.config_revision, Some(CURRENT_CONFIG_REVISION));

        let backup = save_draft(&mut app).expect("the save creates a backup");
        assert_eq!(
            std::fs::read_to_string(&backup).expect("read the backup"),
            LEGACY_REVISION_ZERO_CONFIG,
            "the backup holds the file as it was before the migration"
        );

        let contents = read_config(&path);
        assert_eq!(
            config_setting(&contents, "toggle_command_palette").as_deref(),
            Some("toggle_command_palette = [\"Ctrl+K\", \"Ctrl+Shift+P\"]")
        );
        assert_eq!(
            config_setting(&contents, "capture_full_screen").as_deref(),
            Some("capture_full_screen = [\"Ctrl+Alt+F\"]")
        );
        assert_eq!(
            config_setting(&contents, "config_revision"),
            Some(format!("config_revision = {CURRENT_CONFIG_REVISION}"))
        );
        assert_eq!(
            config_setting(&contents, "default_thickness").as_deref(),
            Some("default_thickness = 3.0"),
            "an applied migration is a keybinding delta, not a rewrite"
        );

        load_config_file(&mut app, &path);
        assert!(
            app.pending_migration().is_none(),
            "the reloaded file is current, so there is nothing left to offer"
        );
    }

    /// Dismissing answers the question for this app run. A reload recomputes
    /// the preview, but the user already said no to this file.
    #[test]
    fn dismissing_hides_the_offer_and_keeps_it_out_of_an_unrelated_save() {
        let (mut app, _dir, path) = app_with_config_file(
            "[keybindings]\ntoggle_command_palette = [\"Ctrl+K\"]\ncapture_full_screen = [\"Ctrl+Shift+P\"]\n",
        );
        assert!(app.pending_migration().is_some());

        let _ = app.handle_migration_dismissed();

        assert!(app.pending_migration().is_none());
        assert!(!app.is_dirty, "dismissing changes nothing in the draft");

        app.draft.drawing_default_thickness = "6".to_string();
        let _ = save_draft(&mut app);

        let contents = read_config(&path);
        assert_eq!(
            config_setting(&contents, "toggle_command_palette").as_deref(),
            Some("toggle_command_palette = [\"Ctrl+K\"]")
        );
        assert_eq!(
            config_setting(&contents, "capture_full_screen").as_deref(),
            Some("capture_full_screen = [\"Ctrl+Shift+P\"]")
        );
        assert_eq!(
            config_setting(&contents, "config_revision"),
            None,
            "a file that never recorded a revision is not stamped by an unrelated save"
        );
        assert!(app.pending_migration().is_none());

        load_config_file(&mut app, &path);
        assert!(
            app.pending_migration().is_none(),
            "pressing Reload is not the user asking again"
        );
    }

    #[test]
    fn a_current_revision_file_never_offers_a_migration() {
        let (app, _dir, _path) = app_with_config_file(&format!(
            "config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\ntoggle_command_palette = [\"Ctrl+K\"]\n"
        ));

        assert!(app.pending_migration().is_none());
    }

    /// An empty file spells no shortcut out, so every recipe declines and
    /// there is nothing to review. A missing file is the same answer from the
    /// other direction: the document is this build's defaults.
    #[test]
    fn an_empty_or_missing_file_never_offers_a_migration() {
        let (app, _dir, _path) = app_with_config_file("");
        assert!(app.pending_migration().is_none());

        let dir = crate::test_temp::tempdir().expect("temporary test directory");
        let (mut app, _cmd) = ConfiguratorApp::new_app();
        load_config_file(&mut app, &dir.path().join("missing.toml"));

        assert!(app.pending_migration().is_none());
    }

    /// The input-HUD step proposes unbinding a default the file never spelled
    /// out, and loading already dropped that default from the effective
    /// keymap — so the draft text does not change at all. The revision is
    /// what makes the applied migration savable.
    #[test]
    fn applying_marks_the_draft_dirty_when_only_the_revision_changes() {
        let (mut app, _dir, path) = app_with_config_file(
            "config_revision = 2\n\n[keybindings]\ncapture_clipboard_full = [\"Ctrl+Shift+K\"]\n",
        );
        let text_before = app
            .draft
            .keybindings
            .value_for(KeybindingField::ToggleInputHud)
            .map(str::to_string);

        let _ = app.handle_migration_apply_requested();

        assert_eq!(
            app.draft
                .keybindings
                .value_for(KeybindingField::ToggleInputHud)
                .map(str::to_string),
            text_before,
            "loading already resolved this binding away, so the text is unchanged"
        );
        assert!(app.is_dirty, "the proposed revision is a draft change");

        let _ = save_draft(&mut app);

        let contents = read_config(&path);
        assert_eq!(
            config_setting(&contents, "config_revision"),
            Some(format!("config_revision = {CURRENT_CONFIG_REVISION}"))
        );
        assert_eq!(
            config_setting(&contents, "capture_clipboard_full").as_deref(),
            Some("capture_clipboard_full = [\"Ctrl+Shift+K\"]"),
            "the authored binding the migration protects is left alone"
        );

        load_config_file(&mut app, &path);
        assert!(app.pending_migration().is_none());
    }
}
