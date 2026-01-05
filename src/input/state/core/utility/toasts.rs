use super::super::base::{
    InputState, ToastAction, UI_TOAST_DURATION_MS, UiToastKind, UiToastState,
};
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

    /// Check if a click at (x, y) hits the toast. If so and the toast has an action,
    /// returns the action and dismisses the toast.
    #[allow(dead_code)] // Called from WaylandState pointer release handler
    pub(crate) fn check_toast_click(&mut self, x: i32, y: i32) -> Option<Action> {
        let bounds = self.ui_toast_bounds?;
        let toast = self.ui_toast.as_ref()?;

        // Check if click is within toast bounds
        let (bx, by, bw, bh) = bounds;
        let xf = x as f64;
        let yf = y as f64;
        if xf >= bx && xf <= bx + bw && yf >= by && yf <= by + bh {
            // Click is within toast
            if let Some(ref action) = toast.action {
                let result = action.action;
                // Dismiss the toast
                self.ui_toast = None;
                self.ui_toast_bounds = None;
                self.needs_redraw = true;
                return Some(result);
            }
        }
        None
    }
}
