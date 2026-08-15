use wayscriber::config::{Config, ConfigDocument};

use crate::messages::ConfigSaveResult;
use crate::models::ConfigDraft;
use crate::models::error::FormError;

use super::super::super::effects::Effect;
use super::super::super::state::{ConfiguratorApp, StatusMessage};
use super::status::{config_document_status, invalid_color_hex_message, save_validation_note};

impl ConfiguratorApp {
    pub(in crate::app::update) fn handle_save_requested(&mut self) -> Vec<Effect> {
        // `is_loading` counts too: a reload replaces the draft and base
        // document when it lands, so a save started underneath it would write
        // the pre-reload draft and then be judged against a document it never
        // saw — leaving stale fields marked clean and the next save rejected.
        if self.is_saving || self.is_loading {
            return Vec::new();
        }
        self.clear_defaults_confirmation();

        // Before the document moves anywhere: a hex field the parser rejects
        // was never applied to the draft, so saving would write the last value
        // that did parse and the reload would wipe the text the user is still
        // fixing.
        let invalid_hex = self.invalid_color_hex_count();
        if invalid_hex > 0 {
            self.status = StatusMessage::error(invalid_color_hex_message(invalid_hex));
            return Vec::new();
        }

        if self.pending_shortcut_conflict.is_some() {
            self.status = StatusMessage::error("Resolve the shortcut conflict before saving.");
            return Vec::new();
        }

        // The write needs the document itself, so the model gives up its only
        // copy here and gets one back from `handle_config_saved` either way.
        // Taking it is also the "nothing loaded" check: there is one `Option`
        // to read, and reading it is what moves the value.
        let Some(document) = self.base_document.take() else {
            self.status = StatusMessage::error(
                "Configuration has not loaded successfully. Reload before saving.",
            );
            return Vec::new();
        };

        match self.prepare_config_to_save(&document) {
            Ok(config) => {
                self.is_saving = true;
                self.status = StatusMessage::info("Saving configuration...");
                vec![Effect::SaveConfig {
                    document: Box::new(document),
                    config: Box::new(config),
                }]
            }
            Err(errors) => {
                // No write starts, so the document goes straight back: this
                // handler must not be a way to lose it.
                self.base_document = Some(document);
                let message = errors
                    .into_iter()
                    .map(|err| format!("{}: {}", err.field, err.message))
                    .collect::<Vec<_>>()
                    .join("\n");
                self.status = StatusMessage::error(format!(
                    "Cannot save due to validation errors:\n{message}"
                ));
                Vec::new()
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
        let before_clamp = config.clone();
        self.pending_save_validation = config.validate_and_clamp();
        if save_clamped_non_keybinding_fields(&before_clamp, &config) {
            self.pending_save_validation = Default::default();
            return Err(vec![FormError::new(
                "config",
                "Some values are outside their allowed ranges and would be changed on save. Fix them before saving.",
            )]);
        }
        Ok(config)
    }

    pub(in crate::app::update) fn handle_config_saved(
        &mut self,
        result: ConfigSaveResult,
    ) -> Vec<Effect> {
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
                self.boards_collapsed = vec![false; self.draft.boards.items.len()];
                self.color_picker_hex.clear();
                self.sync_all_color_picker_hex();
                self.is_dirty = false;
                self.clear_defaults_confirmation();
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
                self.base_document = Some(*saved_document);
            }
            Err((document, err)) => {
                // The write borrowed the model's only document; a failure hands
                // it straight back so the draft stays savable. The one case
                // with nothing to hand back is a blocking job that never
                // returned, which leaves a reload as the way forward.
                let restored = document.is_some();
                self.base_document = document.map(|document| *document);
                let mut message = format!("Failed to save configuration: {err}");
                if !restored {
                    message.push_str(
                        "\nThe loaded configuration did not come back from the failed write. Reload before saving again.",
                    );
                }
                self.status = StatusMessage::error(message);
            }
        }

        Vec::new()
    }
}

fn save_clamped_non_keybinding_fields(before: &Config, after: &Config) -> bool {
    let mut before = before.clone();
    before.keybindings = after.keybindings.clone();
    format!("{before:?}") != format!("{after:?}")
}
