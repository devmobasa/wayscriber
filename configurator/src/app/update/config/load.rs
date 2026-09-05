use wayscriber::config::ConfigDocument;

use crate::models::ConfigDraft;

use super::super::super::effects::Effect;
use super::super::super::state::{ConfiguratorApp, StatusMessage};
use super::status::config_document_status;

impl ConfiguratorApp {
    pub(in crate::app::update) fn handle_config_loaded(
        &mut self,
        result: Result<(Box<ConfigDocument>, Option<String>), String>,
    ) -> Vec<Effect> {
        match result {
            Ok((document, repair_warning)) => {
                let draft = ConfigDraft::from_config(document.config());
                self.draft = draft.clone();
                self.baseline = draft;
                self.override_mode = self.draft.ui_toolbar_layout_mode;
                self.boards_collapsed = vec![false; self.draft.boards.items.len()];
                self.color_picker_hex.clear();
                self.sync_all_color_picker_hex();
                self.is_dirty = false;
                self.clear_defaults_confirmation();
                self.clear_shortcut_editing();
                self.refresh_migration_preview(&document);
                self.status = repair_warning.map_or_else(
                    || config_document_status(&document, "Configuration loaded from disk."),
                    |warning| {
                        StatusMessage::warning(format!(
                            "The configuration could not be parsed, so built-in defaults were loaded for repair. Saving will create a backup before replacing the unreadable configuration with this draft. Unknown settings are retained only when the TOML structure is parseable and they can be separated safely; malformed TOML content remains only in the backup.\n{warning}"
                        ))
                    },
                );
                // Last, so everything above reads the document by reference and
                // the model takes ownership of exactly one copy.
                self.document.finish_load(Some(*document));
            }
            Err(err) => {
                self.document.finish_load(None);
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

    pub(in crate::app::update) fn handle_reload_requested(&mut self) -> Vec<Effect> {
        if self.document.begin_reload() {
            self.clear_defaults_confirmation();
            self.status = StatusMessage::info("Reloading configuration...");
            return vec![Effect::LoadConfig];
        }

        Vec::new()
    }
}
