//! System-clipboard integration for the text editor (Ctrl+C / Ctrl+X / Ctrl+V).
//!
//! Copy and cut publish the selected text asynchronously via `wl-copy` — the
//! source process must outlive the request to serve the selection, which is why
//! this uses a controller. Paste also reads via a worker so a slow clipboard
//! owner cannot block Wayland input or rendering. The keyboard layer records
//! the intent on `InputState`; these
//! handlers, drained each input cycle, fulfill it against the compositor
//! clipboard (reusing the generic clipboard worker pipeline).

use super::{TextCopyOutcome, TextPasteOutcome, WaylandState};
use crate::backend::wayland::RuntimeOperationPoll;
use crate::input::state::{TextClipboardRequest, TextPasteTarget, Toast, ToastPriority};
use std::time::Duration;

impl WaylandState {
    /// Publish captured text-selection to the system clipboard (Ctrl+C / X).
    pub(in crate::backend::wayland) fn handle_copy_text(&mut self, request: TextClipboardRequest) {
        if request.text.is_empty() {
            return;
        }
        self.focus
            .suppress_exit_for(std::time::Instant::now(), Duration::from_millis(1500));
        if let Err(err) = self.clipboard.queue_text_copy(request) {
            log::warn!("Failed to start text clipboard copy: {err}");
            self.input_state.push_toast(
                ToastPriority::Info,
                "text_clipboard",
                Toast::warning("Failed to copy to clipboard"),
            );
        }
    }

    /// Poll the async text-copy pipeline, surface failures, and restart a
    /// queued copy once the controller goes idle.
    pub(in crate::backend::wayland) fn poll_text_copy_completion(&mut self) {
        match self.clipboard.poll_text_copy() {
            RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => {}
            RuntimeOperationPoll::Ready {
                context: request,
                outcome: TextCopyOutcome::Copied,
                ..
            } => self
                .input_state
                .complete_text_copy_with(self.render.text_measurer(), request),
            RuntimeOperationPoll::Ready {
                outcome: TextCopyOutcome::Failed,
                ..
            } => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "text_clipboard",
                    Toast::warning(TextCopyOutcome::Failed.message()),
                );
            }
            RuntimeOperationPoll::ProducerFailed { reason, .. } => {
                log::error!("Text copy producer failed: {reason}");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "text_clipboard",
                    Toast::warning("Failed to copy to clipboard"),
                );
            }
            RuntimeOperationPoll::Disconnected { .. } => {
                log::error!("Text copy producer disconnected");
            }
        }
        if let Err(err) = self.clipboard.submit_pending_text_copy_if_idle() {
            log::warn!("Failed to start pending text clipboard copy: {err}");
        }
    }

    /// Queue a clipboard read for the text-edit session that requested it.
    pub(in crate::backend::wayland) fn handle_paste_text(&mut self, target: TextPasteTarget) {
        if !self.input_state.text_paste_target_is_current(target) {
            return;
        }
        self.focus
            .suppress_exit_for(std::time::Instant::now(), Duration::from_millis(1500));
        if let Err(err) = self.clipboard.queue_text_paste(target) {
            log::warn!("Failed to start text clipboard paste: {err}");
            self.push_text_paste_failure();
        }
    }

    /// Poll the async text-paste read and apply it only to the originating
    /// text-edit session. A later edit must never receive a stale completion.
    pub(in crate::backend::wayland) fn poll_text_paste_completion(&mut self) {
        match self.clipboard.poll_text_paste() {
            RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => {}
            RuntimeOperationPoll::Ready {
                context: target,
                outcome,
                ..
            } => {
                if self.input_state.text_paste_target_is_current(target) {
                    match outcome {
                        TextPasteOutcome::Text(text) => {
                            if let Some(edit) = self.input_state.apply_text_paste_with(
                                self.render.text_measurer(),
                                target,
                                &text,
                            ) {
                                self.clipboard.rebase_pending_text_pastes(&edit);
                            }
                        }
                        TextPasteOutcome::Empty => {}
                        TextPasteOutcome::Failed => self.push_text_paste_failure(),
                    }
                } else {
                    log::debug!("Discarding stale text clipboard paste completion");
                }
            }
            RuntimeOperationPoll::ProducerFailed {
                context: target,
                reason,
                ..
            } => {
                log::error!("Text paste producer failed: {reason}");
                if self.input_state.text_paste_target_is_current(target) {
                    self.push_text_paste_failure();
                }
            }
            RuntimeOperationPoll::Disconnected {
                context: target, ..
            } => {
                log::error!("Text paste producer disconnected");
                if self.input_state.text_paste_target_is_current(target) {
                    self.push_text_paste_failure();
                }
            }
        }
        self.start_pending_text_paste_if_idle();
    }

    fn start_pending_text_paste_if_idle(&mut self) {
        while let Some(target) = self.clipboard.take_pending_text_paste_if_idle() {
            if !self.input_state.text_paste_target_is_current(target) {
                log::debug!("Discarding stale queued text clipboard paste request");
                continue;
            }
            if let Err(err) = self.clipboard.submit_text_paste(target) {
                log::warn!("Failed to start pending text clipboard paste: {err}");
                self.push_text_paste_failure();
            }
            break;
        }
    }

    fn push_text_paste_failure(&mut self) {
        self.input_state.push_toast(
            ToastPriority::Info,
            "text_clipboard",
            Toast::warning(TextPasteOutcome::Failed.message()),
        );
    }
}
