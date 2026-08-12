//! System-clipboard integration for the text editor (Ctrl+C / Ctrl+X / Ctrl+V).
//!
//! Copy and cut publish the selected text asynchronously via `wl-copy` — the
//! source process must outlive the request to serve the selection, which is why
//! this uses a controller. Paste also reads via a worker so a slow clipboard
//! owner cannot block Wayland input or rendering. The keyboard layer records
//! the intent on `InputState`; these
//! handlers, drained each input cycle, fulfill it against the compositor
//! clipboard (reusing the generic clipboard worker pipeline).

use super::{ClipboardOperationController, WaylandState};
use crate::backend::wayland::clipboard::ClipboardPoll;
use crate::clipboard_text::{
    ClipboardTextError, copy_text_via_command, read_clipboard_text_via_command,
};
use crate::input::state::{
    TextClipboardRequest, TextPasteEdit, TextPasteTarget, Toast, ToastPriority,
};
use std::{collections::VecDeque, time::Duration};

impl WaylandState {
    /// Publish captured text-selection to the system clipboard (Ctrl+C / X).
    pub(in crate::backend::wayland) fn handle_copy_text(&mut self, request: TextClipboardRequest) {
        if request.text.is_empty() {
            return;
        }
        self.suppress_focus_exit_for(Duration::from_millis(1500));
        if let Err(err) = queue_text_copy(
            &mut self.clipboard_text_copy,
            &mut self.pending_text_copy,
            request,
            copy_text_via_command,
        ) {
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
        match self.clipboard_text_copy.poll() {
            ClipboardPoll::Idle | ClipboardPoll::Pending { .. } => {}
            ClipboardPoll::Ready {
                context: request,
                outcome: Ok(()),
                ..
            } => self.input_state.complete_text_copy(request),
            ClipboardPoll::Ready {
                outcome: Err(err), ..
            } => {
                log::warn!("wl-copy failed for text copy: {err}");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "text_clipboard",
                    Toast::warning("Failed to copy to clipboard"),
                );
            }
            ClipboardPoll::ProducerFailed { reason, .. } => {
                log::error!("Text copy producer failed: {reason}");
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "text_clipboard",
                    Toast::warning("Failed to copy to clipboard"),
                );
            }
            ClipboardPoll::Disconnected { .. } => {
                log::error!("Text copy producer disconnected");
            }
        }
        if let Err(err) = submit_pending_text_copy_if_idle(
            &mut self.clipboard_text_copy,
            &mut self.pending_text_copy,
            copy_text_via_command,
        ) {
            log::warn!("Failed to start pending text clipboard copy: {err}");
        }
    }

    /// Queue a clipboard read for the text-edit session that requested it.
    pub(in crate::backend::wayland) fn handle_paste_text(&mut self, target: TextPasteTarget) {
        if !self.input_state.text_paste_target_is_current(target) {
            return;
        }
        self.suppress_focus_exit_for(Duration::from_millis(1500));
        if let Err(err) = queue_text_paste(
            &mut self.clipboard_text_paste,
            &mut self.pending_text_paste,
            target,
            read_text_paste,
        ) {
            log::warn!("Failed to start text clipboard paste: {err}");
            self.push_text_paste_failure();
        }
    }

    /// Poll the async text-paste read and apply it only to the originating
    /// text-edit session. A later edit must never receive a stale completion.
    pub(in crate::backend::wayland) fn poll_text_paste_completion(&mut self) {
        match self.clipboard_text_paste.poll() {
            ClipboardPoll::Idle | ClipboardPoll::Pending { .. } => {}
            ClipboardPoll::Ready {
                context: target,
                outcome,
                ..
            } => {
                if self.input_state.text_paste_target_is_current(target) {
                    match outcome {
                        Ok(Some(text)) => {
                            if let Some(edit) = self.input_state.apply_text_paste(target, &text) {
                                for pending in &mut self.pending_text_paste {
                                    rebase_text_paste_target(pending, &edit);
                                }
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            log::warn!("wl-paste failed for text paste: {err}");
                            self.push_text_paste_failure();
                        }
                    }
                } else {
                    log::debug!("Discarding stale text clipboard paste completion");
                }
            }
            ClipboardPoll::ProducerFailed {
                context: target,
                reason,
                ..
            } => {
                log::error!("Text paste producer failed: {reason}");
                if self.input_state.text_paste_target_is_current(target) {
                    self.push_text_paste_failure();
                }
            }
            ClipboardPoll::Disconnected {
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
        if self.clipboard_text_paste.is_active() {
            return;
        }
        while let Some(target) = self.pending_text_paste.pop_front() {
            if !self.input_state.text_paste_target_is_current(target) {
                log::debug!("Discarding stale queued text clipboard paste request");
                continue;
            }
            if let Err(err) =
                start_text_paste(&mut self.clipboard_text_paste, target, read_text_paste)
            {
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
            Toast::warning("Failed to paste from clipboard"),
        );
    }
}

fn start_text_copy(
    controller: &mut ClipboardOperationController<TextClipboardRequest, Result<(), String>>,
    request: TextClipboardRequest,
    operation: impl FnOnce(&str) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let worker_text = request.text.clone();
    controller
        .try_submit(request, "wayscriber-text-copy", move || {
            operation(&worker_text)
        })
        .map(drop)
        .map_err(|failure| failure.into_parts().0.to_string())
}

fn queue_text_copy(
    controller: &mut ClipboardOperationController<TextClipboardRequest, Result<(), String>>,
    pending: &mut VecDeque<TextClipboardRequest>,
    request: TextClipboardRequest,
    operation: impl FnOnce(&str) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    if request.cut.is_none()
        && let Some(previous) = pending.back_mut()
        && previous.cut.is_none()
    {
        *previous = request;
    } else {
        pending.push_back(request);
    }
    submit_pending_text_copy_if_idle(controller, pending, operation)
}

fn submit_pending_text_copy_if_idle(
    controller: &mut ClipboardOperationController<TextClipboardRequest, Result<(), String>>,
    pending: &mut VecDeque<TextClipboardRequest>,
    operation: impl FnOnce(&str) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    if controller.is_active() {
        return Ok(());
    }
    let Some(request) = pending.pop_front() else {
        return Ok(());
    };
    start_text_copy(controller, request, operation)
}

type TextPasteOutcome = Result<Option<String>, String>;

fn read_text_paste() -> TextPasteOutcome {
    match read_clipboard_text_via_command() {
        Ok(text) => Ok(Some(text)),
        Err(ClipboardTextError::Empty) => Ok(None),
        Err(ClipboardTextError::Other(err)) => Err(err),
    }
}

fn start_text_paste(
    controller: &mut ClipboardOperationController<TextPasteTarget, TextPasteOutcome>,
    target: TextPasteTarget,
    operation: impl FnOnce() -> TextPasteOutcome + Send + 'static,
) -> Result<(), String> {
    controller
        .try_submit(target, "wayscriber-text-paste", operation)
        .map(drop)
        .map_err(|failure| failure.into_parts().0.to_string())
}

fn queue_text_paste(
    controller: &mut ClipboardOperationController<TextPasteTarget, TextPasteOutcome>,
    pending: &mut VecDeque<TextPasteTarget>,
    target: TextPasteTarget,
    operation: impl FnOnce() -> TextPasteOutcome + Send + 'static,
) -> Result<(), String> {
    if pending
        .back()
        .is_some_and(|pending_target| pending_target.generation != target.generation)
    {
        pending.clear();
    }
    pending.push_back(target);
    if controller.is_active() {
        return Ok(());
    }
    // Submit from the front so queued requests keep their arrival order; the
    // target just pushed is the only entry whenever the controller is idle.
    let Some(target) = pending.pop_front() else {
        return Ok(());
    };
    start_text_paste(controller, target, operation)
}

fn rebase_text_paste_target(target: &mut TextPasteTarget, edit: &TextPasteEdit) {
    if target.generation != edit.generation || target.revision != edit.previous_revision {
        return;
    }

    target.caret = rebase_text_paste_offset(target.caret, edit);
    target.selection_anchor = target
        .selection_anchor
        .map(|anchor| rebase_text_paste_offset(anchor, edit));
    if target.selection_anchor == Some(target.caret) {
        target.selection_anchor = None;
    }
    target.revision = edit.revision;
}

fn rebase_text_paste_offset(offset: usize, edit: &TextPasteEdit) -> usize {
    if offset < edit.replaced.start {
        offset
    } else if offset > edit.replaced.end {
        edit.replaced.start + edit.inserted_len + (offset - edit.replaced.end)
    } else {
        edit.replaced.start + edit.inserted_len
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;
    use crate::backend::wayland::clipboard::ClipboardOperationIdSource;

    fn copy_request(text: &str) -> TextClipboardRequest {
        TextClipboardRequest {
            text: text.to_string(),
            cut: None,
        }
    }

    fn cut_request(text: &str, start: usize) -> TextClipboardRequest {
        TextClipboardRequest {
            text: text.to_string(),
            cut: Some(crate::input::state::TextCutTarget {
                generation: 7,
                revision: 3,
                range: start..start + text.len(),
            }),
        }
    }

    fn paste_target(generation: u64) -> TextPasteTarget {
        TextPasteTarget {
            generation,
            revision: 3,
            caret: 5,
            selection_anchor: None,
        }
    }

    #[test]
    fn text_copy_keeps_its_request_context_until_worker_completion() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            ClipboardOperationController::new(ClipboardOperationIdSource::new(), wake.handle());

        start_text_copy(&mut controller, copy_request("selected"), |text| {
            assert_eq!(text, "selected");
            Ok(())
        })
        .unwrap();

        assert!(
            wake.wait_readable(Some(Duration::from_secs(1))).unwrap(),
            "text copy completion did not wake the event loop"
        );
        assert!(matches!(
            controller.poll(),
            ClipboardPoll::Ready {
                context: TextClipboardRequest { text, .. },
                outcome: Ok(()),
                ..
            } if text == "selected"
        ));
    }

    #[test]
    fn active_text_copy_retains_only_the_newest_request() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            ClipboardOperationController::new(ClipboardOperationIdSource::new(), wake.handle());
        let mut pending = VecDeque::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        queue_text_copy(
            &mut controller,
            &mut pending,
            copy_request("first"),
            move |_| {
                started_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(1)).unwrap();
                Ok(())
            },
        )
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        queue_text_copy(
            &mut controller,
            &mut pending,
            copy_request("second"),
            |_| panic!("busy submission must not run"),
        )
        .unwrap();
        queue_text_copy(
            &mut controller,
            &mut pending,
            copy_request("newest"),
            |_| panic!("busy submission must not run"),
        )
        .unwrap();

        assert_eq!(
            pending.front().map(|request| request.text.as_str()),
            Some("newest")
        );
        release_tx.send(()).unwrap();
    }

    #[test]
    fn active_text_copy_preserves_every_pending_cut_request() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            ClipboardOperationController::new(ClipboardOperationIdSource::new(), wake.handle());
        let mut pending = VecDeque::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        queue_text_copy(
            &mut controller,
            &mut pending,
            copy_request("first"),
            move |_| {
                started_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(1)).unwrap();
                Ok(())
            },
        )
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        queue_text_copy(
            &mut controller,
            &mut pending,
            cut_request("second", 0),
            |_| panic!("busy submission must not run"),
        )
        .unwrap();
        queue_text_copy(
            &mut controller,
            &mut pending,
            cut_request("third", 6),
            |_| panic!("busy submission must not run"),
        )
        .unwrap();

        assert_eq!(
            pending
                .iter()
                .map(|request| request.text.as_str())
                .collect::<Vec<_>>(),
            ["second", "third"]
        );
        release_tx.send(()).unwrap();
    }

    #[test]
    fn text_paste_read_stays_off_the_event_thread_until_completion() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            ClipboardOperationController::new(ClipboardOperationIdSource::new(), wake.handle());
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        start_text_paste(&mut controller, paste_target(7), move || {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            Ok(Some("clipboard text".to_string()))
        })
        .unwrap();

        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(controller.poll(), ClipboardPoll::Pending { .. }));
        release_tx.send(()).unwrap();
        assert!(
            wake.wait_readable(Some(Duration::from_secs(1))).unwrap(),
            "text paste completion did not wake the event loop"
        );
        assert!(matches!(
            controller.poll(),
            ClipboardPoll::Ready {
                context: TextPasteTarget { generation: 7, .. },
                outcome: Ok(Some(text)),
                ..
            } if text == "clipboard text"
        ));
    }

    #[test]
    fn active_text_paste_preserves_every_request_from_the_same_session() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            ClipboardOperationController::new(ClipboardOperationIdSource::new(), wake.handle());
        let mut pending = VecDeque::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        queue_text_paste(&mut controller, &mut pending, paste_target(7), move || {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            Ok(Some("first".to_string()))
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        queue_text_paste(&mut controller, &mut pending, paste_target(7), || {
            panic!("busy submission must not run")
        })
        .unwrap();
        queue_text_paste(&mut controller, &mut pending, paste_target(7), || {
            panic!("busy submission must not run")
        })
        .unwrap();

        assert_eq!(
            pending
                .iter()
                .map(|target| target.generation)
                .collect::<Vec<_>>(),
            [7, 7]
        );
        release_tx.send(()).unwrap();
    }

    #[test]
    fn newer_text_session_supersedes_older_pending_pastes_without_coalescing_its_own() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller =
            ClipboardOperationController::new(ClipboardOperationIdSource::new(), wake.handle());
        let mut pending = VecDeque::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        queue_text_paste(&mut controller, &mut pending, paste_target(1), move || {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            Ok(Some("stale".to_string()))
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        for generation in [1, 1, 2, 2] {
            queue_text_paste(
                &mut controller,
                &mut pending,
                paste_target(generation),
                || panic!("busy submission must not run"),
            )
            .unwrap();
        }

        assert_eq!(
            pending
                .iter()
                .map(|target| target.generation)
                .collect::<Vec<_>>(),
            [2, 2]
        );
        release_tx.send(()).unwrap();
    }

    #[test]
    fn queued_paste_target_rebases_after_an_earlier_paste() {
        let mut target = TextPasteTarget {
            generation: 7,
            revision: 3,
            caret: 5,
            selection_anchor: Some(2),
        };
        rebase_text_paste_target(
            &mut target,
            &TextPasteEdit {
                generation: 7,
                previous_revision: 3,
                revision: 4,
                replaced: 2..5,
                inserted_len: 4,
            },
        );

        assert_eq!(
            target,
            TextPasteTarget {
                generation: 7,
                revision: 4,
                caret: 6,
                selection_anchor: None,
            }
        );
    }
}
