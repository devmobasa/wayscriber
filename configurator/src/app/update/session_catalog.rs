use std::path::PathBuf;

use iced::Task;

use crate::app::session_catalog::{
    clear_session_catalog_entry, clear_session_catalog_tool_state_entry,
    duplicate_session_catalog_entry, forget_session_catalog_entry, load_session_catalog,
    move_session_catalog_entry, rename_session_catalog_entry, reveal_session_catalog_entry,
    session_clear_cached_status_blocker, session_clear_tool_state_cached_status_blocker,
    session_duplicate_cached_status_blocker, session_move_cached_status_blocker,
};
use crate::messages::Message;
use crate::models::{SessionCatalogActionResult, SessionCatalogItem};

use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_session_catalog_loaded(
        &mut self,
        result: Result<Vec<SessionCatalogItem>, String>,
    ) -> Task<Message> {
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
        Task::none()
    }

    pub(super) fn handle_session_catalog_refresh_requested(&mut self) -> Task<Message> {
        if self.session_catalog.busy {
            return Task::none();
        }
        self.session_catalog.is_loading = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Loading sessions...");
        Task::perform(load_session_catalog(), Message::SessionCatalogLoaded)
    }

    pub(super) fn handle_session_catalog_forget_requested(&mut self, id: String) -> Task<Message> {
        if self.session_catalog.busy {
            return Task::none();
        }
        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Forgetting session metadata...");
        Task::perform(
            forget_session_catalog_entry(id),
            Message::SessionCatalogActionCompleted,
        )
    }

    pub(super) fn handle_session_catalog_rename_input_changed(
        &mut self,
        id: String,
        value: String,
    ) -> Task<Message> {
        self.session_catalog.rename_inputs.insert(id, value);
        Task::none()
    }

    pub(super) fn handle_session_catalog_rename_requested(&mut self, id: String) -> Task<Message> {
        if self.session_catalog.busy {
            return Task::none();
        }
        let Some(item) = self.session_catalog.item(&id) else {
            self.status = StatusMessage::error("Session is no longer in the catalog.");
            return Task::none();
        };
        let display_name = self.session_catalog.rename_value(&id, &item.display_name);
        if display_name.trim().is_empty() {
            self.status = StatusMessage::error("Session display name cannot be empty.");
            return Task::none();
        }

        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Renaming session...");
        Task::perform(
            rename_session_catalog_entry(id, display_name),
            Message::SessionCatalogActionCompleted,
        )
    }

    pub(super) fn handle_session_catalog_duplicate_input_changed(
        &mut self,
        id: String,
        value: String,
    ) -> Task<Message> {
        self.session_catalog.duplicate_inputs.insert(id, value);
        Task::none()
    }

    pub(super) fn handle_session_catalog_duplicate_requested(
        &mut self,
        id: String,
    ) -> Task<Message> {
        if self.session_catalog.busy {
            return Task::none();
        }
        if let Some(blocker) = session_duplicate_cached_status_blocker(self.daemon_status.as_ref())
        {
            self.status = StatusMessage::warning(blocker);
            return Task::none();
        }
        let Some(item) = self.session_catalog.item(&id) else {
            self.status = StatusMessage::error("Session is no longer in the catalog.");
            return Task::none();
        };
        let target = self.session_catalog.duplicate_value(&id, &item.path);
        if target.trim().is_empty() {
            self.status = StatusMessage::error("Duplicate Session target cannot be empty.");
            return Task::none();
        }

        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Duplicating session...");
        Task::perform(
            duplicate_session_catalog_entry(id, PathBuf::from(target)),
            Message::SessionCatalogActionCompleted,
        )
    }

    pub(super) fn handle_session_catalog_move_input_changed(
        &mut self,
        id: String,
        value: String,
    ) -> Task<Message> {
        self.session_catalog.move_inputs.insert(id, value);
        Task::none()
    }

    pub(super) fn handle_session_catalog_move_requested(&mut self, id: String) -> Task<Message> {
        if self.session_catalog.busy {
            return Task::none();
        }
        if let Some(blocker) = session_move_cached_status_blocker(self.daemon_status.as_ref()) {
            self.status = StatusMessage::warning(blocker);
            return Task::none();
        }
        let Some(item) = self.session_catalog.item(&id) else {
            self.status = StatusMessage::error("Session is no longer in the catalog.");
            return Task::none();
        };
        let target = self.session_catalog.move_value(&id, &item.path);
        if target.trim().is_empty() {
            self.status = StatusMessage::error("Move Session target cannot be empty.");
            return Task::none();
        }

        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Moving session...");
        Task::perform(
            move_session_catalog_entry(id, PathBuf::from(target)),
            Message::SessionCatalogActionCompleted,
        )
    }

    pub(super) fn handle_session_catalog_reveal_requested(&mut self, id: String) -> Task<Message> {
        if self.session_catalog.busy {
            return Task::none();
        }
        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Opening session folder...");
        Task::perform(
            reveal_session_catalog_entry(id),
            Message::SessionCatalogActionCompleted,
        )
    }

    pub(super) fn handle_session_catalog_clear_tool_state_requested(
        &mut self,
        id: String,
    ) -> Task<Message> {
        if self.session_catalog.busy {
            return Task::none();
        }
        if let Some(blocker) =
            session_clear_tool_state_cached_status_blocker(self.daemon_status.as_ref())
        {
            self.status = StatusMessage::warning(blocker);
            return Task::none();
        }
        if self.session_catalog.item(&id).is_none() {
            self.status = StatusMessage::error("Session is no longer in the catalog.");
            return Task::none();
        }

        self.session_catalog.busy = true;
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::info("Clearing saved tool state...");
        Task::perform(
            clear_session_catalog_tool_state_entry(id),
            Message::SessionCatalogActionCompleted,
        )
    }

    pub(super) fn handle_session_catalog_clear_requested(&mut self, id: String) -> Task<Message> {
        if self.session_catalog.busy {
            return Task::none();
        }
        if let Some(blocker) = session_clear_cached_status_blocker(self.daemon_status.as_ref()) {
            self.status = StatusMessage::warning(blocker);
            return Task::none();
        }
        self.session_catalog.pending_clear_id = Some(id);
        self.status = StatusMessage::warning(
            "Clear saved data removes the selected session primary and non-lock sidecars. Press Confirm Clear to continue.",
        );
        Task::none()
    }

    pub(super) fn handle_session_catalog_clear_confirmed(&mut self, id: String) -> Task<Message> {
        if self.session_catalog.busy {
            return Task::none();
        }
        if self.session_catalog.pending_clear_id.as_deref() != Some(id.as_str()) {
            return Task::none();
        }
        self.session_catalog.busy = true;
        self.status = StatusMessage::info("Clearing saved session data...");
        Task::perform(
            clear_session_catalog_entry(id),
            Message::SessionCatalogActionCompleted,
        )
    }

    pub(super) fn handle_session_catalog_clear_canceled(&mut self) -> Task<Message> {
        self.session_catalog.pending_clear_id = None;
        self.status = StatusMessage::idle();
        Task::none()
    }

    pub(super) fn handle_session_catalog_action_completed(
        &mut self,
        result: Result<SessionCatalogActionResult, String>,
    ) -> Task<Message> {
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
        Task::none()
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
