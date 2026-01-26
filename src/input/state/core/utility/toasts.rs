use super::super::base::{
    BLOCKED_ACTION_DURATION_MS, BlockedActionFeedback, InputState, PendingClipboardFallback,
    TEXT_EDIT_ENTRY_DURATION_MS, ToastAction, UI_TOAST_DURATION_MS, UiToastKind, UiToastState,
};
use crate::capture::file::{FileSaveConfig, save_screenshot};
use crate::config::keybindings::Action;
use std::path::Path;
use std::time::{Duration, Instant};

impl InputState {
    pub(crate) fn set_ui_toast(&mut self, kind: UiToastKind, message: impl Into<String>) {
        self.set_ui_toast_with_duration(kind, message, UI_TOAST_DURATION_MS);
    }

    pub(crate) fn set_ui_toast_with_duration(
        &mut self,
        kind: UiToastKind,
        message: impl Into<String>,
        duration_ms: u64,
    ) {
        self.ui_toast = Some(UiToastState {
            kind,
            message: message.into(),
            started: Instant::now(),
            duration_ms,
            action: None,
        });
        self.needs_redraw = true;
    }

    /// Set a toast with a clickable action. Clicking the toast triggers the action.
    pub(crate) fn set_ui_toast_with_action(
        &mut self,
        kind: UiToastKind,
        message: impl Into<String>,
        action_label: impl Into<String>,
        action: Action,
    ) {
        self.ui_toast = Some(UiToastState {
            kind,
            message: message.into(),
            started: Instant::now(),
            duration_ms: UI_TOAST_DURATION_MS,
            action: Some(ToastAction {
                label: action_label.into(),
                action,
            }),
        });
        self.needs_redraw = true;
    }

    pub(crate) fn set_ui_toast_with_action_and_duration(
        &mut self,
        kind: UiToastKind,
        message: impl Into<String>,
        action_label: impl Into<String>,
        action: Action,
        duration_ms: u64,
    ) {
        self.ui_toast = Some(UiToastState {
            kind,
            message: message.into(),
            started: Instant::now(),
            duration_ms,
            action: Some(ToastAction {
                label: action_label.into(),
                action,
            }),
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
        let Some(toast) = &self.ui_toast else {
            return false;
        };
        let duration = Duration::from_millis(toast.duration_ms);
        if now.saturating_duration_since(toast.started) >= duration {
            self.ui_toast = None;
            self.ui_toast_bounds = None;
            return false;
        }
        true
    }

    /// Check if a click at (x, y) hits the toast. If so, dismisses it and returns
    /// whether it was hit plus any associated action.
    #[allow(dead_code)] // Called from WaylandState pointer release handler
    pub(crate) fn check_toast_click(&mut self, x: i32, y: i32) -> (bool, Option<Action>) {
        let Some(bounds) = self.ui_toast_bounds else {
            return (false, None);
        };
        let Some(toast) = self.ui_toast.as_ref() else {
            return (false, None);
        };

        // Check if click is within toast bounds
        let (bx, by, bw, bh) = bounds;
        let xf = x as f64;
        let yf = y as f64;
        if xf >= bx && xf <= bx + bw && yf >= by && yf <= by + bh {
            // Click is within toast
            let action = toast.action.as_ref().map(|action| action.action);
            // Dismiss the toast
            self.ui_toast = None;
            self.ui_toast_bounds = None;
            self.needs_redraw = true;
            return (true, action);
        }
        (false, None)
    }

    /// Trigger the blocked action visual feedback (red flash on screen edges).
    pub(crate) fn trigger_blocked_feedback(&mut self) {
        self.blocked_action_feedback = Some(BlockedActionFeedback {
            started: Instant::now(),
        });
        self.needs_redraw = true;
    }

    /// Advance the blocked action feedback animation. Returns true if still active.
    pub fn advance_blocked_feedback(&mut self, now: Instant) -> bool {
        let Some(feedback) = &self.blocked_action_feedback else {
            return false;
        };
        let duration = Duration::from_millis(BLOCKED_ACTION_DURATION_MS);
        if now.saturating_duration_since(feedback.started) >= duration {
            self.blocked_action_feedback = None;
            return false;
        }
        true
    }

    /// Get the progress (0.0 to 1.0) of the blocked action feedback animation.
    pub fn blocked_feedback_progress(&self) -> Option<f64> {
        let feedback = self.blocked_action_feedback.as_ref()?;
        let elapsed = Instant::now()
            .saturating_duration_since(feedback.started)
            .as_millis() as f64;
        let total = BLOCKED_ACTION_DURATION_MS as f64;
        Some((elapsed / total).min(1.0))
    }

    /// Store image data for clipboard fallback (when clipboard copy fails).
    /// Used by wayland backend when capture clipboard copy fails.
    #[allow(dead_code)]
    pub(crate) fn set_clipboard_fallback(
        &mut self,
        image_data: Vec<u8>,
        save_config: FileSaveConfig,
        exit_after_save: bool,
    ) {
        self.pending_clipboard_fallback = Some(PendingClipboardFallback {
            image_data,
            save_config,
            exit_after_save,
        });
    }

    /// Save pending clipboard fallback image to file.
    /// On success, clears the fallback and exits if exit-after-capture was enabled.
    /// On error, retains it for retry.
    pub(crate) fn save_pending_clipboard_to_file(&mut self) {
        let Some(fallback) = self.pending_clipboard_fallback.take() else {
            self.set_ui_toast(UiToastKind::Warning, "No pending image to save");
            self.trigger_blocked_feedback();
            return;
        };

        match save_screenshot(&fallback.image_data, &fallback.save_config) {
            Ok(path) => {
                log::info!("Saved pending screenshot to: {}", path.display());
                self.last_capture_path = Some(path.clone());
                if let Some(filename) = path.file_name() {
                    self.set_ui_toast(
                        UiToastKind::Info,
                        format!("Saved to {}", filename.to_string_lossy()),
                    );
                } else {
                    self.set_ui_toast(UiToastKind::Info, "Screenshot saved");
                }
                // Exit if exit-after-capture was originally enabled
                if fallback.exit_after_save {
                    self.should_exit = true;
                }
            }
            Err(err) => {
                log::error!("Failed to save pending screenshot: {}", err);
                // Restore fallback so user can retry
                self.pending_clipboard_fallback = Some(fallback);
                self.set_ui_toast_with_action(
                    UiToastKind::Error,
                    format!("Save failed: {}", err),
                    "Retry",
                    Action::SavePendingToFile,
                );
                self.trigger_blocked_feedback();
            }
        }
    }

    /// Advance the text edit entry feedback animation. Returns true if still active.
    pub fn advance_text_edit_entry_feedback(&mut self, now: Instant) -> bool {
        let Some(feedback) = &self.text_edit_entry_feedback else {
            return false;
        };
        let duration = Duration::from_millis(TEXT_EDIT_ENTRY_DURATION_MS);
        if now.saturating_duration_since(feedback.started) >= duration {
            self.text_edit_entry_feedback = None;
            return false;
        }
        true
    }

    /// Get the progress (0.0 to 1.0) of the text edit entry animation.
    pub fn text_edit_entry_progress(&self) -> Option<f64> {
        let feedback = self.text_edit_entry_feedback.as_ref()?;
        let elapsed = Instant::now()
            .saturating_duration_since(feedback.started)
            .as_millis() as f64;
        let total = TEXT_EDIT_ENTRY_DURATION_MS as f64;
        Some((elapsed / total).min(1.0))
    }
}
