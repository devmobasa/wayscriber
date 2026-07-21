use super::*;
use crate::input::state::{Toast, ToastPriority};
use crate::session::catalog;
use anyhow::{Context, Error as AnyhowError, Result, anyhow};
use std::path::{Path, PathBuf};
use wayland_client::{Connection, QueueHandle};

pub(super) fn populate_session_snapshot(
    snapshot: &mut ToolbarSnapshot,
    options: Option<&crate::session::SessionOptions>,
) {
    let active_path = options.map(|options| options.session_file_path());
    snapshot.active_session_name = active_path.as_deref().map(session_display_name);
    snapshot.active_session_path = active_path.clone();
    // Recents are only read (from the catalog on disk) while a surface that
    // shows them is up: the side palette's Session pane or the top strip's
    // Session popover.
    let session_surface_open = snapshot.active_side_pane == crate::ui::toolbar::SidePane::Session
        || snapshot.session_popover_open;
    snapshot.recent_sessions = if session_surface_open {
        recent_session_snapshots(active_path.as_deref())
    } else {
        Vec::new()
    };
}

fn session_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

pub(super) fn session_info_summary(inspection: &crate::session::SessionInspection) -> String {
    let name = session_display_name(&inspection.session_path);
    if !inspection.exists {
        return if inspection.backup_exists {
            match inspection.backup_size_bytes {
                Some(size) => format!(
                    "Session {name}: no primary file, backup {}",
                    format_byte_count(size)
                ),
                None => format!("Session {name}: no primary file, backup present"),
            }
        } else {
            format!("Session {name}: no saved file yet")
        };
    }

    let size = inspection
        .size_bytes
        .map(format_byte_count)
        .unwrap_or_else(|| "unknown size".to_string());
    let shapes = inspection
        .frame_counts
        .map(|counts| {
            format!(
                ", shapes T/W/B {}/{}/{}",
                counts.transparent, counts.whiteboard, counts.blackboard
            )
        })
        .unwrap_or_default();
    let history = if inspection.history_present {
        "history"
    } else {
        "no history"
    };
    format!("Session {name}: {size}{shapes}, {history}")
}

fn format_byte_count(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;

    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB)
    }
}

fn recent_session_snapshots(
    current_path: Option<&Path>,
) -> Vec<crate::ui::toolbar::SessionRecentSnapshot> {
    let recent = match catalog::recent_sessions() {
        Ok(recent) => recent,
        Err(err) => {
            log::warn!("Failed to read session catalog for toolbar recents: {err:#}");
            return Vec::new();
        }
    };

    recent
        .into_iter()
        .filter_map(|entry| {
            let path = PathBuf::from(entry.path);
            if current_path
                .map(|current| catalog::session_paths_match(current, &path))
                .unwrap_or(false)
            {
                return None;
            }
            Some(crate::ui::toolbar::SessionRecentSnapshot {
                display_name: entry.display_name,
                path,
            })
        })
        .take(3)
        .collect()
}

mod dialog;

pub(in crate::backend::wayland::state) use dialog::SessionFileDialogController;
pub(super) use dialog::{SessionFileDialogMode, ensure_save_as_extension};
#[cfg(test)]
pub(super) use dialog::{
    SessionFileDialogResult, choose_session_file_from, default_save_as_path, save_as_file_name,
};

impl WaylandState {
    pub(super) fn handle_toolbar_session_event(
        &mut self,
        event: &ToolbarEvent,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) -> bool {
        match event {
            ToolbarEvent::OpenSession => {
                self.handle_toolbar_open_session(conn, qh);
                true
            }
            ToolbarEvent::OpenRecentSession(path) => {
                self.handle_toolbar_open_session_path(path);
                true
            }
            ToolbarEvent::SaveSessionAs => {
                self.handle_toolbar_save_session_as(conn, qh);
                true
            }
            ToolbarEvent::SaveSessionAsConfirm(path) => {
                self.handle_toolbar_save_session_as_confirm(path);
                true
            }
            ToolbarEvent::SaveSessionAsCancel => {
                self.handle_toolbar_save_session_as_cancel();
                true
            }
            ToolbarEvent::SessionInfo => {
                self.handle_toolbar_session_info();
                true
            }
            ToolbarEvent::ClearSession => {
                self.handle_toolbar_clear_session();
                true
            }
            _ => false,
        }
    }

    fn current_session_file_path(&self) -> Option<PathBuf> {
        self.session
            .options()
            .map(crate::session::SessionOptions::session_file_path)
    }

    fn start_session_file_dialog_with_overlay_suppressed(
        &mut self,
        mode: SessionFileDialogMode,
        current_path: Option<&Path>,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) -> Result<()> {
        let suppressed = self.enter_external_dialog_suppression(conn, qh)?;
        if let Err(error) = self
            .session_dialog
            .start(mode, current_path.map(Path::to_path_buf))
        {
            if suppressed {
                self.exit_external_dialog_suppression(conn, qh)?;
            }
            return Err(error);
        }
        Ok(())
    }

    fn enter_external_dialog_suppression(
        &mut self,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) -> Result<bool> {
        if !self.enter_overlay_suppression(OverlaySuppression::ExternalDialog) {
            return Err(anyhow!(
                "another overlay operation is already active; try again after it finishes"
            ));
        }
        if let Err(err) = self.flush_overlay_dialog_frame(conn, qh) {
            self.exit_overlay_suppression(OverlaySuppression::ExternalDialog);
            let _ = self.flush_overlay_dialog_frame(conn, qh);
            return Err(err).context("failed to hide overlay before opening session dialog");
        }
        Ok(true)
    }

    fn exit_external_dialog_suppression(
        &mut self,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) -> Result<()> {
        self.exit_overlay_suppression(OverlaySuppression::ExternalDialog);
        self.flush_overlay_dialog_frame(conn, qh)
            .context("failed to restore overlay after session dialog")
    }

    fn flush_overlay_dialog_frame(
        &mut self,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) -> Result<()> {
        if self.surface.is_configured()
            && let Some(qh) = qh
        {
            self.render(qh)?;
        }
        if let Some(conn) = conn {
            conn.flush()
                .map_err(|err| anyhow!("Wayland flush failed: {err}"))?;
        }
        Ok(())
    }

    fn handle_toolbar_open_session(
        &mut self,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) {
        self.clear_toolbar_save_as_overwrite_prompt();
        let current_path = self.current_session_file_path();
        if let Err(err) = self.start_session_file_dialog_with_overlay_suppressed(
            SessionFileDialogMode::Open,
            current_path.as_deref(),
            conn,
            qh,
        ) {
            self.set_session_toolbar_error(format!("Open session failed: {err:#}"));
        }
    }

    fn handle_toolbar_open_session_path(&mut self, path: &Path) {
        self.clear_toolbar_save_as_overwrite_prompt();
        match self.open_named_session_runtime(path) {
            Ok(report) => self.set_session_toolbar_info(format!(
                "Opened session {}",
                session_display_name(&report.opened_path)
            )),
            Err(err) if missing_session_error_matches_path(path, &err) => {
                match self.forget_named_session_by_path(path.to_path_buf()) {
                    Ok(true) => self.set_session_toolbar_error(format!(
                        "Session file missing; removed from recent sessions: {}",
                        session_display_name(path)
                    )),
                    Ok(false) => self.set_session_toolbar_error(format!(
                        "Session file missing; no recent-session entry matched: {}",
                        session_display_name(path)
                    )),
                    Err(catalog_err) => self.set_session_toolbar_error(format!(
                        "Session file missing and recent-session cleanup failed for {}: {catalog_err:#}",
                        session_display_name(path)
                    )),
                }
            }
            Err(err) => self.set_session_toolbar_error(format!("Open session failed: {err:#}")),
        }
    }

    fn handle_toolbar_save_session_as(
        &mut self,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) {
        self.clear_toolbar_save_as_overwrite_prompt();
        let current_path = self.current_session_file_path();
        if let Err(err) = self.start_session_file_dialog_with_overlay_suppressed(
            SessionFileDialogMode::SaveAs,
            current_path.as_deref(),
            conn,
            qh,
        ) {
            self.set_session_toolbar_error(format!("Save session failed: {err:#}"));
        }
    }

    pub(in crate::backend::wayland) fn poll_session_file_dialog_completion(
        &mut self,
        qh: &QueueHandle<Self>,
    ) {
        let completion = match self.session_dialog.try_receive() {
            Ok(Some(completion)) => completion,
            Ok(None) => return,
            Err(error) => {
                let _ = self.exit_external_dialog_suppression(None, Some(qh));
                self.set_session_toolbar_error(format!("Session dialog failed: {error:#}"));
                return;
            }
        };
        if let Err(error) = self.exit_external_dialog_suppression(None, Some(qh)) {
            self.set_session_toolbar_error(format!("Session dialog restoration failed: {error:#}"));
            return;
        }
        match (completion.mode, completion.result) {
            (SessionFileDialogMode::Open, Ok(Some(path))) => {
                self.handle_toolbar_open_session_path(&path)
            }
            (SessionFileDialogMode::Open, Ok(None)) | (SessionFileDialogMode::SaveAs, Ok(None)) => {
            }
            (SessionFileDialogMode::Open, Err(error)) => {
                self.set_session_toolbar_error(format!("Open session failed: {error}"));
            }
            (SessionFileDialogMode::SaveAs, Err(error)) => {
                self.set_session_toolbar_error(format!("Save session failed: {error}"));
            }
            (SessionFileDialogMode::SaveAs, Ok(Some(path))) => {
                self.handle_selected_save_as_path(ensure_save_as_extension(path));
            }
        }
    }

    fn handle_selected_save_as_path(&mut self, path: PathBuf) {
        match self.save_named_session_as_requires_overwrite(&path) {
            Ok(true) => {
                self.input_state.set_pending_save_as_overwrite(path.clone());
                self.set_session_toolbar_info(format!(
                    "Replace existing session {}?",
                    session_display_name(&path)
                ));
            }
            Ok(false) => {
                self.commit_toolbar_save_session_as(&path, crate::session::SaveAsOverwrite::Deny)
            }
            Err(err) => {
                self.set_session_toolbar_error(format!("Save session failed: {err:#}"));
            }
        }
    }

    fn handle_toolbar_save_session_as_confirm(&mut self, path: &Path) {
        let Some(pending_path) = self
            .input_state
            .pending_save_as_overwrite()
            .map(PathBuf::from)
        else {
            self.set_session_toolbar_error("Save session failed: no overwrite target pending");
            return;
        };
        if !catalog::session_paths_match(&pending_path, path) {
            self.clear_toolbar_save_as_overwrite_prompt();
            self.set_session_toolbar_error("Save session failed: overwrite target changed");
            return;
        }

        self.clear_toolbar_save_as_overwrite_prompt();
        self.commit_toolbar_save_session_as(path, crate::session::SaveAsOverwrite::ConfirmReplace);
    }

    fn handle_toolbar_save_session_as_cancel(&mut self) {
        if self.clear_toolbar_save_as_overwrite_prompt() {
            self.set_session_toolbar_info("Save As canceled");
        }
    }

    fn commit_toolbar_save_session_as(
        &mut self,
        path: &Path,
        overwrite: crate::session::SaveAsOverwrite,
    ) {
        match self.save_named_session_as_runtime(path, overwrite) {
            Ok(report) => {
                self.clear_toolbar_save_as_overwrite_prompt();
                self.set_session_toolbar_info(format!(
                    "Saved session as {}",
                    session_display_name(&report.saved_path)
                ));
            }
            Err(err) => {
                self.clear_toolbar_save_as_overwrite_prompt();
                self.set_session_toolbar_error(format!("Save session failed: {err:#}"));
            }
        }
    }

    fn handle_toolbar_session_info(&mut self) {
        match self.inspect_active_session() {
            Ok(inspection) => self.set_session_toolbar_info(session_info_summary(&inspection)),
            Err(err) => self.set_session_toolbar_error(format!("Session info failed: {err:#}")),
        }
    }

    fn handle_toolbar_clear_session(&mut self) {
        self.clear_toolbar_save_as_overwrite_prompt();
        match self.clear_current_session_runtime() {
            Ok(report) => self.set_session_toolbar_info(format!(
                "Cleared session {}",
                session_display_name(&report.cleared_path)
            )),
            Err(err) => self.set_session_toolbar_error(format!("Clear session failed: {err:#}")),
        }
    }

    fn clear_toolbar_save_as_overwrite_prompt(&mut self) -> bool {
        let cleared = self.input_state.clear_pending_save_as_overwrite().is_some();
        if cleared {
            self.mark_session_toolbar_changed();
        }
        cleared
    }

    fn set_session_toolbar_info(&mut self, message: impl Into<String>) {
        self.input_state
            .push_toast(ToastPriority::Info, "session", Toast::info(message));
        self.mark_session_toolbar_changed();
    }

    fn set_session_toolbar_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        log::warn!("{message}");
        self.input_state
            .push_toast(ToastPriority::Critical, "session", Toast::error(message));
        self.mark_session_toolbar_changed();
    }

    fn mark_session_toolbar_changed(&mut self) {
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
        self.refresh_keyboard_interactivity();
    }
}

#[cfg(test)]
pub(super) fn forget_missing_recent_session_after_open_error(
    path: &Path,
    err: &AnyhowError,
) -> bool {
    if !missing_session_error_matches_path(path, err) {
        return false;
    }

    match catalog::forget_session_by_path(path) {
        Ok(true) => true,
        Ok(false) => {
            log::warn!(
                "Open recent session target is missing but no catalog entry matched {}",
                path.display()
            );
            false
        }
        Err(catalog_err) => {
            log::warn!(
                "Failed to remove missing recent session {} from catalog: {}",
                path.display(),
                catalog_err
            );
            false
        }
    }
}

fn missing_session_error_matches_path(path: &Path, err: &AnyhowError) -> bool {
    err.downcast_ref::<crate::session::MissingNamedSessionFile>()
        .is_some_and(|missing| catalog::session_paths_match(missing.path(), path))
        || err
            .downcast_ref::<crate::session::MissingNamedSessionParent>()
            .is_some_and(|missing| catalog::session_paths_match(missing.path(), path))
}
