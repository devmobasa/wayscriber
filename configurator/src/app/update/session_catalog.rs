use std::path::PathBuf;

use crate::models::{SessionCatalogActionResult, SessionCatalogItem, SessionCatalogOperation};

use super::super::effects::Effect;
use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_session_catalog_loaded(
        &mut self,
        result: Result<Vec<SessionCatalogItem>, String>,
    ) -> Vec<Effect> {
        match result {
            Ok(items) => {
                self.session_catalog.replace_items(items);
                if matches!(self.status, StatusMessage::Info(_))
                    && self
                        .status_text()
                        .is_some_and(|message| message.contains("Loading sessions"))
                {
                    self.status = StatusMessage::idle();
                }
            }
            Err(err) => {
                self.session_catalog.is_loading = false;
                self.session_catalog.busy = false;
                self.status =
                    StatusMessage::error(format!("Failed to load session catalog: {err}"));
            }
        }
        Vec::new()
    }

    pub(super) fn handle_session_catalog_refresh_requested(&mut self) -> Vec<Effect> {
        if self.session_catalog.busy {
            return Vec::new();
        }
        self.session_catalog.is_loading = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Loading sessions...");
        vec![Effect::LoadSessionCatalog]
    }

    pub(super) fn handle_session_catalog_forget_requested(&mut self, id: String) -> Vec<Effect> {
        if self.session_catalog.busy {
            return Vec::new();
        }
        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Forgetting session metadata...");
        vec![Effect::ForgetSessionEntry { id }]
    }

    pub(super) fn handle_session_catalog_rename_input_changed(
        &mut self,
        id: String,
        value: String,
    ) -> Vec<Effect> {
        self.session_catalog.rename_inputs.insert(id, value);
        Vec::new()
    }

    pub(super) fn handle_session_catalog_rename_requested(&mut self, id: String) -> Vec<Effect> {
        if self.session_catalog.busy {
            return Vec::new();
        }
        let Some(item) = self.session_catalog.item(&id) else {
            self.status = StatusMessage::error("Session is no longer in the catalog.");
            return Vec::new();
        };
        let display_name = self.session_catalog.rename_value(&id, &item.display_name);
        if display_name.trim().is_empty() {
            self.status = StatusMessage::error("Session display name cannot be empty.");
            return Vec::new();
        }

        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Renaming session...");
        vec![Effect::RenameSessionEntry { id, display_name }]
    }

    pub(super) fn handle_session_catalog_duplicate_input_changed(
        &mut self,
        id: String,
        value: String,
    ) -> Vec<Effect> {
        self.session_catalog.duplicate_inputs.insert(id, value);
        Vec::new()
    }

    pub(super) fn handle_session_catalog_duplicate_requested(&mut self, id: String) -> Vec<Effect> {
        if self.session_catalog.busy {
            return Vec::new();
        }
        if let Some(blocker) =
            SessionCatalogOperation::Duplicate.cached_status_blocker(self.daemon_status.as_ref())
        {
            self.status = StatusMessage::warning(blocker);
            return Vec::new();
        }
        let Some(item) = self.session_catalog.item(&id) else {
            self.status = StatusMessage::error("Session is no longer in the catalog.");
            return Vec::new();
        };
        let target = self.session_catalog.duplicate_value(&id, &item.path);
        if target.trim().is_empty() {
            self.status = StatusMessage::error("Duplicate Session target cannot be empty.");
            return Vec::new();
        }

        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Duplicating session...");
        vec![Effect::DuplicateSessionEntry {
            id,
            target: PathBuf::from(target),
        }]
    }

    pub(super) fn handle_session_catalog_move_input_changed(
        &mut self,
        id: String,
        value: String,
    ) -> Vec<Effect> {
        self.session_catalog.move_inputs.insert(id, value);
        Vec::new()
    }

    pub(super) fn handle_session_catalog_move_requested(&mut self, id: String) -> Vec<Effect> {
        if self.session_catalog.busy {
            return Vec::new();
        }
        if let Some(blocker) =
            SessionCatalogOperation::Move.cached_status_blocker(self.daemon_status.as_ref())
        {
            self.status = StatusMessage::warning(blocker);
            return Vec::new();
        }
        let Some(item) = self.session_catalog.item(&id) else {
            self.status = StatusMessage::error("Session is no longer in the catalog.");
            return Vec::new();
        };
        let target = self.session_catalog.move_value(&id, &item.path);
        if target.trim().is_empty() {
            self.status = StatusMessage::error("Move Session target cannot be empty.");
            return Vec::new();
        }

        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Moving session...");
        vec![Effect::MoveSessionEntry {
            id,
            target: PathBuf::from(target),
        }]
    }

    pub(super) fn handle_session_catalog_reveal_requested(&mut self, id: String) -> Vec<Effect> {
        if self.session_catalog.busy {
            return Vec::new();
        }
        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Opening session folder...");
        vec![Effect::RevealSessionEntry { id }]
    }

    pub(super) fn handle_session_catalog_clear_tool_state_requested(
        &mut self,
        id: String,
    ) -> Vec<Effect> {
        if self.session_catalog.busy {
            return Vec::new();
        }
        if let Some(blocker) = SessionCatalogOperation::ClearToolState
            .cached_status_blocker(self.daemon_status.as_ref())
        {
            self.status = StatusMessage::warning(blocker);
            return Vec::new();
        }
        if self.session_catalog.item(&id).is_none() {
            self.status = StatusMessage::error("Session is no longer in the catalog.");
            return Vec::new();
        }

        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Clearing saved tool state...");
        vec![Effect::ClearSessionToolState { id }]
    }

    pub(super) fn handle_session_catalog_clear_requested(&mut self, id: String) -> Vec<Effect> {
        if self.session_catalog.busy {
            return Vec::new();
        }
        if let Some(blocker) =
            SessionCatalogOperation::Clear.cached_status_blocker(self.daemon_status.as_ref())
        {
            self.status = StatusMessage::warning(blocker);
            return Vec::new();
        }
        self.session_catalog.pending_clear_id = Some(id);
        self.status = StatusMessage::warning(
            "Clear saved data removes the selected session primary and non-lock sidecars. Press Confirm Clear to continue.",
        );
        Vec::new()
    }

    pub(super) fn handle_session_catalog_clear_confirmed(&mut self, id: String) -> Vec<Effect> {
        if self.session_catalog.busy {
            return Vec::new();
        }
        if self.session_catalog.pending_clear_id.as_deref() != Some(id.as_str()) {
            return Vec::new();
        }
        self.session_catalog.busy = true;
        self.status = StatusMessage::info("Clearing saved session data...");
        vec![Effect::ClearSessionEntry { id }]
    }

    pub(super) fn handle_session_catalog_clear_canceled(&mut self) -> Vec<Effect> {
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::idle();
        Vec::new()
    }

    pub(super) fn handle_session_catalog_action_completed(
        &mut self,
        result: Result<SessionCatalogActionResult, String>,
    ) -> Vec<Effect> {
        self.session_catalog.busy = false;
        self.session_catalog.pending_clear_id = None;
        match result {
            Ok(result) => {
                self.session_catalog.replace_items(result.items);
                self.status = if result.warning {
                    StatusMessage::warning(result.message)
                } else {
                    StatusMessage::success(result.message)
                };
            }
            Err(err) => {
                self.status = StatusMessage::error(err);
            }
        }
        Vec::new()
    }

    fn status_text(&self) -> Option<&str> {
        match &self.status {
            StatusMessage::Info(message)
            | StatusMessage::Success(message)
            | StatusMessage::Error(message)
            | StatusMessage::Warning(message) => Some(message.as_str()),
            StatusMessage::Idle => None,
        }
    }
}

#[cfg(test)]
mod tests;
