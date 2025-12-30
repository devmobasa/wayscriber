use super::base::{
    DrawingState, InputState, PresetAction, UI_TOAST_DURATION_MS, UiToastKind, UiToastState,
    ZoomAction,
};
use crate::config::Action;
use crate::config::Config;
use crate::util::Rect;
use std::io::ErrorKind;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

impl InputState {
    /// Updates the cached pointer location.
    pub fn update_pointer_position(&mut self, x: i32, y: i32) {
        self.last_pointer_position = (x, y);
    }

    /// Updates the undo stack limit for subsequent actions.
    pub fn set_undo_stack_limit(&mut self, limit: usize) {
        self.undo_stack_limit = limit.max(1);
    }

    /// Updates screen dimensions after backend configuration.
    ///
    /// This should be called by the backend when it receives the actual
    /// screen dimensions from the display server.
    pub fn update_screen_dimensions(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Cancels the current text input session and restores any edited shape.
    pub(crate) fn cancel_text_input(&mut self) {
        self.cancel_text_edit();
        self.clear_text_preview_dirty();
        self.last_text_preview_bounds = None;
        self.text_wrap_width = None;
        self.state = DrawingState::Idle;
        self.needs_redraw = true;
    }

    /// Cancels any in-progress interaction without exiting the application.
    pub(crate) fn cancel_active_interaction(&mut self) {
        match &self.state {
            DrawingState::TextInput { .. } => {
                self.cancel_text_input();
            }
            DrawingState::PendingTextClick { .. } => {
                self.state = DrawingState::Idle;
            }
            DrawingState::Drawing { .. } => {
                self.clear_provisional_dirty();
                self.last_provisional_bounds = None;
                self.state = DrawingState::Idle;
                self.needs_redraw = true;
            }
            DrawingState::MovingSelection { snapshots, .. } => {
                self.restore_selection_from_snapshots(snapshots.clone());
                self.state = DrawingState::Idle;
            }
            DrawingState::Selecting { .. } => {
                self.clear_provisional_dirty();
                self.last_provisional_bounds = None;
                self.state = DrawingState::Idle;
                self.needs_redraw = true;
            }
            DrawingState::ResizingText {
                shape_id, snapshot, ..
            } => {
                self.restore_selection_from_snapshots(vec![(*shape_id, snapshot.clone())]);
                self.state = DrawingState::Idle;
            }
            DrawingState::Idle => {}
        }
    }

    /// Drains pending dirty rectangles for the current surface size.
    #[allow(dead_code)]
    pub fn take_dirty_regions(&mut self) -> Vec<Rect> {
        let width = self.screen_width.min(i32::MAX as u32) as i32;
        let height = self.screen_height.min(i32::MAX as u32) as i32;
        self.dirty_tracker.take_regions(width, height)
    }

    /// Look up an action for the given key and modifiers.
    pub(crate) fn find_action(&self, key_str: &str) -> Option<Action> {
        for (binding, action) in &self.action_map {
            if binding.matches(
                key_str,
                self.modifiers.ctrl,
                self.modifiers.shift,
                self.modifiers.alt,
            ) {
                return Some(*action);
            }
        }
        None
    }

    /// Adjusts the current font size by a delta, clamping to valid range.
    ///
    /// Font size is clamped to 8.0-72.0px range (same as config validation).
    pub fn adjust_font_size(&mut self, delta: f64) {
        self.current_font_size = (self.current_font_size + delta).clamp(8.0, 72.0);
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        log::debug!("Font size adjusted to {:.1}px", self.current_font_size);
    }

    /// Takes and clears any pending capture action.
    pub fn take_pending_capture_action(&mut self) -> Option<Action> {
        self.pending_capture_action.take()
    }

    /// Stores a capture action for retrieval by the backend.
    pub(crate) fn set_pending_capture_action(&mut self, action: Action) {
        self.pending_capture_action = Some(action);
    }

    /// Stores a zoom action for retrieval by the backend.
    pub(crate) fn request_zoom_action(&mut self, action: ZoomAction) {
        self.pending_zoom_action = Some(action);
    }

    /// Takes and clears any pending zoom action.
    pub fn take_pending_zoom_action(&mut self) -> Option<ZoomAction> {
        self.pending_zoom_action.take()
    }

    /// Takes and clears any pending preset save/clear action.
    pub fn take_pending_preset_action(&mut self) -> Option<PresetAction> {
        self.pending_preset_action.take()
    }

    /// Marks a frozen-mode toggle request for the backend.
    pub(crate) fn request_frozen_toggle(&mut self) {
        self.pending_frozen_toggle = true;
    }

    /// Returns and clears any pending frozen-mode toggle request.
    pub fn take_pending_frozen_toggle(&mut self) -> bool {
        let pending = self.pending_frozen_toggle;
        self.pending_frozen_toggle = false;
        pending
    }

    /// Updates the cached frozen-mode status and triggers a redraw when it changes.
    pub fn set_frozen_active(&mut self, active: bool) {
        if self.frozen_active != active {
            self.frozen_active = active;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Returns whether frozen mode is active.
    pub fn frozen_active(&self) -> bool {
        self.frozen_active
    }

    /// Updates cached zoom status and triggers a redraw when it changes.
    pub fn set_zoom_status(&mut self, active: bool, locked: bool, scale: f64) {
        let changed = self.zoom_active != active
            || self.zoom_locked != locked
            || (self.zoom_scale - scale).abs() > f64::EPSILON;
        if changed {
            self.zoom_active = active;
            self.zoom_locked = locked;
            self.zoom_scale = scale;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Returns whether zoom mode is active.
    pub fn zoom_active(&self) -> bool {
        self.zoom_active
    }

    /// Returns whether zoom view is locked.
    pub fn zoom_locked(&self) -> bool {
        self.zoom_locked
    }

    /// Returns the current zoom scale.
    pub fn zoom_scale(&self) -> f64 {
        self.zoom_scale
    }

    pub(crate) fn launch_configurator(&mut self) {
        let binary = std::env::var("WAYSCRIBER_CONFIGURATOR")
            .unwrap_or_else(|_| "wayscriber-configurator".to_string());

        match Command::new(&binary)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                log::info!(
                    "Launched wayscriber-configurator (binary: {binary}, pid: {})",
                    child.id()
                );
                self.should_exit = true;
            }
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    log::error!(
                        "Configurator not found (looked for '{binary}'). Install 'wayscriber-configurator' (Arch: yay -S wayscriber-configurator; deb/rpm users: grab the wayscriber-configurator package from the release page) or set WAYSCRIBER_CONFIGURATOR to its path."
                    );
                    self.set_ui_toast(
                        UiToastKind::Warning,
                        format!("Configurator not found: {binary}"),
                    );
                } else {
                    log::error!("Failed to launch wayscriber-configurator using '{binary}': {err}");
                    log::error!(
                        "Set WAYSCRIBER_CONFIGURATOR to override the executable path if needed."
                    );
                    self.set_ui_toast(
                        UiToastKind::Error,
                        "Failed to launch configurator (see logs).",
                    );
                }
            }
        }
    }

    pub(crate) fn set_ui_toast(&mut self, kind: UiToastKind, message: impl Into<String>) {
        self.ui_toast = Some(UiToastState {
            kind,
            message: message.into(),
            started: Instant::now(),
        });
        self.needs_redraw = true;
    }

    #[allow(dead_code)]
    pub(crate) fn set_capture_feedback(
        &mut self,
        saved_path: Option<&Path>,
        copied_to_clipboard: bool,
        open_folder_binding: Option<&str>,
    ) {
        let mut parts = Vec::new();
        self.last_capture_path = saved_path.map(|path| path.to_path_buf());
        if let Some(path) = saved_path {
            let mut saved = format!("Saved to {}", path.display());
            if let Some(binding) = open_folder_binding {
                saved.push_str(&format!(" ({binding} opens folder)"));
            }
            parts.push(saved);
        }

        if copied_to_clipboard {
            if saved_path.is_none() {
                parts.push("Clipboard only (no file saved)".to_string());
            }
            parts.push("Copied to clipboard".to_string());
        }

        if parts.is_empty() {
            parts.push("Screenshot captured".to_string());
        }

        self.set_ui_toast(UiToastKind::Info, parts.join(" | "));
    }

    pub fn advance_ui_toast(&mut self, now: Instant) -> bool {
        let duration = Duration::from_millis(UI_TOAST_DURATION_MS);
        let Some(toast) = &self.ui_toast else {
            return false;
        };
        if now.saturating_duration_since(toast.started) >= duration {
            self.ui_toast = None;
            return false;
        }
        true
    }

    /// Opens the most recent capture directory using the desktop default application.
    pub(crate) fn open_capture_folder(&mut self) {
        let Some(path) = self.last_capture_path.clone() else {
            self.set_ui_toast(UiToastKind::Warning, "No saved capture to open.");
            return;
        };

        let folder = if path.is_dir() {
            path
        } else if let Some(parent) = path.parent() {
            parent.to_path_buf()
        } else {
            self.set_ui_toast(UiToastKind::Warning, "Capture folder is unavailable.");
            return;
        };

        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "xdg-open"
        };

        let mut cmd = Command::new(opener);
        if cfg!(target_os = "windows") {
            cmd.args(["/C", "start", ""]).arg(&folder);
        } else {
            cmd.arg(&folder);
        }

        match cmd.spawn() {
            Ok(child) => {
                log::info!(
                    "Opened capture folder at {} (pid {})",
                    folder.display(),
                    child.id()
                );
                self.should_exit = true;
            }
            Err(err) => {
                log::error!(
                    "Failed to open capture folder at {}: {}",
                    folder.display(),
                    err
                );
                self.set_ui_toast(UiToastKind::Error, "Failed to open capture folder.");
            }
        }
    }

    /// Opens the primary config file using the desktop default application.
    pub(crate) fn open_config_file_default(&mut self) {
        let path = match Config::get_config_path() {
            Ok(p) => p,
            Err(err) => {
                log::error!("Unable to resolve config path: {}", err);
                return;
            }
        };

        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "xdg-open"
        };

        let mut cmd = Command::new(opener);
        if cfg!(target_os = "windows") {
            cmd.args(["/C", "start", ""]).arg(&path);
        } else {
            cmd.arg(&path);
        }

        match cmd.spawn() {
            Ok(child) => {
                log::info!(
                    "Opened config file at {} (pid {})",
                    path.display(),
                    child.id()
                );
                self.should_exit = true;
            }
            Err(err) => {
                log::error!("Failed to open config file at {}: {}", path.display(), err);
            }
        }
    }
}
