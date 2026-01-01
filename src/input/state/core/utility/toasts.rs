use super::super::base::{InputState, UI_TOAST_DURATION_MS, UiToastKind, UiToastState};
use std::path::Path;
use std::time::{Duration, Instant};

impl InputState {
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
}
