use super::*;
use crate::backend::wayland::session::{SessionCommand, SessionCommandReport};
use crate::input::state::{Toast, ToastPriority};
use crate::session::catalog;
use crate::ui::toolbar::session_format::session_display_name;
use anyhow::{Context, Error as AnyhowError, Result, anyhow};
use std::path::{Path, PathBuf};
use wayland_client::{Connection, QueueHandle};

pub(super) fn populate_session_snapshot(
    snapshot: &mut ToolbarSnapshot,
    options: Option<&crate::session::SessionOptions>,
    home: &crate::backend::wayland::session::SessionHome,
) {
    snapshot.home_session_name = home.name();
    snapshot.at_home_session = home.is_at_home();
    let active_path = options.map(|options| options.session_file_path());
    snapshot.active_session_name = active_path.as_deref().map(session_display_name);
    // Recents are only read (from the catalog on disk) while the top strip's
    // Session popover is up.
    snapshot.recent_sessions = if snapshot.session_popover_open {
        recent_session_snapshots(active_path.as_deref())
    } else {
        Vec::new()
    };
    snapshot.active_session_path = active_path;
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
mod home;
mod queued;

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
            ToolbarEvent::OpenHomeSession => {
                self.handle_toolbar_open_home_session();
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
        let hidden = self
            .flush_overlay_dialog_frame(conn, qh)
            .and_then(|outcome| {
                if dialog_frame_accepted(DialogFramePhase::Entry, outcome) {
                    Ok(())
                } else {
                    // The transparent frame was deferred, so the overlay's old
                    // pixels are still on screen. Starting the chooser now would
                    // let them sit over it - on Niri the overlay layer can cover
                    // the chooser outright. Roll back and make the user retry
                    // instead: nothing has been chosen yet, so nothing is lost.
                    Err(anyhow!(
                        "overlay buffers were still in flight; try again in a moment"
                    ))
                }
            });
        if let Err(err) = hidden {
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
        // Restoration takes the opposite policy to entry: a deferred frame is
        // fine here. The redraw stays pending and the event loop paints it,
        // whereas failing would discard the file the user just chose.
        self.flush_overlay_dialog_frame(conn, qh)
            .and_then(|outcome| {
                if dialog_frame_accepted(DialogFramePhase::Restoration, outcome) {
                    Ok(())
                } else {
                    Err(anyhow!(
                        "overlay restoration frame was rejected by dialog policy"
                    ))
                }
            })
            .context("failed to restore overlay after session dialog")
    }

    /// Renders and flushes the overlay's dialog-suppression frame.
    ///
    /// Returns what the render actually did so each caller can apply its own
    /// policy; with no surface or queue there is nothing to commit, which
    /// counts as committed.
    fn flush_overlay_dialog_frame(
        &mut self,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) -> Result<RenderOutcome> {
        let mut outcome = RenderOutcome::Committed {
            keep_rendering: false,
        };
        if self.surface.is_configured()
            && let Some(qh) = qh
        {
            // This frame makes the overlay's pixels transparent before a file
            // dialog opens, and opaque again afterwards.
            //
            // Never block waiting for a slot here: a `Connection::roundtrip`
            // only confirms the server processed our requests - it says
            // nothing about `wl_buffer.release`, which arrives when the
            // compositor stops using the buffer - and it has no timeout.
            // Report the outcome instead and let the caller decide.
            outcome = self.render(qh)?;
            if let RenderOutcome::BuffersInFlight = outcome {
                debug!("Overlay dialog frame deferred - all buffers still in flight");
            }
        }
        if let Some(conn) = conn {
            conn.flush()
                .map_err(|err| anyhow!("Wayland flush failed: {err}"))?;
        }
        Ok(outcome)
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
        let command = SessionCommand::Open(path.to_path_buf());
        if let Err(error) = self.start_session_command(command) {
            self.report_session_command_error("Open session failed", &error);
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
        if let Err(error) = self.start_session_command(SessionCommand::CheckOverwrite(path)) {
            self.report_session_command_error("Save session failed", &error);
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
        if let Err(error) =
            self.start_session_command(SessionCommand::SaveAs(path.to_path_buf(), overwrite))
        {
            self.clear_toolbar_save_as_overwrite_prompt();
            self.report_session_command_error("Save session failed", &error);
        }
    }

    fn handle_toolbar_session_info(&mut self) {
        if let Err(error) = self.start_session_command(SessionCommand::Inspect) {
            self.report_session_command_error("Session info failed", &error);
        }
    }

    fn handle_toolbar_clear_session(&mut self) {
        self.clear_toolbar_save_as_overwrite_prompt();
        if let Err(error) = self.start_session_command(SessionCommand::Clear) {
            self.report_session_command_error("Clear session failed", &error);
        }
    }

    pub(in crate::backend::wayland) fn finish_session_command(
        &mut self,
        report: SessionCommandReport,
    ) {
        match report {
            SessionCommandReport::Open(report) => {
                let name = session_display_name(&report.opened_path);
                if let Some(error) = report.catalog_error {
                    self.set_session_toolbar_error(format!(
                        "Opened session {name}; recent-session catalog update failed: {error:#}"
                    ));
                } else {
                    self.set_session_toolbar_info(format!("Opened session {name}"));
                }
            }
            SessionCommandReport::Output {
                abandoned,
                too_large,
                first_output_resolved,
            } => {
                if let Some(too_large) = too_large {
                    self.protect_too_large_session(too_large);
                }
                if let Some((remembered, reason)) = abandoned {
                    self.notify_remembered_session_abandoned(&remembered, &reason);
                }
                self.announce_launch_restore(first_output_resolved);
            }
            SessionCommandReport::Home => self.finish_open_home_session(),
            SessionCommandReport::SaveAs(report) => {
                self.clear_toolbar_save_as_overwrite_prompt();
                self.set_session_toolbar_info(format!(
                    "Saved session as {}",
                    session_display_name(&report.saved_path)
                ));
            }
            SessionCommandReport::Overwrite(path, required) => {
                if required {
                    self.input_state.set_pending_save_as_overwrite(path.clone());
                    self.set_session_toolbar_info(format!(
                        "Replace existing session {}?",
                        session_display_name(&path)
                    ));
                } else {
                    self.commit_toolbar_save_session_as(
                        &path,
                        crate::session::SaveAsOverwrite::Deny,
                    );
                }
            }
            SessionCommandReport::Clear(report) => self.set_session_toolbar_info(format!(
                "Cleared session {}",
                session_display_name(&report.cleared_path)
            )),
            SessionCommandReport::ClearTools(report) => {
                let message = match report.outcome {
                    Some(crate::session::ClearToolStateOutcome::Cleared {
                        preserved_board_data: true,
                    }) => {
                        "Tool defaults reset from config. Saved boards and history were preserved."
                    }
                    Some(crate::session::ClearToolStateOutcome::Cleared {
                        preserved_board_data: false,
                    }) => "Tool defaults reset from config. No board data was present.",
                    Some(crate::session::ClearToolStateOutcome::NoToolState) => {
                        "Tool defaults reset from config. No saved tool state was stored."
                    }
                    Some(crate::session::ClearToolStateOutcome::NoSession) => {
                        "Tool defaults reset from config. No saved session file was present."
                    }
                    None => {
                        "Tool defaults reset from config for this run. No active session file to edit."
                    }
                };
                self.set_session_toolbar_info(message);
            }
            SessionCommandReport::Inspection(inspection) => {
                self.set_session_toolbar_info(session_info_summary(&inspection))
            }
            SessionCommandReport::Forgotten(path, forgotten) => {
                self.set_session_toolbar_error(format!(
                    "Session file missing; {}: {}",
                    if forgotten {
                        "removed from recent sessions"
                    } else {
                        "no recent-session entry matched"
                    },
                    session_display_name(&path)
                ))
            }
        }
    }

    pub(in crate::backend::wayland) fn fail_session_command(
        &mut self,
        command: &SessionCommand,
        error: &AnyhowError,
    ) {
        if let SessionCommand::Open(path) = command
            && missing_session_error_matches_path(path, error)
        {
            if let Err(catalog_error) =
                self.start_session_command(SessionCommand::Forget(path.clone()))
            {
                self.report_session_command_error(
                    "Session file missing and recent-session cleanup failed",
                    &catalog_error,
                );
            }
            return;
        }
        let prefix = match command {
            SessionCommand::Output { .. } => return self.fail_output_session_command(error),
            SessionCommand::Open(_) => "Open session failed",
            SessionCommand::OpenHome(_) => "Return to the home session failed",
            SessionCommand::SaveAs(..) | SessionCommand::CheckOverwrite(_) => "Save session failed",
            SessionCommand::Clear => "Clear session failed",
            SessionCommand::ClearTools(_) => "Failed to reset tool defaults",
            SessionCommand::Inspect => "Session info failed",
            SessionCommand::Forget(_) => "Session file missing and recent-session cleanup failed",
        };
        self.report_session_command_error(prefix, error);
    }

    fn fail_output_session_command(&mut self, error: &AnyhowError) {
        let preservation_failed = error
            .downcast_ref::<crate::session::CorruptArtifactPreservationFailed>()
            .is_some();
        let source_unwritable = error
            .downcast_ref::<crate::backend::wayland::session::SourceWriteRefused>()
            .is_some();
        let backoff = self.output_transition_failure_backoff();
        if !crate::backend::wayland::session::driver::defer_failed_output(
            &mut self.session,
            error,
            std::time::Instant::now(),
            backoff,
        ) {
            log::debug!("Output session transition requeued after guard abort: {error:#}");
            return;
        }
        log::warn!("Output session transition deferred after persistence failure: {error:#}");
        if preservation_failed {
            self.notify_session_load_failure(error);
        } else if source_unwritable {
            if self.session.mark_output_transition_notified() {
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "session.save",
                    Toast::error(format!("Drawings kept on screen: {error:#}")).duration_ms(20_000),
                );
            }
        } else if self.session.is_loaded() {
            self.notify_output_transition_deferred();
        }
    }

    pub(in crate::backend::wayland) fn report_session_command_error(
        &mut self,
        prefix: &str,
        error: &AnyhowError,
    ) {
        self.set_session_toolbar_error(format!("{prefix}: {error:#}"));
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

#[derive(Clone, Copy)]
enum DialogFramePhase {
    Entry,
    Restoration,
}

/// Whether a dialog-suppression frame is acceptable at the given phase.
///
/// Entry and restoration take deliberately opposite policies. Entry needs the
/// transparent frame on screen first: starting the chooser while the overlay's
/// old pixels are still up leaves them sitting over it, and on compositors
/// where the overlay maps to the overlay layer (Niri, Sway) they can cover the
/// chooser outright. Nothing is lost by refusing - the user has not chosen a
/// file yet. Restoration is the reverse: the file has been chosen, so a
/// deferred frame is accepted and the event loop repaints.
fn dialog_frame_accepted(phase: DialogFramePhase, outcome: RenderOutcome) -> bool {
    match (phase, outcome) {
        (DialogFramePhase::Entry, RenderOutcome::Committed { .. })
        | (DialogFramePhase::Restoration, RenderOutcome::Committed { .. })
        | (DialogFramePhase::Restoration, RenderOutcome::BuffersInFlight) => true,
        (DialogFramePhase::Entry, RenderOutcome::BuffersInFlight) => false,
    }
}

#[cfg(test)]
mod tests;
