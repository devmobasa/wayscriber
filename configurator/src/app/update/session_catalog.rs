use std::path::PathBuf;

use iced::Task;

use crate::app::session_catalog::{
    clear_session_catalog_entry, duplicate_session_catalog_entry, forget_session_catalog_entry,
    load_session_catalog, rename_session_catalog_entry, reveal_session_catalog_entry,
    session_clear_blocker, session_duplicate_blocker,
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
        if let Some(blocker) = session_duplicate_blocker(self.daemon_status.as_ref()) {
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

    pub(super) fn handle_session_catalog_clear_requested(&mut self, id: String) -> Task<Message> {
        if self.session_catalog.busy {
            return Task::none();
        }
        if let Some(blocker) = session_clear_blocker(self.daemon_status.as_ref()) {
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
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::sync::MutexGuard;

    use super::*;
    use crate::models::{
        DesktopEnvironment, LightShortcutApplyCapability, SessionCatalogState,
        ShortcutApplyCapability, ShortcutBackend,
    };

    struct RuntimeEnvGuard {
        previous: Option<OsString>,
        _guard: MutexGuard<'static, ()>,
    }

    impl RuntimeEnvGuard {
        fn set_xdg_runtime_dir(path: &Path) -> Self {
            let guard = crate::test_env::lock();
            let previous = std::env::var_os("XDG_RUNTIME_DIR");
            unsafe {
                std::env::set_var("XDG_RUNTIME_DIR", path);
            }
            Self {
                previous,
                _guard: guard,
            }
        }
    }

    impl Drop for RuntimeEnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => unsafe { std::env::set_var("XDG_RUNTIME_DIR", value) },
                None => unsafe { std::env::remove_var("XDG_RUNTIME_DIR") },
            }
        }
    }

    fn catalog_item(id: &str, display_name: &str) -> SessionCatalogItem {
        SessionCatalogItem {
            id: id.to_string(),
            display_name: display_name.to_string(),
            path: PathBuf::from(format!("/tmp/{id}.wayscriber-session")),
            path_label: format!("/tmp/{id}.wayscriber-session"),
            canonical_path_label: None,
            created_label: "now".to_string(),
            last_opened_label: "Never".to_string(),
            last_saved_label: "Never".to_string(),
            artifacts: crate::models::session::SessionArtifactSummary {
                primary_exists: false,
                backup_exists: false,
                recovery_exists: false,
                clear_marker_exists: false,
                lock_exists: false,
                non_lock_size_bytes: 0,
            },
        }
    }

    fn inactive_daemon_status() -> crate::models::DaemonRuntimeStatus {
        crate::models::DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Unknown,
            shortcut_backend: ShortcutBackend::Manual,
            shortcut_apply_capability: ShortcutApplyCapability::Manual,
            light_shortcut_apply_capability: LightShortcutApplyCapability::Manual,
            systemctl_available: false,
            gsettings_available: false,
            service_installed: false,
            service_enabled: false,
            service_active: false,
            service_unit_path: None,
            configured_shortcut: None,
            light_controls_configured: false,
            light_controls_config_path: None,
        }
    }

    fn status_contains(status: &StatusMessage, needle: &str) -> bool {
        match status {
            StatusMessage::Info(text)
            | StatusMessage::Success(text)
            | StatusMessage::Error(text)
            | StatusMessage::Warning(text) => text.contains(needle),
            StatusMessage::Idle => false,
        }
    }

    #[test]
    fn rename_input_change_does_not_dirty_config() {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.is_dirty = false;

        let _ = app
            .handle_session_catalog_rename_input_changed("s-1".to_string(), "Lecture".to_string());

        assert!(!app.is_dirty);
        assert_eq!(
            app.session_catalog.rename_inputs.get("s-1"),
            Some(&"Lecture".to_string())
        );
    }

    #[test]
    fn duplicate_input_change_does_not_dirty_config() {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.is_dirty = false;

        let _ = app.handle_session_catalog_duplicate_input_changed(
            "s-1".to_string(),
            "/tmp/copy.wayscriber-session".to_string(),
        );

        assert!(!app.is_dirty);
        assert_eq!(
            app.session_catalog.duplicate_inputs.get("s-1"),
            Some(&"/tmp/copy.wayscriber-session".to_string())
        );
    }

    #[test]
    fn duplicate_request_blocks_without_daemon_status() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.session_catalog = SessionCatalogState::loading();
        app.session_catalog
            .replace_items(vec![catalog_item("s-1", "Lecture")]);
        app.daemon_status = None;

        let _ = app.handle_session_catalog_duplicate_requested("s-1".to_string());

        assert!(!app.session_catalog.busy);
        assert!(status_contains(&app.status, "status finishes loading"));
    }

    #[test]
    fn duplicate_request_sets_busy_when_safe() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.session_catalog = SessionCatalogState::loading();
        app.session_catalog
            .replace_items(vec![catalog_item("s-1", "Lecture")]);
        app.daemon_status = Some(inactive_daemon_status());

        let _ = app.handle_session_catalog_duplicate_requested("s-1".to_string());

        assert!(app.session_catalog.busy);
        assert!(status_contains(&app.status, "Duplicating session"));
    }

    #[test]
    fn clear_request_blocks_without_daemon_status() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.session_catalog = SessionCatalogState::loading();
        app.session_catalog
            .replace_items(vec![catalog_item("s-1", "Lecture")]);
        app.daemon_status = None;

        let _ = app.handle_session_catalog_clear_requested("s-1".to_string());

        assert!(app.session_catalog.pending_clear_id.is_none());
        assert!(status_contains(&app.status, "status finishes loading"));
    }

    #[test]
    fn clear_request_sets_pending_confirmation_when_safe() {
        let temp = crate::test_temp::tempdir().unwrap();
        let _env = RuntimeEnvGuard::set_xdg_runtime_dir(temp.path());
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.session_catalog = SessionCatalogState::loading();
        app.session_catalog
            .replace_items(vec![catalog_item("s-1", "Lecture")]);
        app.daemon_status = Some(inactive_daemon_status());

        let _ = app.handle_session_catalog_clear_requested("s-1".to_string());

        assert_eq!(app.session_catalog.pending_clear_id.as_deref(), Some("s-1"));
        assert!(status_contains(&app.status, "Confirm Clear"));
    }

    #[test]
    fn action_completed_replaces_catalog_items() {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.session_catalog.busy = true;

        let _ = app.handle_session_catalog_action_completed(Ok(SessionCatalogActionResult {
            message: "Done.".to_string(),
            items: vec![catalog_item("s-2", "Updated")],
            warning: false,
        }));

        assert!(!app.session_catalog.busy);
        assert_eq!(app.session_catalog.items.len(), 1);
        assert_eq!(app.session_catalog.items[0].display_name, "Updated");
        assert!(status_contains(&app.status, "Done."));
    }

    #[test]
    fn warning_action_completed_sets_warning_status() {
        let (mut app, _task) = ConfiguratorApp::new_app();
        app.session_catalog.busy = true;

        let _ = app.handle_session_catalog_action_completed(Ok(SessionCatalogActionResult {
            message: "Committed with warning.".to_string(),
            items: vec![catalog_item("s-2", "Updated")],
            warning: true,
        }));

        assert!(!app.session_catalog.busy);
        assert!(matches!(app.status, StatusMessage::Warning(_)));
        assert!(status_contains(&app.status, "Committed with warning."));
    }
}
