use wayscriber::config::ConfigDocument;

use super::super::super::effects::Effect;
use super::super::super::state::{ConfiguratorApp, StatusMessage};
use super::status::list_with_overflow;

impl ConfiguratorApp {
    /// Recomputes what a migration would propose for the document now in hand.
    ///
    /// The authored values are the ones to diff: proposing a change to a
    /// binding that only exists because loading dropped a contested key would
    /// offer the user an edit their file never contained.
    ///
    /// A dismissal answers the question for one file, so it survives a reload
    /// of that same file and no other. With `config.toml` a link into one
    /// profile among several, retargeting it and pressing Reload brings up a
    /// configuration the user has never been asked about; keeping the earlier
    /// answer would hide its offer until the app is restarted. The document's
    /// destination is what tells the two apart — the path is the same either
    /// way.
    pub(super) fn refresh_migration_preview(&mut self, document: &ConfigDocument) {
        self.migration.refresh(document);
    }

    pub(in crate::app::update) fn handle_migration_apply_requested(&mut self) -> Vec<Effect> {
        if self.document.is_loading() || self.document.is_saving() {
            return Vec::new();
        }
        let Some(result) = self.migration.apply(&mut self.draft) else {
            return Vec::new();
        };
        let applied = result.changed;
        let kept = result.kept;
        let label = if applied == 1 {
            "shortcut update"
        } else {
            "shortcut updates"
        };
        let mut message = format!("Applied {applied} {label} to the draft.");
        if !kept.is_empty() {
            message.push_str(&format!(
                " Kept your edit to {}.",
                list_with_overflow(&kept, ", ")
            ));
        }
        message.push_str(" Nothing is written until you press Save.");
        self.status = StatusMessage::info(message);
        self.refresh_dirty_flag();

        Vec::new()
    }

    pub(in crate::app::update) fn handle_migration_dismissed(&mut self) -> Vec<Effect> {
        // Left silent on purpose: the status banner may be carrying the load
        // diagnostics for this file, and hiding the offer is not worth losing
        // them over.
        //
        // Recorded against the file the offer was about, not the path that
        // reached it: only a reload landing on that same file is the reload
        // this answer covers. Without a document in hand there is no file to
        // name — no load has produced one, or a running save is holding it —
        // and an answer already given stands rather than being cleared.
        self.migration.dismiss(self.document.loaded());

        Vec::new()
    }
}
