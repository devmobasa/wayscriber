use super::super::base::{
    CompositorCapabilities, InputState, Toast, ToastCommand, ToastPress, ToastPriority,
    ToastPushOutcome, UiToastState,
};
use super::super::feedback::ToastBounds;
use crate::capture::{
    ImageOperationKind,
    file::{FileSaveConfig, save_screenshot},
};
use crate::domain::Action;
use std::path::Path;
use std::time::Instant;

impl InputState {
    /// Push a toast into the priority queue.
    pub(crate) fn push_toast(
        &mut self,
        priority: ToastPriority,
        key: &'static str,
        toast: Toast,
    ) -> ToastPushOutcome {
        let outcome = self.feedback.push(priority, key, toast, Instant::now());
        if outcome.changed_active() {
            self.needs_redraw = true;
        }
        outcome
    }

    pub(crate) fn toasts_idle(&self) -> bool {
        self.feedback.idle()
    }

    pub(crate) fn active_toast(&self) -> Option<&UiToastState> {
        self.feedback.active()
    }

    pub(crate) fn has_active_toast(&self) -> bool {
        self.feedback.active().is_some()
    }

    pub(crate) fn command_palette_toast_duration_ms(&self) -> u64 {
        self.feedback.command_palette_toast_duration_ms()
    }

    pub(crate) fn set_command_palette_toast_duration_ms(&mut self, duration_ms: u64) {
        self.feedback
            .set_command_palette_toast_duration_ms(duration_ms);
    }

    pub(crate) fn set_toast_geometry(
        &mut self,
        bounds: Option<ToastBounds>,
        action_bounds: [Option<ToastBounds>; 2],
    ) {
        self.feedback.set_geometry(bounds, action_bounds);
    }

    pub(crate) fn remove_matching_toasts(
        &mut self,
        should_remove: impl FnMut(&'static str, Option<Action>) -> bool,
    ) -> bool {
        let active_removed = self.feedback.remove_matching(should_remove);
        if active_removed {
            self.needs_redraw = true;
        }
        active_removed
    }

    #[cfg(test)]
    pub(crate) fn test_toast_count(&self) -> usize {
        self.feedback.toast_count()
    }

    #[cfg(test)]
    pub(crate) fn test_pending_toast_count(&self) -> usize {
        self.feedback.pending_toast_count()
    }

    #[cfg(test)]
    pub(crate) fn test_active_toast_message(&self) -> Option<&str> {
        self.feedback.active().map(|toast| toast.message.as_str())
    }

    #[cfg(test)]
    pub(crate) fn test_active_toast_key(&self) -> Option<&'static str> {
        self.feedback.active().map(|toast| toast.key)
    }

    #[cfg(test)]
    pub(crate) fn test_toast_geometry(&self) -> Option<ToastBounds> {
        self.feedback.geometry()
    }

    #[cfg(test)]
    pub(crate) fn test_blocked_feedback_active(&self) -> bool {
        self.feedback.blocked_action_active()
    }

    #[allow(dead_code)]
    pub(crate) fn set_capture_feedback(
        &mut self,
        saved_path: Option<&Path>,
        copied_to_clipboard: bool,
        open_folder_binding: Option<&str>,
    ) {
        let mut parts = Vec::new();
        self.set_last_capture_path(saved_path.map(|path| path.to_path_buf()));
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

        self.push_toast(
            ToastPriority::Info,
            "capture.feedback",
            Toast::info(parts.join(" | ")),
        );
    }

    pub fn advance_ui_toast(&mut self, now: Instant) -> bool {
        let before = self.feedback.active().map(|toast| toast.activation_id);
        let still_showing = self.feedback.advance(now);
        let after = self.feedback.active().map(|toast| toast.activation_id);
        if before != after {
            self.needs_redraw = true;
        }
        still_showing
    }

    pub(crate) fn toast_press_at(&self, x: i32, y: i32) -> Option<ToastPress> {
        self.feedback.press_at(x, y)
    }

    pub(crate) fn resolve_toast_release(
        &mut self,
        pressed: ToastPress,
        x: i32,
        y: i32,
    ) -> (bool, Option<ToastCommand>) {
        let result = self.feedback.release_at(pressed, x, y, Instant::now());
        if result.0 {
            self.needs_redraw = true;
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn toast_contains(&self, x: i32, y: i32) -> bool {
        self.feedback.contains(x, y)
    }

    pub(crate) fn note_capability_toast(&mut self, caps: CompositorCapabilities) -> Option<String> {
        self.feedback.note_capability_toast(caps)
    }

    pub(crate) fn trigger_blocked_feedback(&mut self) {
        self.feedback.trigger_blocked_action(Instant::now());
        self.needs_redraw = true;
    }

    pub fn advance_blocked_feedback(&mut self, now: Instant) -> bool {
        self.feedback.advance_blocked_action(now)
    }

    pub fn blocked_feedback_progress(&self) -> Option<f64> {
        self.feedback.blocked_action_progress(Instant::now())
    }

    /// Request overlay exit that must not be deferred by XDG stay-mode focus loss.
    pub(crate) fn request_explicit_exit(&mut self) {
        self.explicit_exit_requested = true;
        self.should_exit = true;
    }

    /// Take and clear the explicit-exit bit set by [`Self::request_explicit_exit`].
    pub(crate) fn take_explicit_exit_requested(&mut self) -> bool {
        let was_requested = self.explicit_exit_requested;
        self.explicit_exit_requested = false;
        was_requested
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
        self.selection_clipboard.set_pending_image_fallback(
            image_data,
            save_config,
            operation,
            exit_after_save,
        );
    }

    /// Save pending clipboard fallback image to file.
    /// On success, clears the fallback and exits if exit-after-capture was enabled.
    /// On error, retains it for retry.
    pub(crate) fn save_pending_clipboard_to_file(&mut self) {
        let Some(fallback) = self.selection_clipboard.take_pending_image_fallback() else {
            self.push_toast(
                ToastPriority::Info,
                "capture.save",
                Toast::warning("No pending image to save"),
            );
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
                self.set_last_capture_path(Some(path.clone()));
                if let Some(filename) = path.file_name() {
                    self.push_toast(
                        ToastPriority::Info,
                        "capture.save",
                        Toast::info(format!("Saved to {}", filename.to_string_lossy())),
                    );
                } else {
                    self.push_toast(
                        ToastPriority::Info,
                        "capture.save",
                        Toast::info(match fallback.operation {
                            ImageOperationKind::Screenshot => "Screenshot saved",
                            ImageOperationKind::CanvasExport => "Canvas exported",
                            ImageOperationKind::BoardPdfExport => "Board exported",
                            ImageOperationKind::AllBoardsPdfExport => "Boards exported",
                        }),
                    );
                }
                // Exit if exit-after-capture was originally enabled
                if fallback.exit_after_save {
                    self.request_explicit_exit();
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
                self.selection_clipboard
                    .restore_pending_image_fallback(fallback);
                self.push_toast(
                    ToastPriority::Critical,
                    "capture.save",
                    Toast::error(format!("Save failed: {message}"))
                        .action("Retry", Action::SavePendingToFile),
                );
                self.trigger_blocked_feedback();
            }
        }
    }

    /// Advance the text edit entry feedback animation. Returns true if still active.
    pub fn advance_text_edit_entry_feedback(&mut self, now: Instant) -> bool {
        self.text_editing.expire_edit_entry_feedback(now)
    }

    /// Get the progress (0.0 to 1.0) of the text edit entry animation.
    pub fn text_edit_entry_progress(&self) -> Option<f64> {
        self.text_editing.edit_entry_progress(Instant::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeybindingsConfig;
    use crate::domain::OnboardingTip;
    use crate::draw::{Color, Shape};
    use crate::input::state::core::base::UiToastKind;
    use crate::input::state::core::text_editing::{
        TEXT_EDIT_ENTRY_DURATION_MS, TextEditEntryFeedback,
    };

    use crate::ui::toolbar::ToolbarEvent;
    use std::time::Duration;

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let _action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        crate::input::state::test_support::make_test_input_state()
    }

    #[test]
    fn advance_ui_toast_clears_expired_toast_and_bounds() {
        let mut state = make_state();
        state.push_toast(
            ToastPriority::Info,
            "test",
            Toast::info("Hello").duration_ms(10),
        );
        state.set_toast_geometry(Some((1.0, 2.0, 3.0, 4.0)), [None, None]);
        let now = state.active_toast().unwrap().started + Duration::from_millis(10);

        assert!(!state.advance_ui_toast(now));
        assert!(state.active_toast().is_none());
        assert!(state.test_toast_geometry().is_none());
    }

    #[test]
    fn advance_ui_toast_promotes_queued_toast_when_active_expires() {
        let mut state = make_state();
        state.push_toast(ToastPriority::Info, "first", Toast::info("First"));
        state.push_toast(ToastPriority::Info, "second", Toast::info("Second"));
        state.set_toast_geometry(Some((1.0, 2.0, 3.0, 4.0)), [None, None]);
        state.needs_redraw = false;
        let now = state.active_toast().unwrap().started
            + Duration::from_millis(state.active_toast().unwrap().duration_ms);

        assert!(state.advance_ui_toast(now), "queued toast keeps showing");
        let toast = state.active_toast().expect("promoted toast");
        assert_eq!(toast.message, "Second");
        assert!(
            state.test_toast_geometry().is_none(),
            "stale bounds cleared"
        );
        assert!(state.needs_redraw);
    }

    #[test]
    fn toast_release_returns_action_and_dismisses_inside_bounds() {
        let mut state = make_state();
        state.push_toast(
            ToastPriority::Action,
            "test",
            Toast::info("Saved").action("Open", Action::OpenCaptureFolder),
        );
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);

        let pressed = state.toast_press_at(50, 40).expect("toast press");
        let (hit, action) = state.resolve_toast_release(pressed, 50, 40);

        assert!(hit);
        assert_eq!(
            action,
            Some(ToastCommand::Dispatch(Action::OpenCaptureFolder))
        );
        assert!(state.active_toast().is_none());
        assert!(state.test_toast_geometry().is_none());
    }

    #[test]
    fn two_action_toast_dispatches_only_the_chip_pressed_and_released() {
        let mut state = make_state();
        state.push_toast(
            ToastPriority::Hint,
            "tip",
            Toast::info("Try the board picker")
                .command(
                    "Got it",
                    ToastCommand::AcknowledgeTip {
                        tip: OnboardingTip::StatusBar,
                        then: None,
                    },
                )
                .secondary_command(
                    "Tip settings…",
                    ToastCommand::AcknowledgeTip {
                        tip: OnboardingTip::StatusBar,
                        then: Some(Action::OpenConfiguratorOnboardingHints),
                    },
                ),
        );
        state.set_toast_geometry(
            Some((10.0, 20.0, 220.0, 40.0)),
            [
                Some((120.0, 24.0, 44.0, 28.0)),
                Some((170.0, 24.0, 56.0, 28.0)),
            ],
        );

        let pressed = state.toast_press_at(190, 38).expect("secondary chip press");
        let (hit, action) = state.resolve_toast_release(pressed, 190, 38);

        assert!(hit);
        assert_eq!(
            action,
            Some(ToastCommand::AcknowledgeTip {
                tip: OnboardingTip::StatusBar,
                then: Some(Action::OpenConfiguratorOnboardingHints),
            })
        );
        assert!(state.active_toast().is_none());
    }

    #[test]
    fn two_action_toast_body_dismisses_without_dispatching_a_chip() {
        let mut state = make_state();
        state.push_toast(
            ToastPriority::Hint,
            "tip",
            Toast::info("Try the board picker")
                .command(
                    "Got it",
                    ToastCommand::AcknowledgeTip {
                        tip: OnboardingTip::StatusBar,
                        then: None,
                    },
                )
                .secondary_command(
                    "Tip settings…",
                    ToastCommand::AcknowledgeTip {
                        tip: OnboardingTip::StatusBar,
                        then: Some(Action::OpenConfiguratorOnboardingHints),
                    },
                ),
        );
        state.set_toast_geometry(
            Some((10.0, 20.0, 220.0, 40.0)),
            [
                Some((120.0, 24.0, 44.0, 28.0)),
                Some((170.0, 24.0, 56.0, 28.0)),
            ],
        );

        let pressed = state.toast_press_at(50, 38).expect("toast body press");
        let (hit, action) = state.resolve_toast_release(pressed, 50, 38);

        assert!(hit);
        assert_eq!(action, None);
        assert!(state.active_toast().is_none());
    }

    #[test]
    fn two_action_toast_does_not_retarget_between_chips_on_release() {
        let mut state = make_state();
        state.push_toast(
            ToastPriority::Hint,
            "tip",
            Toast::info("Try the board picker")
                .command(
                    "Got it",
                    ToastCommand::AcknowledgeTip {
                        tip: OnboardingTip::StatusBar,
                        then: None,
                    },
                )
                .secondary_command(
                    "Tip settings…",
                    ToastCommand::AcknowledgeTip {
                        tip: OnboardingTip::StatusBar,
                        then: Some(Action::OpenConfiguratorOnboardingHints),
                    },
                ),
        );
        state.set_toast_geometry(
            Some((10.0, 20.0, 220.0, 40.0)),
            [
                Some((120.0, 24.0, 44.0, 28.0)),
                Some((170.0, 24.0, 56.0, 28.0)),
            ],
        );

        let pressed = state.toast_press_at(140, 38).expect("primary chip press");
        let (hit, action) = state.resolve_toast_release(pressed, 190, 38);

        assert!(!hit);
        assert_eq!(action, None);
        assert!(
            state.active_toast().is_some(),
            "mismatched release keeps the toast"
        );
    }

    #[test]
    fn toast_release_promotes_next_queued_toast() {
        let mut state = make_state();
        state.push_toast(
            ToastPriority::Action,
            "confirm",
            Toast::info("Delete page?").action("Confirm", Action::PageDelete),
        );
        state.push_toast(ToastPriority::Info, "info", Toast::info("Later"));
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);

        let pressed = state.toast_press_at(50, 40).expect("toast press");
        let (hit, action) = state.resolve_toast_release(pressed, 50, 40);

        assert!(hit);
        assert_eq!(action, Some(ToastCommand::Dispatch(Action::PageDelete)));
        let promoted = state.active_toast().expect("queued toast promoted");
        assert_eq!(promoted.message, "Later");
        assert!(state.test_toast_geometry().is_none());
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
        let toast = state.active_toast().expect("undo toast");
        assert_eq!(toast.kind, UiToastKind::Info);
        assert_eq!(toast.message, "Cleared");
        assert_eq!(toast.duration_ms, 2000, "short-lived action toast");
        let action = toast.action.as_ref().expect("undo action chip");
        assert_eq!(action.label, "Undo?");
        assert_eq!(action.dispatch_action(), Some(Action::Undo));

        // Clicking inside the toast returns the attached Undo action.
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);
        let pressed = state.toast_press_at(50, 40).expect("toast press");
        assert_eq!(
            state.resolve_toast_release(pressed, 50, 40),
            (true, Some(ToastCommand::Dispatch(Action::Undo)))
        );
    }

    #[test]
    fn instant_clear_skips_the_undo_toast() {
        let mut state = make_state();
        add_test_shape(&mut state);

        assert!(state.apply_toolbar_event(ToolbarEvent::ClearCanvas { instant: true }));

        assert!(state.boards.active_frame().shapes.is_empty());
        assert!(
            state.active_toast().is_none(),
            "Shift+click clears silently"
        );
    }

    #[test]
    fn empty_canvas_clear_shows_no_undo_toast() {
        let mut state = make_state();

        assert!(state.apply_toolbar_event(ToolbarEvent::ClearCanvas { instant: false }));

        assert!(
            state.active_toast().is_none(),
            "nothing was cleared, so nothing to undo"
        );
    }

    #[test]
    fn toast_contains_reports_hit_without_dismissing() {
        let mut state = make_state();
        state.push_toast(ToastPriority::Info, "test", Toast::info("Saved"));
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);

        assert!(state.toast_contains(50, 40));
        assert!(state.active_toast().is_some());
        assert!(state.test_toast_geometry().is_some());
    }

    #[test]
    fn preempting_toast_clears_stale_click_bounds() {
        let mut state = make_state();
        state.push_toast(ToastPriority::Info, "info", Toast::info("Saved"));
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);

        // Action priority preempts the plain info toast.
        let outcome = state.push_toast(
            ToastPriority::Action,
            "confirm",
            Toast::warning("Delete page?").action("Confirm", Action::PageDelete),
        );

        assert_eq!(outcome, ToastPushOutcome::Displayed);
        let toast = state.active_toast().expect("preempting toast visible");
        assert_eq!(toast.message, "Delete page?");
        assert!(state.test_toast_geometry().is_none());
        assert!(!state.toast_contains(50, 40));
        let stale_press = ToastPress::body(0);
        assert_eq!(
            state.resolve_toast_release(stale_press, 50, 40),
            (false, None)
        );
    }

    #[test]
    fn same_key_update_keeps_single_toast() {
        let mut state = make_state();
        state.push_toast(ToastPriority::Info, "board.switch", Toast::info("Board 2"));
        let outcome = state.push_toast(ToastPriority::Info, "board.switch", Toast::info("Board 3"));

        assert_eq!(outcome, ToastPushOutcome::UpdatedActive);
        assert_eq!(state.active_toast().unwrap().message, "Board 3");
        assert!(state.toasts_idle() || state.active_toast().is_some());
        assert!(
            state.test_pending_toast_count() == 0,
            "no stacking for spam producers"
        );
    }

    #[test]
    fn hints_only_show_when_toasts_idle() {
        let mut state = make_state();
        state.push_toast(ToastPriority::Info, "info", Toast::info("Busy"));
        assert!(!state.toasts_idle());

        let outcome = state.push_toast(ToastPriority::Hint, "hint", Toast::info("Press F1"));
        assert_eq!(outcome, ToastPushOutcome::HintYielded);
        assert!(!outcome.accepted());
        assert_eq!(state.active_toast().unwrap().message, "Busy");

        // Once idle again, the hint is accepted.
        let now = state.active_toast().unwrap().started
            + Duration::from_millis(state.active_toast().unwrap().duration_ms);
        state.advance_ui_toast(now);
        assert!(state.toasts_idle());
        let outcome = state.push_toast(ToastPriority::Hint, "hint", Toast::info("Press F1"));
        assert_eq!(outcome, ToastPushOutcome::Displayed);
    }

    #[test]
    fn toast_release_ignores_releases_outside_bounds() {
        let mut state = make_state();
        state.push_toast(ToastPriority::Info, "test", Toast::info("Saved"));
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);

        let pressed = state.toast_press_at(50, 40).expect("toast press");
        let (hit, action) = state.resolve_toast_release(pressed, 5, 5);

        assert!(!hit);
        assert_eq!(action, None);
        assert!(state.active_toast().is_some());
    }

    #[test]
    fn toast_release_cannot_retarget_after_queue_promotion() {
        let mut state = make_state();
        state.push_toast(
            ToastPriority::Action,
            "first",
            Toast::info("Open folder?")
                .duration_ms(10)
                .action("Open", Action::OpenCaptureFolder),
        );
        state.push_toast(
            ToastPriority::Action,
            "destructive",
            Toast::warning("Delete page?").action("Delete", Action::PageDelete),
        );
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);
        let pressed = state.toast_press_at(50, 40).expect("first toast press");
        let expiry = state.active_toast().expect("first toast").started + Duration::from_millis(10);

        assert!(state.advance_ui_toast(expiry));
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);
        assert_eq!(
            state.active_toast().expect("promoted toast").message,
            "Delete page?"
        );

        assert_eq!(
            state.resolve_toast_release(pressed, 50, 40),
            (false, None),
            "release must not dispatch the promoted destructive toast"
        );
        assert_eq!(
            state
                .active_toast()
                .expect("promoted toast remains")
                .message,
            "Delete page?"
        );
    }

    #[test]
    fn toast_release_cannot_retarget_after_same_key_update() {
        let mut state = make_state();
        state.push_toast(
            ToastPriority::Action,
            "confirm",
            Toast::info("Undo clear?").action("Undo", Action::Undo),
        );
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);
        let pressed = state.toast_press_at(50, 40).expect("original toast press");

        assert_eq!(
            state.push_toast(
                ToastPriority::Action,
                "confirm",
                Toast::warning("Delete board?").action("Delete", Action::BoardDelete),
            ),
            ToastPushOutcome::UpdatedActive
        );
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);

        assert_eq!(
            state.resolve_toast_release(pressed, 50, 40),
            (false, None),
            "same-key content replacement must invalidate the press"
        );
    }

    #[test]
    fn save_pending_clipboard_to_file_without_pending_data_warns_and_triggers_feedback() {
        let mut state = make_state();

        state.save_pending_clipboard_to_file();

        let toast = state.active_toast().expect("warning toast");
        assert_eq!(toast.kind, UiToastKind::Warning);
        assert_eq!(toast.message, "No pending image to save");
        assert!(state.test_blocked_feedback_active());
    }

    #[test]
    fn clipboard_fallback_exit_after_save_requests_explicit_overlay_exit() {
        let mut state = make_state();
        let temp = crate::test_temp::tempdir().expect("tempdir");
        state.set_clipboard_fallback(
            b"not-a-real-png-but-save-writes-bytes".to_vec(),
            FileSaveConfig {
                save_directory: temp.path().to_path_buf(),
                filename_template: "fallback".to_string(),
                format: "png".to_string(),
            },
            ImageOperationKind::Screenshot,
            true,
        );

        state.save_pending_clipboard_to_file();

        assert!(state.should_exit);
        assert!(state.take_explicit_exit_requested());
        assert!(!state.take_explicit_exit_requested());
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

        let toast = state.active_toast().expect("error toast");
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
        assert!(state.selection_clipboard.has_pending_image_fallback());
        assert!(state.test_blocked_feedback_active());
    }

    /// Producer-migration completeness: every toast producer goes through
    /// `push_toast(priority, key, toast)`. The legacy `set_ui_toast*` shims
    /// have been removed; no module may reintroduce them.
    #[test]
    fn all_toast_producers_use_the_priority_queue_api() {
        let src_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let allowlist = [
            // This file names the retired shims in the assertion string below.
            "input/state/core/utility/toasts.rs",
        ];

        let mut offenders = Vec::new();
        let mut stack = vec![src_root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read src dir") {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                let rel = path
                    .strip_prefix(&src_root)
                    .expect("path under src")
                    .to_string_lossy()
                    .replace('\\', "/");
                if allowlist.contains(&rel.as_str()) {
                    continue;
                }
                let contents = std::fs::read_to_string(&path).expect("read source file");
                if contents.contains(".set_ui_toast") {
                    offenders.push(rel);
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "files still using legacy set_ui_toast* instead of push_toast: {offenders:?}"
        );
    }

    #[test]
    fn advance_text_edit_entry_feedback_clears_expired_feedback() {
        let mut state = make_state();
        let started = Instant::now();
        state
            .text_editing
            .set_edit_entry_feedback(Some(TextEditEntryFeedback { started }));
        let now = started + Duration::from_millis(TEXT_EDIT_ENTRY_DURATION_MS);

        assert!(!state.advance_text_edit_entry_feedback(now));
        assert!(state.text_editing.edit_entry_progress(now).is_none());
    }
}
