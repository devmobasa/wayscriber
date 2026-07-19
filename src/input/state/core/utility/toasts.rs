use super::super::base::{
    BLOCKED_ACTION_DURATION_MS, BlockedActionFeedback, InputState, PendingClipboardFallback,
    TEXT_EDIT_ENTRY_DURATION_MS, ToastAction, UI_TOAST_DURATION_MS, UiToastKind, UiToastState,
};
use crate::capture::{
    ImageOperationKind,
    file::{FileSaveConfig, save_screenshot},
};
use crate::domain::Action;
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
        self.ui_toast_bounds = None;
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
        self.ui_toast_bounds = None;
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
        self.ui_toast_bounds = None;
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
        let Some(toast) = self.ui_toast.as_ref() else {
            return (false, None);
        };

        if self.toast_contains(x, y) {
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

    pub(crate) fn toast_contains(&self, x: i32, y: i32) -> bool {
        self.ui_toast.is_some()
            && self.ui_toast_bounds.is_some_and(|(bx, by, bw, bh)| {
                let xf = x as f64;
                let yf = y as f64;
                xf >= bx && xf <= bx + bw && yf >= by && yf <= by + bh
            })
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
        operation: ImageOperationKind,
        exit_after_save: bool,
    ) {
        self.pending_clipboard_fallback = Some(PendingClipboardFallback {
            image_data,
            save_config,
            operation,
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
                log::info!(
                    "Saved pending {} to: {}",
                    fallback.operation.saved_log_label(),
                    path.display()
                );
                self.last_capture_path = Some(path.clone());
                if let Some(filename) = path.file_name() {
                    self.set_ui_toast(
                        UiToastKind::Info,
                        format!("Saved to {}", filename.to_string_lossy()),
                    );
                } else {
                    self.set_ui_toast(
                        UiToastKind::Info,
                        match fallback.operation {
                            ImageOperationKind::Screenshot => "Screenshot saved",
                            ImageOperationKind::CanvasExport => "Canvas exported",
                            ImageOperationKind::BoardPdfExport => "Board exported",
                            ImageOperationKind::AllBoardsPdfExport => "Boards exported",
                        },
                    );
                }
                // Exit if exit-after-capture was originally enabled
                if fallback.exit_after_save {
                    self.should_exit = true;
                }
            }
            Err(err) => {
                let message = fallback.operation.format_error(&err);
                log::error!(
                    "Failed to save pending {}: {}",
                    fallback.operation.saved_log_label(),
                    message
                );
                // Restore fallback so user can retry
                self.pending_clipboard_fallback = Some(fallback);
                self.set_ui_toast_with_action(
                    UiToastKind::Error,
                    format!("Save failed: {message}"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor, Shape};
    use crate::input::state::core::base::TextEditEntryFeedback;
    use crate::input::{ClickHighlightSettings, EraserMode};
    use crate::ui::toolbar::ToolbarEvent;

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        )
    }

    #[test]
    fn advance_ui_toast_clears_expired_toast_and_bounds() {
        let mut state = make_state();
        state.set_ui_toast_with_duration(UiToastKind::Info, "Hello", 10);
        state.ui_toast_bounds = Some((1.0, 2.0, 3.0, 4.0));
        let now = state.ui_toast.as_ref().unwrap().started + Duration::from_millis(10);

        assert!(!state.advance_ui_toast(now));
        assert!(state.ui_toast.is_none());
        assert!(state.ui_toast_bounds.is_none());
    }

    #[test]
    fn check_toast_click_returns_action_and_dismisses_inside_bounds() {
        let mut state = make_state();
        state.set_ui_toast_with_action(
            UiToastKind::Info,
            "Saved",
            "Open",
            Action::OpenCaptureFolder,
        );
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

        let (hit, action) = state.check_toast_click(50, 40);

        assert!(hit);
        assert_eq!(action, Some(Action::OpenCaptureFolder));
        assert!(state.ui_toast.is_none());
        assert!(state.ui_toast_bounds.is_none());
    }

    fn add_test_shape(state: &mut InputState) {
        state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 10,
            y: 10,
            w: 5,
            h: 5,
            fill: false,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 1.0,
        });
    }

    #[test]
    fn toolbar_clear_offers_a_two_second_undo_toast() {
        let mut state = make_state();
        add_test_shape(&mut state);

        assert!(state.apply_toolbar_event(ToolbarEvent::ClearCanvas { instant: false }));

        assert!(state.boards.active_frame().shapes.is_empty());
        assert!(
            state.boards.active_frame().undo_stack_len() > 0,
            "the toast's Undo? chip needs an undoable clear"
        );
        let toast = state.ui_toast.as_ref().expect("undo toast");
        assert_eq!(toast.kind, UiToastKind::Info);
        assert_eq!(toast.message, "Cleared");
        assert_eq!(toast.duration_ms, 2000, "short-lived action toast");
        let action = toast.action.as_ref().expect("undo action chip");
        assert_eq!(action.label, "Undo?");
        assert_eq!(action.action, Action::Undo);

        // Clicking inside the toast returns the attached Undo action.
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));
        assert_eq!(state.check_toast_click(50, 40), (true, Some(Action::Undo)));
    }

    #[test]
    fn instant_clear_skips_the_undo_toast() {
        let mut state = make_state();
        add_test_shape(&mut state);

        assert!(state.apply_toolbar_event(ToolbarEvent::ClearCanvas { instant: true }));

        assert!(state.boards.active_frame().shapes.is_empty());
        assert!(state.ui_toast.is_none(), "Shift+click clears silently");
    }

    #[test]
    fn empty_canvas_clear_shows_no_undo_toast() {
        let mut state = make_state();

        assert!(state.apply_toolbar_event(ToolbarEvent::ClearCanvas { instant: false }));

        assert!(
            state.ui_toast.is_none(),
            "nothing was cleared, so nothing to undo"
        );
    }

    #[test]
    fn toast_contains_reports_hit_without_dismissing() {
        let mut state = make_state();
        state.set_ui_toast(UiToastKind::Info, "Saved");
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

        assert!(state.toast_contains(50, 40));
        assert!(state.ui_toast.is_some());
        assert!(state.ui_toast_bounds.is_some());
    }

    #[test]
    fn replacing_toast_clears_stale_click_bounds() {
        let mut state = make_state();
        state.set_ui_toast(UiToastKind::Info, "Saved");
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

        state.set_ui_toast_with_action(
            UiToastKind::Warning,
            "Delete page?",
            "Confirm",
            Action::PageDelete,
        );

        assert!(state.ui_toast.is_some());
        assert!(state.ui_toast_bounds.is_none());
        assert!(!state.toast_contains(50, 40));
        assert_eq!(state.check_toast_click(50, 40), (false, None));
    }

    #[test]
    fn check_toast_click_ignores_clicks_outside_bounds() {
        let mut state = make_state();
        state.set_ui_toast(UiToastKind::Info, "Saved");
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

        let (hit, action) = state.check_toast_click(5, 5);

        assert!(!hit);
        assert_eq!(action, None);
        assert!(state.ui_toast.is_some());
    }

    #[test]
    fn save_pending_clipboard_to_file_without_pending_data_warns_and_triggers_feedback() {
        let mut state = make_state();

        state.save_pending_clipboard_to_file();

        let toast = state.ui_toast.as_ref().expect("warning toast");
        assert_eq!(toast.kind, UiToastKind::Warning);
        assert_eq!(toast.message, "No pending image to save");
        assert!(state.blocked_action_feedback.is_some());
    }

    #[test]
    fn canvas_clipboard_fallback_retry_failure_uses_canvas_wording() {
        let mut state = make_state();
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let not_a_directory = temp.path().join("not-a-directory");
        std::fs::write(&not_a_directory, b"file").expect("test fixture file");

        state.set_clipboard_fallback(
            vec![1, 2, 3],
            FileSaveConfig {
                save_directory: not_a_directory,
                filename_template: "canvas_fallback".to_string(),
                format: "png".to_string(),
            },
            ImageOperationKind::CanvasExport,
            false,
        );

        state.save_pending_clipboard_to_file();

        let toast = state.ui_toast.as_ref().expect("error toast");
        assert_eq!(toast.kind, UiToastKind::Error);
        assert!(
            toast.message.contains("Failed to save canvas export"),
            "unexpected toast: {}",
            toast.message
        );
        assert!(
            !toast.message.to_lowercase().contains("screenshot"),
            "canvas fallback failure should not mention screenshot: {}",
            toast.message
        );
        assert!(state.pending_clipboard_fallback.is_some());
        assert!(state.blocked_action_feedback.is_some());
    }

    #[test]
    fn advance_text_edit_entry_feedback_clears_expired_feedback() {
        let mut state = make_state();
        state.text_edit_entry_feedback = Some(TextEditEntryFeedback {
            started: Instant::now(),
        });
        let now = state.text_edit_entry_feedback.as_ref().unwrap().started
            + Duration::from_millis(TEXT_EDIT_ENTRY_DURATION_MS);

        assert!(!state.advance_text_edit_entry_feedback(now));
        assert!(state.text_edit_entry_feedback.is_none());
    }
}
