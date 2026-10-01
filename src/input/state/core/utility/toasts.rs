use super::super::base::{
    CompositorCapabilities, InputState, Toast, ToastCommand, ToastPress, ToastPriority,
    ToastPushOutcome, UiToastState,
};
use super::super::feedback::ToastBounds;
use crate::capture::{ImageOperationKind, file::FileSaveConfig};
use crate::domain::Action;
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

    /// Queue the retained image for backend file work without holding input dispatch.
    pub(crate) fn save_pending_clipboard_to_file(&mut self) {
        let Some(request_id) = self.selection_clipboard.request_image_save() else {
            if !self.selection_clipboard.has_pending_image_fallback() {
                self.push_toast(
                    ToastPriority::Info,
                    "capture.save",
                    Toast::warning("No pending image to save"),
                );
                self.trigger_blocked_feedback();
            }
            return;
        };

        self.set_pending_backend_action(
            super::super::base::PendingBackendAction::SaveClipboardFallback { request_id },
        );
    }

    pub(crate) fn clipboard_fallback_save_request(
        &self,
        id: u64,
    ) -> Option<std::sync::Arc<crate::input::state::ClipboardFallbackSaveRequest>> {
        self.selection_clipboard.image_save_request(id)
    }

    pub(crate) fn complete_clipboard_fallback_save(
        &mut self,
        id: u64,
        result: Result<std::path::PathBuf, String>,
    ) {
        let Some(fallback) = self
            .selection_clipboard
            .complete_image_save(id, result.is_ok())
        else {
            return;
        };
        match result {
            Ok(path) => {
                log::info!(
                    "Saved pending {} to: {}",
                    fallback.operation.saved_log_label(),
                    path.display()
                );
                self.set_capture_feedback(Some(&path), false);
                if fallback.exit_after_save {
                    self.request_explicit_exit();
                }
            }
            Err(message) => {
                log::error!(
                    "Failed to save pending {}: {message}",
                    fallback.operation.saved_log_label()
                );
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
    use crate::domain::OnboardingTip;
    use crate::draw::{Color, Shape};
    use crate::input::state::core::base::UiToastKind;
    use crate::input::state::core::text_editing::{
        TEXT_EDIT_ENTRY_DURATION_MS, TextEditEntryFeedback,
    };

    use crate::ui::toolbar::ToolbarEvent;
    use std::time::Duration;

    fn make_state() -> InputState {
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
            fill_color: None,
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
    fn toast_press_reports_hit_without_dismissing() {
        let mut state = make_state();
        state.push_toast(ToastPriority::Info, "test", Toast::info("Saved"));
        state.set_toast_geometry(Some((10.0, 20.0, 100.0, 40.0)), [None, None]);

        assert!(state.toast_press_at(50, 40).is_some());
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
        assert!(state.toast_press_at(50, 40).is_none());
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
