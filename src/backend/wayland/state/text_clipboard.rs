//! System-clipboard integration for the text editor (Ctrl+C / Ctrl+X / Ctrl+V).
//!
//! Copy and cut publish the selected text asynchronously via `wl-copy` — the
//! source process must outlive the request to serve the selection, which is why
//! this uses a controller. Paste also reads via a worker so a slow clipboard
//! owner cannot block Wayland input or rendering. The keyboard layer records
//! the intent on `InputState`; these
//! handlers, drained each input cycle, fulfill it against the compositor
//! clipboard (reusing the generic clipboard worker pipeline).

use super::color_picker::{
    ClipboardTextError, copy_text_via_command, read_clipboard_text_via_command,
};
use super::{ClipboardOperationController, ClipboardOperationIdSource, WaylandState};
use crate::backend::wayland::clipboard::ClipboardPoll;
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
        let process_broker = self.process_broker.clone();
        if let Err(err) = queue_text_copy(
            &mut self.clipboard_operation_ids,
            &mut self.clipboard_text_copy,
            &mut self.pending_text_copy,
            request,
            move |text| copy_text_via_command(&process_broker, text),
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
            ClipboardPoll::Cancelled { .. } => {
                log::info!("Text copy producer was cancelled");
            }
        }
        let process_broker = self.process_broker.clone();
        if let Err(err) = submit_pending_text_copy_if_idle(
            &mut self.clipboard_operation_ids,
            &mut self.clipboard_text_copy,
            &mut self.pending_text_copy,
            move |text| copy_text_via_command(&process_broker, text),
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
        let process_broker = self.process_broker.clone();
        if let Err(err) = queue_text_paste(
            &mut self.clipboard_operation_ids,
            &mut self.clipboard_text_paste,
            &mut self.pending_text_paste,
            target,
            move || read_text_paste(&process_broker),
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
            ClipboardPoll::Cancelled {
                context: target, ..
            } => {
                log::info!("Text paste producer was cancelled");
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
            let process_broker = self.process_broker.clone();
            if let Err(err) = start_text_paste(
                &mut self.clipboard_operation_ids,
                &mut self.clipboard_text_paste,
                target,
                move || read_text_paste(&process_broker),
            ) {
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
    ids: &mut ClipboardOperationIdSource,
    controller: &mut ClipboardOperationController<TextClipboardRequest, Result<(), String>>,
    request: TextClipboardRequest,
    operation: impl FnOnce(&str) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let worker_text = request.text.clone();
    controller
        .try_submit(ids, request, "wayscriber-text-copy", move |_cancellation| {
            operation(&worker_text)
        })
        .map(drop)
        .map_err(|failure| failure.into_parts().0.to_string())
}

fn queue_text_copy(
    ids: &mut ClipboardOperationIdSource,
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
    submit_pending_text_copy_if_idle(ids, controller, pending, operation)
}

fn submit_pending_text_copy_if_idle(
    ids: &mut ClipboardOperationIdSource,
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
    start_text_copy(ids, controller, request, operation)
}

type TextPasteOutcome = Result<Option<String>, String>;

fn read_text_paste(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
) -> TextPasteOutcome {
    match read_clipboard_text_via_command(process_broker) {
        Ok(text) => Ok(Some(text)),
        Err(ClipboardTextError::Empty) => Ok(None),
        Err(ClipboardTextError::Other(err)) => Err(err),
    }
}

fn start_text_paste(
    ids: &mut ClipboardOperationIdSource,
    controller: &mut ClipboardOperationController<TextPasteTarget, TextPasteOutcome>,
    target: TextPasteTarget,
    operation: impl FnOnce() -> TextPasteOutcome + Send + 'static,
) -> Result<(), String> {
    controller
        .try_submit(ids, target, "wayscriber-text-paste", move |_cancellation| {
            operation()
        })
        .map(drop)
        .map_err(|failure| failure.into_parts().0.to_string())
}

fn queue_text_paste(
    ids: &mut ClipboardOperationIdSource,
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
    start_text_paste(ids, controller, target, operation)
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

    fn clipboard_controller<C, T: Send + 'static>() -> (
        RuntimeWakeSource,
        ClipboardOperationIdSource,
        ClipboardOperationController<C, T>,
    ) {
        let wake =
            RuntimeWakeSource::new().expect("text clipboard fixture creates a runtime eventfd");
        let controller = ClipboardOperationController::new(
            wake.try_sender()
                .expect("text clipboard fixture duplicates its runtime eventfd"),
        );
        (wake, ClipboardOperationIdSource::new(), controller)
    }

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
        let (wake, mut ids, mut controller) = clipboard_controller();

        start_text_copy(
            &mut ids,
            &mut controller,
            copy_request("selected"),
            |text| {
                assert_eq!(text, "selected");
                Ok(())
            },
        )
        .expect("text-copy fixture submits its request");

        assert!(
            wake.wait_readable(Some(Duration::from_secs(1)))
                .expect("text-copy fixture polls its valid runtime eventfd"),
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
        let (_wake, mut ids, mut controller) = clipboard_controller();
        let mut pending = VecDeque::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        queue_text_copy(
            &mut ids,
            &mut controller,
            &mut pending,
            copy_request("first"),
            move |_| {
                started_tx.send(()).map_err(|error| error.to_string())?;
                release_rx
                    .recv_timeout(Duration::from_secs(1))
                    .map_err(|error| error.to_string())?;
                Ok(())
            },
        )
        .expect("newest text-copy fixture submits its active request");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("active text-copy worker announces that it started");

        let (unexpected_tx, unexpected_rx) = mpsc::channel();
        let second_unexpected_tx = unexpected_tx.clone();
        queue_text_copy(
            &mut ids,
            &mut controller,
            &mut pending,
            copy_request("second"),
            move |_| {
                let _ = second_unexpected_tx.send("second");
                Ok(())
            },
        )
        .expect("busy text-copy fixture queues its second request");
        queue_text_copy(
            &mut ids,
            &mut controller,
            &mut pending,
            copy_request("newest"),
            move |_| {
                let _ = unexpected_tx.send("newest");
                Ok(())
            },
        )
        .expect("busy text-copy fixture replaces its pending copy request");

        assert!(matches!(
            unexpected_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected)
        ));
        assert_eq!(
            pending.front().map(|request| request.text.as_str()),
            Some("newest")
        );
        release_tx
            .send(())
            .expect("newest text-copy fixture retains its active worker receiver");
    }

    #[test]
    fn active_text_copy_preserves_every_pending_cut_request() {
        let (_wake, mut ids, mut controller) = clipboard_controller();
        let mut pending = VecDeque::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        queue_text_copy(
            &mut ids,
            &mut controller,
            &mut pending,
            copy_request("first"),
            move |_| {
                started_tx.send(()).map_err(|error| error.to_string())?;
                release_rx
                    .recv_timeout(Duration::from_secs(1))
                    .map_err(|error| error.to_string())?;
                Ok(())
            },
        )
        .expect("pending-cut fixture submits its active copy request");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("pending-cut worker announces that it started");

        let (unexpected_tx, unexpected_rx) = mpsc::channel();
        let second_unexpected_tx = unexpected_tx.clone();
        queue_text_copy(
            &mut ids,
            &mut controller,
            &mut pending,
            cut_request("second", 0),
            move |_| {
                let _ = second_unexpected_tx.send("second");
                Ok(())
            },
        )
        .expect("busy cut fixture queues its second request");
        queue_text_copy(
            &mut ids,
            &mut controller,
            &mut pending,
            cut_request("third", 6),
            move |_| {
                let _ = unexpected_tx.send("third");
                Ok(())
            },
        )
        .expect("busy cut fixture queues its third request");

        assert!(matches!(
            unexpected_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected)
        ));
        assert_eq!(
            pending
                .iter()
                .map(|request| request.text.as_str())
                .collect::<Vec<_>>(),
            ["second", "third"]
        );
        release_tx
            .send(())
            .expect("pending-cut fixture retains its active worker receiver");
    }

    #[test]
    fn text_paste_read_stays_off_the_event_thread_until_completion() {
        let (wake, mut ids, mut controller) = clipboard_controller();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        start_text_paste(&mut ids, &mut controller, paste_target(7), move || {
            started_tx.send(()).map_err(|error| error.to_string())?;
            release_rx
                .recv_timeout(Duration::from_secs(1))
                .map_err(|error| error.to_string())?;
            Ok(Some("clipboard text".to_string()))
        })
        .expect("text-paste fixture submits its read request");

        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("text-paste worker announces that it started");
        assert!(matches!(controller.poll(), ClipboardPoll::Pending { .. }));
        release_tx
            .send(())
            .expect("text-paste fixture retains its worker receiver");
        assert!(
            wake.wait_readable(Some(Duration::from_secs(1)))
                .expect("text-paste fixture polls its valid runtime eventfd"),
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
        let (_wake, mut ids, mut controller) = clipboard_controller();
        let mut pending = VecDeque::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        queue_text_paste(
            &mut ids,
            &mut controller,
            &mut pending,
            paste_target(7),
            move || {
                started_tx.send(()).map_err(|error| error.to_string())?;
                release_rx
                    .recv_timeout(Duration::from_secs(1))
                    .map_err(|error| error.to_string())?;
                Ok(Some("first".to_string()))
            },
        )
        .expect("same-session paste fixture submits its active request");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("same-session paste worker announces that it started");

        let (unexpected_tx, unexpected_rx) = mpsc::channel();
        let second_unexpected_tx = unexpected_tx.clone();
        queue_text_paste(
            &mut ids,
            &mut controller,
            &mut pending,
            paste_target(7),
            move || {
                let _ = second_unexpected_tx.send("second");
                Ok(None)
            },
        )
        .expect("same-session paste fixture queues its second request");
        queue_text_paste(
            &mut ids,
            &mut controller,
            &mut pending,
            paste_target(7),
            move || {
                let _ = unexpected_tx.send("third");
                Ok(None)
            },
        )
        .expect("same-session paste fixture queues its third request");

        assert!(matches!(
            unexpected_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected)
        ));
        assert_eq!(
            pending
                .iter()
                .map(|target| target.generation)
                .collect::<Vec<_>>(),
            [7, 7]
        );
        release_tx
            .send(())
            .expect("same-session paste fixture retains its active worker receiver");
    }

    #[test]
    fn newer_text_session_supersedes_older_pending_pastes_without_coalescing_its_own() {
        let (_wake, mut ids, mut controller) = clipboard_controller();
        let mut pending = VecDeque::new();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        queue_text_paste(
            &mut ids,
            &mut controller,
            &mut pending,
            paste_target(1),
            move || {
                started_tx.send(()).map_err(|error| error.to_string())?;
                release_rx
                    .recv_timeout(Duration::from_secs(1))
                    .map_err(|error| error.to_string())?;
                Ok(Some("stale".to_string()))
            },
        )
        .expect("new-session paste fixture submits its active request");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("new-session paste worker announces that it started");

        let (unexpected_tx, unexpected_rx) = mpsc::channel();
        for generation in [1, 1, 2, 2] {
            let unexpected_tx = unexpected_tx.clone();
            queue_text_paste(
                &mut ids,
                &mut controller,
                &mut pending,
                paste_target(generation),
                move || {
                    let _ = unexpected_tx.send(generation);
                    Ok(None)
                },
            )
            .expect("busy new-session paste fixture queues its request");
        }

        assert!(matches!(
            unexpected_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected)
        ));
        assert_eq!(
            pending
                .iter()
                .map(|target| target.generation)
                .collect::<Vec<_>>(),
            [2, 2]
        );
        release_tx
            .send(())
            .expect("new-session paste fixture retains its active worker receiver");
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
