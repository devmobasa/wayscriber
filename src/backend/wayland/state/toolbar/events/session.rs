use super::*;
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
    snapshot.recent_sessions =
        if snapshot.drawer_open && snapshot.drawer_tab == crate::input::ToolbarDrawerTab::Session {
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

use dialog::choose_session_file;
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

    fn choose_session_file_with_overlay_suppressed(
        &mut self,
        mode: SessionFileDialogMode,
        current_path: Option<&Path>,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) -> Result<Option<PathBuf>> {
        let suppressed = self.enter_external_dialog_suppression(conn, qh)?;
        let result = choose_session_file(mode, current_path);
        if suppressed {
            self.exit_external_dialog_suppression(conn, qh)?;
        }
        result
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
        match self.choose_session_file_with_overlay_suppressed(
            SessionFileDialogMode::Open,
            current_path.as_deref(),
            conn,
            qh,
        ) {
            Ok(Some(path)) => self.handle_toolbar_open_session_path(&path),
            Ok(None) => {}
            Err(err) => self.set_session_toolbar_error(format!("Open session failed: {err:#}")),
        }
    }

    fn handle_toolbar_open_session_path(&mut self, path: &Path) {
        self.clear_toolbar_save_as_overwrite_prompt();
        match self.open_named_session_runtime(path) {
            Ok(report) => self.set_session_toolbar_info(format!(
                "Opened session {}",
                session_display_name(&report.opened_path)
            )),
            Err(err) if forget_missing_recent_session_after_open_error(path, &err) => self
                .set_session_toolbar_error(format!(
                    "Session file missing; removed from recent sessions: {}",
                    session_display_name(path)
                )),
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
        match self.choose_session_file_with_overlay_suppressed(
            SessionFileDialogMode::SaveAs,
            current_path.as_deref(),
            conn,
            qh,
        ) {
            Ok(Some(path)) => {
                let path = ensure_save_as_extension(path);
                match self.save_named_session_as_requires_overwrite(&path) {
                    Ok(true) => {
                        self.input_state.set_pending_save_as_overwrite(path.clone());
                        self.set_session_toolbar_info(format!(
                            "Replace existing session {}?",
                            session_display_name(&path)
                        ));
                    }
                    Ok(false) => {
                        self.commit_toolbar_save_session_as(
                            &path,
                            crate::session::SaveAsOverwrite::Deny,
                        );
                    }
                    Err(err) => {
                        self.set_session_toolbar_error(format!("Save session failed: {err:#}"));
                    }
                }
            }
            Ok(None) => {}
            Err(err) => self.set_session_toolbar_error(format!("Save session failed: {err:#}")),
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
        let Some(options) = self.session.options().cloned() else {
            self.set_session_toolbar_error(
                "Session info unavailable: no active persisted session target",
            );
            return;
        };

        match crate::session::inspect_session(&options) {
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
            .set_ui_toast(crate::input::state::UiToastKind::Info, message);
        self.mark_session_toolbar_changed();
    }

    fn set_session_toolbar_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        log::warn!("{message}");
        self.input_state
            .set_ui_toast(crate::input::state::UiToastKind::Error, message);
        self.mark_session_toolbar_changed();
    }

    fn mark_session_toolbar_changed(&mut self) {
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
        self.refresh_keyboard_interactivity();
    }
}

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
