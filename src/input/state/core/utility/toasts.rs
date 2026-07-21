use super::super::base::{
    BLOCKED_ACTION_DURATION_MS, BlockedActionFeedback, InputState, PendingClipboardFallback,
    TEXT_EDIT_ENTRY_DURATION_MS, Toast, ToastPress, ToastPriority, ToastPushOutcome,
};
use crate::capture::{
    ImageOperationKind,
    file::{FileSaveConfig, save_screenshot},
};
use crate::domain::Action;
use std::path::Path;
use std::time::{Duration, Instant};

impl InputState {
    /// Push a toast into the priority queue. Higher priorities preempt the
    /// visible toast, equal priorities queue FIFO, and a push with the key of
    /// an active/queued toast updates it in place (see
    /// [`crate::input::state::ToastQueue`]).
    pub(crate) fn push_toast(
        &mut self,
        priority: ToastPriority,
        key: &'static str,
        toast: Toast,
    ) -> ToastPushOutcome {
        let outcome =
            self.toast_queue
                .push(&mut self.ui_toast, priority, key, toast, Instant::now());
        if outcome.changed_active() {
            self.ui_toast_bounds = None;
            self.needs_redraw = true;
        }
        outcome
    }

    /// Whether no toast is visible and nothing is queued. Hint producers use
    /// this to defer instead of competing with real feedback.
    pub(crate) fn toasts_idle(&self) -> bool {
        self.ui_toast.is_none() && self.toast_queue.is_empty()
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

        self.push_toast(
            ToastPriority::Info,
            "capture.feedback",
            Toast::info(parts.join(" | ")),
        );
    }

    pub fn advance_ui_toast(&mut self, now: Instant) -> bool {
        let had_toast = self.ui_toast.is_some();
        let (still_showing, activated) = self.toast_queue.advance(&mut self.ui_toast, now);
        if activated || (had_toast && !still_showing) {
            // The visible toast changed (expired or a queued one replaced it).
            self.ui_toast_bounds = None;
            self.needs_redraw = true;
        }
        still_showing
    }

    /// Capture the identity of the toast under a press without dismissing it.
    pub(crate) fn toast_press_at(&self, x: i32, y: i32) -> Option<ToastPress> {
        let toast = self.ui_toast.as_ref()?;
        self.toast_contains(x, y)
            .then(|| ToastPress::new(toast.activation_id))
    }

    /// Resolve a toast release only when the exact toast activation captured
    /// on press is still visible and the release remains within its bounds.
    pub(crate) fn resolve_toast_release(
        &mut self,
        pressed: ToastPress,
        x: i32,
        y: i32,
    ) -> (bool, Option<Action>) {
        let Some(toast) = self.ui_toast.as_ref() else {
            return (false, None);
        };

        if pressed.matches(toast) && self.toast_contains(x, y) {
            // Click is within toast
            let action = toast.action.as_ref().map(|action| action.action);
            // Dismiss the toast and promote the next queued one, if any.
            self.toast_queue
                .on_dismissed(&mut self.ui_toast, Instant::now());
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
                self.last_capture_path = Some(path.clone());
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
    use crate::input::state::core::base::{TextEditEntryFeedback, UiToastKind};
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
        state.push_toast(
            ToastPriority::Info,
            "test",
            Toast::info("Hello").duration_ms(10),
        );
        state.ui_toast_bounds = Some((1.0, 2.0, 3.0, 4.0));
        let now = state.ui_toast.as_ref().unwrap().started + Duration::from_millis(10);

        assert!(!state.advance_ui_toast(now));
        assert!(state.ui_toast.is_none());
        assert!(state.ui_toast_bounds.is_none());
    }

    #[test]
    fn advance_ui_toast_promotes_queued_toast_when_active_expires() {
        let mut state = make_state();
        state.push_toast(ToastPriority::Info, "first", Toast::info("First"));
        state.push_toast(ToastPriority::Info, "second", Toast::info("Second"));
        state.ui_toast_bounds = Some((1.0, 2.0, 3.0, 4.0));
        state.needs_redraw = false;
        let now = state.ui_toast.as_ref().unwrap().started
            + Duration::from_millis(state.ui_toast.as_ref().unwrap().duration_ms);

        assert!(state.advance_ui_toast(now), "queued toast keeps showing");
        let toast = state.ui_toast.as_ref().expect("promoted toast");
        assert_eq!(toast.message, "Second");
        assert!(state.ui_toast_bounds.is_none(), "stale bounds cleared");
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
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

        let pressed = state.toast_press_at(50, 40).expect("toast press");
        let (hit, action) = state.resolve_toast_release(pressed, 50, 40);

        assert!(hit);
        assert_eq!(action, Some(Action::OpenCaptureFolder));
        assert!(state.ui_toast.is_none());
        assert!(state.ui_toast_bounds.is_none());
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
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

        let pressed = state.toast_press_at(50, 40).expect("toast press");
        let (hit, action) = state.resolve_toast_release(pressed, 50, 40);

        assert!(hit);
        assert_eq!(action, Some(Action::PageDelete));
        let promoted = state.ui_toast.as_ref().expect("queued toast promoted");
        assert_eq!(promoted.message, "Later");
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
        let pressed = state.toast_press_at(50, 40).expect("toast press");
        assert_eq!(
            state.resolve_toast_release(pressed, 50, 40),
            (true, Some(Action::Undo))
        );
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
        state.push_toast(ToastPriority::Info, "test", Toast::info("Saved"));
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

        assert!(state.toast_contains(50, 40));
        assert!(state.ui_toast.is_some());
        assert!(state.ui_toast_bounds.is_some());
    }

    #[test]
    fn preempting_toast_clears_stale_click_bounds() {
        let mut state = make_state();
        state.push_toast(ToastPriority::Info, "info", Toast::info("Saved"));
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

        // Action priority preempts the plain info toast.
        let outcome = state.push_toast(
            ToastPriority::Action,
            "confirm",
            Toast::warning("Delete page?").action("Confirm", Action::PageDelete),
        );

        assert_eq!(outcome, ToastPushOutcome::Displayed);
        let toast = state.ui_toast.as_ref().expect("preempting toast visible");
        assert_eq!(toast.message, "Delete page?");
        assert!(state.ui_toast_bounds.is_none());
        assert!(!state.toast_contains(50, 40));
        let stale_press = ToastPress::new(0);
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
        assert_eq!(state.ui_toast.as_ref().unwrap().message, "Board 3");
        assert!(state.toasts_idle() || state.ui_toast.is_some());
        assert!(
            state.toast_queue.is_empty(),
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
        assert_eq!(state.ui_toast.as_ref().unwrap().message, "Busy");

        // Once idle again, the hint is accepted.
        let now = state.ui_toast.as_ref().unwrap().started
            + Duration::from_millis(state.ui_toast.as_ref().unwrap().duration_ms);
        state.advance_ui_toast(now);
        assert!(state.toasts_idle());
        let outcome = state.push_toast(ToastPriority::Hint, "hint", Toast::info("Press F1"));
        assert_eq!(outcome, ToastPushOutcome::Displayed);
    }

    #[test]
    fn toast_release_ignores_releases_outside_bounds() {
        let mut state = make_state();
        state.push_toast(ToastPriority::Info, "test", Toast::info("Saved"));
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

        let pressed = state.toast_press_at(50, 40).expect("toast press");
        let (hit, action) = state.resolve_toast_release(pressed, 5, 5);

        assert!(!hit);
        assert_eq!(action, None);
        assert!(state.ui_toast.is_some());
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
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));
        let pressed = state.toast_press_at(50, 40).expect("first toast press");
        let expiry =
            state.ui_toast.as_ref().expect("first toast").started + Duration::from_millis(10);

        assert!(state.advance_ui_toast(expiry));
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));
        assert_eq!(
            state.ui_toast.as_ref().expect("promoted toast").message,
            "Delete page?"
        );

        assert_eq!(
            state.resolve_toast_release(pressed, 50, 40),
            (false, None),
            "release must not dispatch the promoted destructive toast"
        );
        assert_eq!(
            state
                .ui_toast
                .as_ref()
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
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));
        let pressed = state.toast_press_at(50, 40).expect("original toast press");

        assert_eq!(
            state.push_toast(
                ToastPriority::Action,
                "confirm",
                Toast::warning("Delete board?").action("Delete", Action::BoardDelete),
            ),
            ToastPushOutcome::UpdatedActive
        );
        state.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

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
        state.text_edit_entry_feedback = Some(TextEditEntryFeedback {
            started: Instant::now(),
        });
        let now = state.text_edit_entry_feedback.as_ref().unwrap().started
            + Duration::from_millis(TEXT_EDIT_ENTRY_DURATION_MS);

        assert!(!state.advance_text_edit_entry_feedback(now));
        assert!(state.text_edit_entry_feedback.is_none());
    }
}
