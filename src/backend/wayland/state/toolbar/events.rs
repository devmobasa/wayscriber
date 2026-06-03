use super::*;
use crate::{
    input::InputState,
    onboarding::OnboardingState,
    session::catalog,
    ui::toolbar::model::{
        ToolbarBackendRoute, ToolbarEventPolicy, ToolbarPersistence, ToolbarPersistenceTarget,
        ToolbarPreApplyEffect, ToolbarUiPersistenceTarget,
    },
};
use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::Command;
use wayland_client::{Connection, QueueHandle};

const SESSION_FILE_EXTENSION: &str = "wayscriber-session";

fn persisted_tool_preview_value(current: bool, presenter_restore: Option<bool>) -> bool {
    presenter_restore.unwrap_or(current)
}

fn record_drawer_hint_shown(state: &mut OnboardingState) -> bool {
    if state.drawer_hint_count >= crate::onboarding::DRAWER_HINT_MAX {
        return false;
    }

    state.drawer_hint_count = state.drawer_hint_count.saturating_add(1);
    state.drawer_hint_shown = state.drawer_hint_count >= crate::onboarding::DRAWER_HINT_MAX;
    true
}

fn apply_toolbar_ui_config_target(
    config: &mut crate::config::Config,
    input_state: &InputState,
    target: ToolbarUiPersistenceTarget,
) {
    match target {
        ToolbarUiPersistenceTarget::StatusBar => {
            config.ui.show_status_bar = input_state.show_status_bar;
        }
        ToolbarUiPersistenceTarget::StatusBoardBadge => {
            config.ui.show_status_board_badge = input_state.show_status_board_badge;
        }
        ToolbarUiPersistenceTarget::StatusPageBadge => {
            config.ui.show_status_page_badge = input_state.show_status_page_badge;
        }
        ToolbarUiPersistenceTarget::FloatingBadgeAlways => {
            config.ui.show_floating_badge_always = input_state.show_floating_badge_always;
        }
    }
}

fn populate_session_snapshot(
    snapshot: &mut ToolbarSnapshot,
    options: Option<&crate::session::SessionOptions>,
) {
    let active_path = options.map(|options| options.session_file_path());
    snapshot.active_session_name = active_path.as_deref().map(session_display_name);
    snapshot.active_session_path = active_path.clone();
    snapshot.recent_sessions =
        if snapshot.drawer_open && snapshot.drawer_tab == crate::input::ToolbarDrawerTab::App {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionFileDialogMode {
    Open,
    SaveAs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionFileDialogResult {
    Selected(PathBuf),
    Cancelled,
}

type SessionFileChooser =
    fn(SessionFileDialogMode, Option<&Path>) -> Result<Option<SessionFileDialogResult>>;

fn choose_session_file(
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
) -> Result<Option<PathBuf>> {
    choose_session_file_from(
        mode,
        current_path,
        &[
            run_zenity_session_file_dialog,
            run_kdialog_session_file_dialog,
        ],
    )
}

fn choose_session_file_from(
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
    choosers: &[SessionFileChooser],
) -> Result<Option<PathBuf>> {
    let mut errors = Vec::new();
    for chooser in choosers {
        match chooser(mode, current_path) {
            Ok(Some(SessionFileDialogResult::Selected(path))) => return Ok(Some(path)),
            Ok(Some(SessionFileDialogResult::Cancelled)) => return Ok(None),
            Ok(None) => {}
            Err(err) => {
                let message = format!("{err:#}");
                log::warn!("Session file chooser failed; trying fallback if available: {message}");
                errors.push(message);
            }
        }
    }

    if errors.is_empty() {
        return Err(anyhow!(
            "No supported session file chooser found; tried zenity and kdialog"
        ));
    }

    Err(anyhow!(
        "No usable session file chooser found; tried zenity and kdialog: {}",
        errors.join("; ")
    ))
}

fn run_zenity_session_file_dialog(
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
) -> Result<Option<SessionFileDialogResult>> {
    let mut command = Command::new("zenity");
    command
        .arg("--file-selection")
        .arg("--title")
        .arg(match mode {
            SessionFileDialogMode::Open => "Open Wayscriber Session",
            SessionFileDialogMode::SaveAs => "Save Wayscriber Session As",
        });
    match mode {
        SessionFileDialogMode::Open => {
            if let Some(path) = current_path.and_then(Path::parent) {
                command.arg("--filename").arg(path);
            }
        }
        SessionFileDialogMode::SaveAs => {
            command.arg("--save");
            command
                .arg("--filename")
                .arg(default_save_as_path(current_path));
        }
    }
    command
        .arg("--file-filter")
        .arg("Wayscriber sessions | *.wayscriber-session *.session")
        .arg("--file-filter")
        .arg("All files | *");
    run_session_file_dialog_command(command, "zenity")
}

fn run_kdialog_session_file_dialog(
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
) -> Result<Option<SessionFileDialogResult>> {
    let mut command = Command::new("kdialog");
    match mode {
        SessionFileDialogMode::Open => {
            command.arg("--getopenfilename");
            command.arg(
                current_path
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(default_session_dir),
            );
        }
        SessionFileDialogMode::SaveAs => {
            command.arg("--getsavefilename");
            command.arg(default_save_as_path(current_path));
        }
    }
    command.arg("Wayscriber sessions (*.wayscriber-session *.session);;All files (*)");
    run_session_file_dialog_command(command, "kdialog")
}

fn run_session_file_dialog_command(
    mut command: Command,
    program: &'static str,
) -> Result<Option<SessionFileDialogResult>> {
    let output = match command.output() {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(anyhow!("failed to launch {program}: {err}")),
    };

    let selected = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from);
    if output.status.success() {
        return Ok(Some(match selected {
            Some(path) => SessionFileDialogResult::Selected(path),
            None => SessionFileDialogResult::Cancelled,
        }));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.trim().is_empty() {
        return Ok(Some(SessionFileDialogResult::Cancelled));
    }

    Err(anyhow!(
        "{program} session file chooser failed: {}",
        stderr.trim()
    ))
}

fn default_session_dir() -> PathBuf {
    crate::paths::home_dir().unwrap_or_else(std::env::temp_dir)
}

fn default_save_as_path(current_path: Option<&Path>) -> PathBuf {
    default_save_as_dir().join(save_as_file_name(current_path))
}

fn default_save_as_dir() -> PathBuf {
    let Some(home) = crate::paths::home_dir() else {
        return std::env::temp_dir();
    };
    let documents = home.join("Documents");
    if documents.is_dir() { documents } else { home }
}

fn save_as_file_name(current_path: Option<&Path>) -> String {
    let Some(current) = current_path
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
    else {
        return format!("session-copy.{SESSION_FILE_EXTENSION}");
    };
    let path = Path::new(current);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("session");
    format!("{stem}-copy.{SESSION_FILE_EXTENSION}")
}

fn ensure_save_as_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .is_some()
    {
        return path;
    }

    path.with_extension(SESSION_FILE_EXTENSION)
}

impl WaylandState {
    /// Returns a snapshot of the current input state for toolbar UI consumption.
    pub(in crate::backend::wayland) fn toolbar_snapshot(&self) -> ToolbarSnapshot {
        let hints = ToolbarBindingHints::from_input_state(&self.input_state);
        let hint_max = crate::onboarding::DRAWER_HINT_MAX;
        let show_drawer_hint = self.onboarding.state().drawer_hint_count < hint_max
            && !self.input_state.toolbar_drawer_open;
        let mut snapshot =
            ToolbarSnapshot::from_input_with_options(&self.input_state, hints, show_drawer_hint);
        populate_session_snapshot(&mut snapshot, self.session.options());
        snapshot
    }

    /// Applies an incoming toolbar event and schedules redraws as needed.
    pub(in crate::backend::wayland) fn handle_toolbar_event(
        &mut self,
        event: ToolbarEvent,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) {
        if self.handle_toolbar_session_event(&event, conn, qh) {
            return;
        }

        let policy = ToolbarEventPolicy::for_event(&event);
        for effect in &policy.pre_apply_effects {
            match effect {
                ToolbarPreApplyEffect::RecordDrawerHintShown => {
                    if record_drawer_hint_shown(self.onboarding.state_mut()) {
                        self.onboarding.save();
                    }
                }
            }
        }

        match (&policy.backend_route, &event) {
            (ToolbarBackendRoute::MoveTopToolbar, ToolbarEvent::MoveTopToolbar { x, y }) => {
                let inline_active = self.inline_toolbars_active();
                let coord_is_screen = inline_active;
                drag_log(format!(
                    "toolbar move event: kind=Top, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}",
                    *x, *y, coord_is_screen, inline_active
                ));
                self.begin_toolbar_move_drag(MoveDragKind::Top, (*x, *y), coord_is_screen);
                if coord_is_screen {
                    self.handle_toolbar_move_screen(MoveDragKind::Top, (*x, *y));
                } else {
                    self.handle_toolbar_move(MoveDragKind::Top, (*x, *y));
                }
                return;
            }
            (ToolbarBackendRoute::MoveSideToolbar, ToolbarEvent::MoveSideToolbar { x, y }) => {
                let inline_active = self.inline_toolbars_active();
                let coord_is_screen = inline_active;
                drag_log(format!(
                    "toolbar move event: kind=Side, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}",
                    *x, *y, coord_is_screen, inline_active
                ));
                self.begin_toolbar_move_drag(MoveDragKind::Side, (*x, *y), coord_is_screen);
                if coord_is_screen {
                    self.handle_toolbar_move_screen(MoveDragKind::Side, (*x, *y));
                } else {
                    self.handle_toolbar_move(MoveDragKind::Side, (*x, *y));
                }
                return;
            }
            (ToolbarBackendRoute::ApplyToInput, _)
            | (ToolbarBackendRoute::MoveTopToolbar, _)
            | (ToolbarBackendRoute::MoveSideToolbar, _) => {}
        }

        #[cfg(tablet)]
        let prev_thickness = self.input_state.current_thickness;
        #[cfg(tablet)]
        let thickness_event = policy.tablet_thickness_sensitive;

        if self.input_state.apply_toolbar_event(event) {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;

            #[cfg(tablet)]
            if thickness_event && self.sync_stylus_thickness_cache(prev_thickness) {
                if self.stylus_tip_down {
                    self.record_stylus_peak(self.input_state.current_thickness);
                } else {
                    self.stylus_peak_thickness = None;
                }
            }

            match policy.persistence {
                ToolbarPersistence::RuntimeOnly => {}
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::Toolbar) => {
                    self.save_toolbar_pin_config();
                }
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::Ui(target)) => {
                    self.save_toolbar_ui_config(target);
                }
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::History) => {
                    self.save_toolbar_history_config();
                }
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::ClickHighlight) => {
                    self.save_click_highlight_preferences();
                }
            }
        }
        if let Some(action) = self.input_state.take_pending_preset_action() {
            self.handle_preset_action(action);
        }
        if self.input_state.take_pending_copy_hex() {
            self.handle_copy_hex_color();
        }
        if self.input_state.take_pending_paste_hex() {
            self.handle_paste_hex_color();
        }
        self.drain_clipboard_requests();
        self.refresh_keyboard_interactivity();
    }

    fn handle_toolbar_session_event(
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
        match self.open_named_session_runtime(path) {
            Ok(report) => self.set_session_toolbar_info(format!(
                "Opened session {}",
                session_display_name(&report.opened_path)
            )),
            Err(err) => self.set_session_toolbar_error(format!("Open session failed: {err:#}")),
        }
    }

    fn handle_toolbar_save_session_as(
        &mut self,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) {
        let current_path = self.current_session_file_path();
        match self.choose_session_file_with_overlay_suppressed(
            SessionFileDialogMode::SaveAs,
            current_path.as_deref(),
            conn,
            qh,
        ) {
            Ok(Some(path)) => {
                let path = ensure_save_as_extension(path);
                match self
                    .save_named_session_as_runtime(&path, crate::session::SaveAsOverwrite::Deny)
                {
                    Ok(report) => self.set_session_toolbar_info(format!(
                        "Saved session as {}",
                        session_display_name(&report.saved_path)
                    )),
                    Err(err) => {
                        self.set_session_toolbar_error(format!("Save session failed: {err:#}"))
                    }
                }
            }
            Ok(None) => {}
            Err(err) => self.set_session_toolbar_error(format!("Save session failed: {err:#}")),
        }
    }

    fn handle_toolbar_clear_session(&mut self) {
        match self.clear_current_session_runtime() {
            Ok(report) => self.set_session_toolbar_info(format!(
                "Cleared session {}",
                session_display_name(&report.cleared_path)
            )),
            Err(err) => self.set_session_toolbar_error(format!("Clear session failed: {err:#}")),
        }
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

    #[cfg(tablet)]
    pub(in crate::backend::wayland) fn sync_stylus_thickness_cache(&mut self, prev: f64) -> bool {
        let cur = self.input_state.current_thickness;
        if (cur - prev).abs() <= f64::EPSILON {
            return false;
        }

        self.stylus_base_thickness = Some(cur);
        if self.stylus_tip_down {
            self.stylus_pressure_thickness = Some(cur);
        } else {
            self.stylus_pressure_thickness = None;
        }
        true
    }

    /// Records the maximum stylus thickness seen during the current stroke.
    #[cfg(tablet)]
    pub(in crate::backend::wayland) fn record_stylus_peak(&mut self, thickness: f64) {
        self.stylus_peak_thickness = Some(
            self.stylus_peak_thickness
                .map_or(thickness, |p| p.max(thickness)),
        );
    }

    /// Saves the current toolbar configuration to disk (pinned state, icon mode, section visibility).
    pub(super) fn save_toolbar_pin_config(&mut self) {
        self.config.ui.toolbar.layout_mode = self.input_state.toolbar_layout_mode;
        self.config.ui.toolbar.top_pinned = self.input_state.toolbar_top_pinned;
        self.config.ui.toolbar.side_pinned = self.input_state.toolbar_side_pinned;
        self.config.ui.toolbar.use_icons = self.input_state.toolbar_use_icons;
        self.config.ui.toolbar.show_more_colors = self.input_state.show_more_colors;
        self.config.ui.toolbar.show_actions_section = self.input_state.show_actions_section;
        self.config.ui.toolbar.show_actions_advanced = self.input_state.show_actions_advanced;
        self.config.ui.toolbar.show_zoom_actions = self.input_state.show_zoom_actions;
        self.config.ui.toolbar.show_pages_section = self.input_state.show_pages_section;
        self.config.ui.toolbar.show_boards_section = self.input_state.show_boards_section;
        self.config.ui.toolbar.show_presets = self.input_state.show_presets;
        self.config.ui.toolbar.show_step_section = self.input_state.show_step_section;
        self.config.ui.toolbar.show_text_controls = self.input_state.show_text_controls;
        self.config.ui.toolbar.context_aware_ui = self.input_state.context_aware_ui;
        self.config.ui.toolbar.show_settings_section = self.input_state.show_settings_section;
        self.config.ui.toolbar.show_delay_sliders = self.input_state.show_delay_sliders;
        self.config.ui.toolbar.show_marker_opacity_section =
            self.input_state.show_marker_opacity_section;
        self.config.ui.toolbar.show_preset_toasts = self.input_state.show_preset_toasts;
        self.config.ui.toolbar.show_tool_preview = persisted_tool_preview_value(
            self.input_state.show_tool_preview,
            self.input_state
                .presenter_restore
                .as_ref()
                .and_then(|restore| restore.show_tool_preview),
        );
        self.config.ui.toolbar.top_offset = self.data.toolbar_top_offset;
        self.config.ui.toolbar.top_offset_y = self.data.toolbar_top_offset_y;
        self.config.ui.toolbar.side_offset = self.data.toolbar_side_offset;
        self.config.ui.toolbar.side_offset_x = self.data.toolbar_side_offset_x;

        if let Err(err) = self.config.save() {
            log::warn!("Failed to save toolbar config: {}", err);
        } else {
            log::debug!("Saved toolbar config");
        }
    }

    fn save_toolbar_ui_config(&mut self, target: ToolbarUiPersistenceTarget) {
        apply_toolbar_ui_config_target(&mut self.config, &self.input_state, target);

        if let Err(err) = self.config.save() {
            log::warn!("Failed to save toolbar UI config: {}", err);
        } else {
            log::debug!("Saved toolbar UI config");
        }
    }

    fn save_toolbar_history_config(&mut self) {
        self.config.history.custom_section_enabled = self.input_state.custom_section_enabled;

        if let Err(err) = self.config.save() {
            log::warn!("Failed to save toolbar history config: {}", err);
        } else {
            log::debug!("Saved toolbar history config");
        }
    }

    pub(in crate::backend::wayland) fn save_click_highlight_preferences(&mut self) {
        if !(self.input_state.presenter_mode
            && self
                .input_state
                .presenter_mode_config
                .enable_click_highlight)
        {
            self.config.ui.click_highlight.enabled = self.input_state.click_highlight_enabled();
        }
        self.config.ui.click_highlight.show_on_highlight_tool =
            self.input_state.highlight_tool_ring_enabled();
        if let Err(err) = self.config.save() {
            log::warn!("Failed to persist click highlight preferences: {}", err);
        }
    }

    pub(in crate::backend::wayland) fn handle_preset_action(
        &mut self,
        action: crate::input::state::PresetAction,
    ) {
        match action {
            crate::input::state::PresetAction::Save { slot, preset } => {
                self.config.presets.set_slot(slot, Some(*preset));
                if let Err(err) = self.config.save() {
                    log::warn!("Failed to save preset slot {}: {}", slot, err);
                }
            }
            crate::input::state::PresetAction::Clear { slot } => {
                self.config.presets.set_slot(slot, None);
                if let Err(err) = self.config.save() {
                    log::warn!("Failed to clear preset slot {}: {}", slot, err);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ToolbarLayoutMode;
    use crate::draw::{Color, FontDescriptor};
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::{EraserMode, Tool};

    fn persistence_for(event: &ToolbarEvent) -> ToolbarPersistence {
        ToolbarEventPolicy::for_event(event).persistence
    }

    #[test]
    fn runtime_toolbar_events_do_not_directly_save_config() {
        let events = vec![
            ToolbarEvent::SelectTool(Tool::Line),
            ToolbarEvent::SetColor(Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            }),
            ToolbarEvent::SetThickness(8.0),
            ToolbarEvent::NudgeThickness(1.0),
            ToolbarEvent::SetMarkerOpacity(0.5),
            ToolbarEvent::NudgeMarkerOpacity(0.1),
            ToolbarEvent::SetEraserMode(EraserMode::Stroke),
            ToolbarEvent::SetFont(FontDescriptor::new(
                "Monospace".to_string(),
                "normal".to_string(),
                "italic".to_string(),
            )),
            ToolbarEvent::SetFontSize(44.0),
            ToolbarEvent::ToggleFill(true),
            ToolbarEvent::ApplyPreset(1),
            ToolbarEvent::OpenSession,
            ToolbarEvent::OpenRecentSession(std::path::PathBuf::from(
                "/tmp/recent.wayscriber-session",
            )),
            ToolbarEvent::SaveSessionAs,
            ToolbarEvent::ClearSession,
        ];

        for event in events {
            assert_eq!(
                persistence_for(&event),
                ToolbarPersistence::RuntimeOnly,
                "{event:?} should not directly save config"
            );
        }
    }

    #[test]
    fn toolbar_preference_events_save_toolbar_config() {
        let events = vec![
            ToolbarEvent::PinTopToolbar(true),
            ToolbarEvent::PinSideToolbar(true),
            ToolbarEvent::ToggleIconMode(true),
            ToolbarEvent::ToggleMoreColors(true),
            ToolbarEvent::ToggleActionsSection(true),
            ToolbarEvent::ToggleActionsAdvanced(true),
            ToolbarEvent::ToggleZoomActions(true),
            ToolbarEvent::TogglePagesSection(true),
            ToolbarEvent::ToggleBoardsSection(true),
            ToolbarEvent::TogglePresets(true),
            ToolbarEvent::ToggleStepSection(true),
            ToolbarEvent::ToggleTextControls(true),
            ToolbarEvent::ToggleContextAwareUi(true),
            ToolbarEvent::TogglePresetToasts(true),
            ToolbarEvent::ToggleToolPreview(true),
            ToolbarEvent::ToggleDelaySliders(true),
            ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Advanced),
        ];

        for event in events {
            assert_eq!(
                persistence_for(&event),
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::Toolbar),
                "{event:?} should save toolbar config"
            );
        }
    }

    #[test]
    fn ui_and_history_preference_events_save_their_own_config_targets() {
        let ui_events = [
            (
                ToolbarEvent::ToggleStatusBar(true),
                ToolbarUiPersistenceTarget::StatusBar,
            ),
            (
                ToolbarEvent::ToggleStatusBoardBadge(true),
                ToolbarUiPersistenceTarget::StatusBoardBadge,
            ),
            (
                ToolbarEvent::ToggleStatusPageBadge(true),
                ToolbarUiPersistenceTarget::StatusPageBadge,
            ),
            (
                ToolbarEvent::ToggleFloatingBadgeAlways(true),
                ToolbarUiPersistenceTarget::FloatingBadgeAlways,
            ),
        ];

        for (event, target) in ui_events {
            assert_eq!(
                persistence_for(&event),
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::Ui(target)),
                "{event:?} should save only its UI config field"
            );
        }

        assert_eq!(
            persistence_for(&ToolbarEvent::ToggleCustomSection(true)),
            ToolbarPersistence::Persist(ToolbarPersistenceTarget::History)
        );
    }

    #[test]
    fn toolbar_ui_config_target_save_leaves_sibling_fields_unchanged() {
        let mut config = crate::config::Config::default();
        config.ui.show_status_bar = true;
        config.ui.show_status_board_badge = false;
        config.ui.show_status_page_badge = true;
        config.ui.show_floating_badge_always = false;

        let mut input_state = make_test_input_state();
        input_state.show_status_bar = false;
        input_state.show_status_board_badge = true;
        input_state.show_status_page_badge = false;
        input_state.show_floating_badge_always = true;

        apply_toolbar_ui_config_target(
            &mut config,
            &input_state,
            ToolbarUiPersistenceTarget::StatusBoardBadge,
        );

        assert!(config.ui.show_status_bar);
        assert!(config.ui.show_status_board_badge);
        assert!(config.ui.show_status_page_badge);
        assert!(!config.ui.show_floating_badge_always);
    }

    #[test]
    fn click_highlight_toolbar_events_are_explicit_config_exceptions() {
        let events = vec![
            ToolbarEvent::ToggleAllHighlight(true),
            ToolbarEvent::SelectTool(Tool::Highlight),
            ToolbarEvent::ToggleHighlightToolRing(true),
        ];

        for event in events {
            assert_eq!(
                persistence_for(&event),
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::ClickHighlight),
                "{event:?} should save click-highlight config"
            );
        }
    }

    #[test]
    fn drawer_hint_pre_apply_effect_is_conditionally_recorded_below_max() {
        let mut state = OnboardingState {
            drawer_hint_count: crate::onboarding::DRAWER_HINT_MAX - 1,
            drawer_hint_shown: false,
            ..OnboardingState::default()
        };

        assert!(record_drawer_hint_shown(&mut state));
        assert_eq!(state.drawer_hint_count, crate::onboarding::DRAWER_HINT_MAX);
        assert!(state.drawer_hint_shown);
    }

    #[test]
    fn drawer_hint_pre_apply_effect_is_ignored_at_max() {
        let mut state = OnboardingState {
            drawer_hint_count: crate::onboarding::DRAWER_HINT_MAX,
            drawer_hint_shown: true,
            ..OnboardingState::default()
        };

        assert!(!record_drawer_hint_shown(&mut state));
        assert_eq!(state.drawer_hint_count, crate::onboarding::DRAWER_HINT_MAX);
        assert!(state.drawer_hint_shown);
    }

    fn failing_session_file_chooser(
        _mode: SessionFileDialogMode,
        _current_path: Option<&Path>,
    ) -> Result<Option<SessionFileDialogResult>> {
        Err(anyhow!("zenity failed"))
    }

    fn missing_session_file_chooser(
        _mode: SessionFileDialogMode,
        _current_path: Option<&Path>,
    ) -> Result<Option<SessionFileDialogResult>> {
        Ok(None)
    }

    fn selecting_session_file_chooser(
        _mode: SessionFileDialogMode,
        _current_path: Option<&Path>,
    ) -> Result<Option<SessionFileDialogResult>> {
        Ok(Some(SessionFileDialogResult::Selected(PathBuf::from(
            "/tmp/selected.wayscriber-session",
        ))))
    }

    #[test]
    fn session_file_chooser_falls_back_after_backend_error() {
        let selected = choose_session_file_from(
            SessionFileDialogMode::Open,
            None,
            &[failing_session_file_chooser, selecting_session_file_chooser],
        )
        .expect("fallback chooser should succeed");

        assert_eq!(
            selected,
            Some(PathBuf::from("/tmp/selected.wayscriber-session"))
        );
    }

    #[test]
    fn session_file_chooser_reports_errors_after_all_backends_fail() {
        let err = choose_session_file_from(
            SessionFileDialogMode::Open,
            None,
            &[failing_session_file_chooser, missing_session_file_chooser],
        )
        .expect_err("all chooser failures should be reported");

        assert!(format!("{err:#}").contains("zenity failed"));
    }

    #[test]
    fn default_session_save_as_path_uses_visible_dir_and_session_extension() {
        let path = default_save_as_path(Some(Path::new("/tmp/lecture.wayscriber-session")));

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("lecture-copy.wayscriber-session")
        );
    }

    #[test]
    fn save_as_file_name_normalizes_extensionless_auto_session_names() {
        assert_eq!(
            save_as_file_name(Some(Path::new(
                "session-wayland_1-DP_3_ASUSTek_COMPUTER_INC_PC32UCDP"
            ))),
            "session-wayland_1-DP_3_ASUSTek_COMPUTER_INC_PC32UCDP-copy.wayscriber-session"
        );
    }

    #[test]
    fn save_as_file_name_replaces_existing_extension_with_session_extension() {
        assert_eq!(
            save_as_file_name(Some(Path::new("lecture.session"))),
            "lecture-copy.wayscriber-session"
        );
    }

    #[test]
    fn save_as_dialog_selection_adds_session_extension_when_missing() {
        assert_eq!(
            ensure_save_as_extension(PathBuf::from("/tmp/lecture-copy")),
            PathBuf::from("/tmp/lecture-copy.wayscriber-session")
        );
    }

    #[test]
    fn save_as_dialog_selection_keeps_explicit_extension() {
        assert_eq!(
            ensure_save_as_extension(PathBuf::from("/tmp/lecture.session")),
            PathBuf::from("/tmp/lecture.session")
        );
    }

    #[test]
    fn tool_preview_config_preserves_presenter_mode_restore_value() {
        assert!(persisted_tool_preview_value(false, Some(true)));
        assert!(!persisted_tool_preview_value(false, Some(false)));
        assert!(persisted_tool_preview_value(true, None));
        assert!(!persisted_tool_preview_value(false, None));
    }
}
