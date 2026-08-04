use super::*;

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
        if self.migration_dismissed.as_deref() != Some(document.destination()) {
            self.migration_dismissed = None;
        }
        self.migration_preview = MigrationPreview::for_authored_config(document.authored_config());
    }

    pub(in crate::app::update) fn handle_migration_apply_requested(&mut self) -> Vec<Effect> {
        if self.is_loading || self.is_saving {
            return Vec::new();
        }
        let Some(preview) = self.pending_migration().cloned() else {
            return Vec::new();
        };

        let mut applied = 0usize;
        let mut kept = Vec::new();
        for change in preview.changes() {
            // A key this build has no field for cannot be shown or edited, so
            // it is left alone rather than written blind.
            let Some(field) = KeybindingField::from_field_key(change.config_key()) else {
                continue;
            };
            // The preview was computed when the file loaded; the draft has been
            // editable ever since. A field that no longer reads as the "before"
            // the proposal was built from is the user's own edit, and applying
            // the proposal's "after" over it would silently discard what they
            // typed — so it is kept and reported instead.
            if self.draft.keybindings.parses_to(field, change.before()) {
                self.draft.keybindings.set(field, change.after().join(", "));
                applied += 1;
            } else if !self.draft.keybindings.parses_to(field, change.after()) {
                kept.push(change.action_label());
            }
        }
        // Apply answers the migration question even when the user's own edits
        // cover every proposed field. Those edits are kept above; recording the
        // revision says this generation was reviewed, not that every shipped
        // default was copied verbatim. Without the stamp, customized fields make
        // the recipes decline on the next load anyway, leaving an old revision
        // while the status incorrectly promises the offer will return.
        self.draft.config_revision = Some(preview.proposed_revision());
        self.migration_preview = None;
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
        if let Some(document) = self.base_document.as_ref() {
            self.migration_dismissed = Some(document.destination().to_path_buf());
        }

        Vec::new()
    }
}
