use std::collections::VecDeque;

use crate::backend::wayland::{
    RuntimeOperationController, RuntimeOperationIdSource, RuntimeOperationPoll,
    RuntimeOperationSubmitFailure, RuntimeWakeHandle,
    clipboard::{ClipboardPasteCompletion, ClipboardPublishCompletion, transfer},
};
use crate::clipboard_text::{
    ClipboardTextError, copy_text_via_command, read_clipboard_text_via_command,
};
use crate::input::state::{
    ClipboardPasteRequest, TextClipboardRequest, TextPasteEdit, TextPasteTarget,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum HexCopyOutcome {
    Copied,
    Failed,
}

impl HexCopyOutcome {
    pub(in crate::backend::wayland) const fn message(&self) -> &'static str {
        match self {
            Self::Copied => "Copied to clipboard",
            Self::Failed => "Failed to copy to clipboard",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum TextCopyOutcome {
    Copied,
    Failed,
}

impl TextCopyOutcome {
    pub(in crate::backend::wayland) const fn message(&self) -> &'static str {
        match self {
            Self::Copied => "Copied to clipboard",
            Self::Failed => "Failed to copy to clipboard",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum TextPasteOutcome {
    Text(String),
    Empty,
    Failed,
}

pub(in crate::backend::wayland) struct ClipboardPasteSubmitFailure {
    error: String,
    request: Box<ClipboardPasteRequest>,
}

impl ClipboardPasteSubmitFailure {
    pub(in crate::backend::wayland) fn into_parts(self) -> (String, ClipboardPasteRequest) {
        (self.error, *self.request)
    }
}

impl TextPasteOutcome {
    pub(in crate::backend::wayland) const fn message(&self) -> &'static str {
        match self {
            Self::Text(_) => "Pasted from clipboard",
            Self::Empty => "Clipboard empty",
            Self::Failed => "Failed to paste from clipboard",
        }
    }
}

/// Single-flight clipboard workers and their request-coalescing queues.
pub(in crate::backend::wayland) struct ClipboardRuntime {
    publish: RuntimeOperationController<u64, ClipboardPublishCompletion>,
    paste: RuntimeOperationController<ClipboardPasteRequest, ClipboardPasteCompletion>,
    hex_copy: RuntimeOperationController<String, HexCopyOutcome>,
    pending_hex_copy: Option<String>,
    text_copy: RuntimeOperationController<TextClipboardRequest, TextCopyOutcome>,
    pending_text_copy: VecDeque<TextClipboardRequest>,
    text_paste: RuntimeOperationController<TextPasteTarget, TextPasteOutcome>,
    pending_text_paste: VecDeque<TextPasteTarget>,
}

impl ClipboardRuntime {
    pub(super) fn new(ids: RuntimeOperationIdSource, wake: RuntimeWakeHandle) -> Self {
        Self {
            publish: RuntimeOperationController::new(ids.clone(), wake.clone()),
            paste: RuntimeOperationController::new(ids.clone(), wake.clone()),
            hex_copy: RuntimeOperationController::new(ids.clone(), wake.clone()),
            pending_hex_copy: None,
            text_copy: RuntimeOperationController::new(ids.clone(), wake.clone()),
            pending_text_copy: VecDeque::new(),
            text_paste: RuntimeOperationController::new(ids, wake),
            pending_text_paste: VecDeque::new(),
        }
    }

    pub(in crate::backend::wayland) fn publish_active(&self) -> bool {
        self.publish.is_active()
    }

    pub(in crate::backend::wayland) fn submit_publish(
        &mut self,
        generation: u64,
        payload_json: String,
    ) -> Result<(), RuntimeOperationSubmitFailure<u64>> {
        self.publish
            .try_submit(generation, "clipboard-publish", move || {
                transfer::resolve_selection_clipboard_publish(generation, payload_json)
            })
            .map(drop)
    }

    pub(in crate::backend::wayland) fn poll_publish(
        &mut self,
    ) -> RuntimeOperationPoll<u64, ClipboardPublishCompletion> {
        self.publish.poll()
    }

    pub(in crate::backend::wayland) fn paste_active(&self) -> bool {
        self.paste.is_active()
    }

    pub(in crate::backend::wayland) fn submit_paste(
        &mut self,
        request: ClipboardPasteRequest,
        thread_name: &'static str,
        operation: impl FnOnce() -> ClipboardPasteCompletion + Send + 'static,
    ) -> Result<(), ClipboardPasteSubmitFailure> {
        self.paste
            .try_submit(request, thread_name, operation)
            .map(drop)
            .map_err(|failure| {
                let (error, request) = failure.into_parts();
                ClipboardPasteSubmitFailure {
                    error: error.to_string(),
                    request: Box::new(request),
                }
            })
    }

    pub(in crate::backend::wayland) fn poll_paste(
        &mut self,
    ) -> RuntimeOperationPoll<ClipboardPasteRequest, ClipboardPasteCompletion> {
        self.paste.poll()
    }

    pub(in crate::backend::wayland) fn queue_hex_copy(
        &mut self,
        hex: String,
    ) -> Result<(), String> {
        self.enqueue_hex_copy(hex);
        self.submit_pending_hex_copy_if_idle()
    }

    fn enqueue_hex_copy(&mut self, hex: String) {
        self.pending_hex_copy = Some(hex);
    }

    pub(in crate::backend::wayland) fn submit_pending_hex_copy_if_idle(
        &mut self,
    ) -> Result<(), String> {
        if self.hex_copy.is_active() {
            return Ok(());
        }
        let Some(hex) = self.pending_hex_copy.take() else {
            return Ok(());
        };
        let worker_hex = hex.clone();
        self.hex_copy
            .try_submit(
                hex,
                "wayscriber-hex-copy",
                move || match copy_text_via_command(&worker_hex) {
                    Ok(()) => HexCopyOutcome::Copied,
                    Err(error) => {
                        log::warn!("wl-copy failed for hex copy: {error}");
                        HexCopyOutcome::Failed
                    }
                },
            )
            .map(drop)
            .map_err(|failure| failure.into_parts().0.to_string())
    }

    pub(in crate::backend::wayland) fn poll_hex_copy(
        &mut self,
    ) -> RuntimeOperationPoll<String, HexCopyOutcome> {
        self.hex_copy.poll()
    }

    pub(in crate::backend::wayland) fn queue_text_copy(
        &mut self,
        request: TextClipboardRequest,
    ) -> Result<(), String> {
        self.enqueue_text_copy(request);
        self.submit_pending_text_copy_if_idle()
    }

    fn enqueue_text_copy(&mut self, request: TextClipboardRequest) {
        if request.cut.is_none()
            && let Some(previous) = self.pending_text_copy.back_mut()
            && previous.cut.is_none()
        {
            *previous = request;
        } else {
            self.pending_text_copy.push_back(request);
        }
    }

    pub(in crate::backend::wayland) fn submit_pending_text_copy_if_idle(
        &mut self,
    ) -> Result<(), String> {
        if self.text_copy.is_active() {
            return Ok(());
        }
        let Some(request) = self.pending_text_copy.pop_front() else {
            return Ok(());
        };
        let worker_text = request.text.clone();
        self.text_copy
            .try_submit(
                request,
                "wayscriber-text-copy",
                move || match copy_text_via_command(&worker_text) {
                    Ok(()) => TextCopyOutcome::Copied,
                    Err(error) => {
                        log::warn!("wl-copy failed for text copy: {error}");
                        TextCopyOutcome::Failed
                    }
                },
            )
            .map(drop)
            .map_err(|failure| failure.into_parts().0.to_string())
    }

    pub(in crate::backend::wayland) fn poll_text_copy(
        &mut self,
    ) -> RuntimeOperationPoll<TextClipboardRequest, TextCopyOutcome> {
        self.text_copy.poll()
    }

    pub(in crate::backend::wayland) fn queue_text_paste(
        &mut self,
        target: TextPasteTarget,
    ) -> Result<(), String> {
        self.enqueue_text_paste(target);
        self.submit_pending_text_paste_if_idle()
    }

    fn enqueue_text_paste(&mut self, target: TextPasteTarget) {
        if self
            .pending_text_paste
            .back()
            .is_some_and(|pending| pending.generation != target.generation)
        {
            self.pending_text_paste.clear();
        }
        self.pending_text_paste.push_back(target);
    }

    pub(in crate::backend::wayland) fn take_pending_text_paste_if_idle(
        &mut self,
    ) -> Option<TextPasteTarget> {
        (!self.text_paste.is_active())
            .then(|| self.pending_text_paste.pop_front())
            .flatten()
    }

    pub(in crate::backend::wayland) fn submit_text_paste(
        &mut self,
        target: TextPasteTarget,
    ) -> Result<(), String> {
        self.text_paste
            .try_submit(target, "wayscriber-text-paste", read_text_paste)
            .map(drop)
            .map_err(|failure| failure.into_parts().0.to_string())
    }

    pub(in crate::backend::wayland) fn submit_pending_text_paste_if_idle(
        &mut self,
    ) -> Result<(), String> {
        let Some(target) = self.take_pending_text_paste_if_idle() else {
            return Ok(());
        };
        self.submit_text_paste(target)
    }

    pub(in crate::backend::wayland) fn poll_text_paste(
        &mut self,
    ) -> RuntimeOperationPoll<TextPasteTarget, TextPasteOutcome> {
        self.text_paste.poll()
    }

    pub(in crate::backend::wayland) fn rebase_pending_text_pastes(&mut self, edit: &TextPasteEdit) {
        for target in &mut self.pending_text_paste {
            rebase_text_paste_target(target, edit);
        }
    }
}

fn read_text_paste() -> TextPasteOutcome {
    match read_clipboard_text_via_command() {
        Ok(text) => TextPasteOutcome::Text(text),
        Err(ClipboardTextError::Empty) => TextPasteOutcome::Empty,
        Err(ClipboardTextError::Other(error)) => {
            log::warn!("wl-paste failed for text paste: {error}");
            TextPasteOutcome::Failed
        }
    }
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
    use super::*;
    use crate::backend::wayland::{RuntimeOperationIdSource, RuntimeWakeSource};

    fn runtime() -> ClipboardRuntime {
        let wake = RuntimeWakeSource::new().expect("runtime wake source");
        ClipboardRuntime::new(RuntimeOperationIdSource::new(), wake.handle())
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
    fn active_hex_copy_retains_only_the_newest_request() {
        let mut runtime = runtime();
        runtime
            .hex_copy
            .try_submit_with_spawner_for_test(
                "#000000".to_string(),
                || HexCopyOutcome::Copied,
                |_job| Ok(()),
            )
            .expect("test transport starts");

        runtime.queue_hex_copy("#111111".to_string()).unwrap();
        runtime.queue_hex_copy("#222222".to_string()).unwrap();

        assert_eq!(runtime.pending_hex_copy.as_deref(), Some("#222222"));
    }

    #[test]
    fn active_text_copy_replaces_pending_copies_but_preserves_cuts() {
        let mut runtime = runtime();
        runtime
            .text_copy
            .try_submit_with_spawner_for_test(
                copy_request("active"),
                || TextCopyOutcome::Copied,
                |_job| Ok(()),
            )
            .expect("test transport starts");

        runtime.queue_text_copy(copy_request("older")).unwrap();
        runtime.queue_text_copy(cut_request("cut", 0)).unwrap();
        runtime.queue_text_copy(copy_request("replace me")).unwrap();
        runtime.queue_text_copy(copy_request("newest")).unwrap();

        assert_eq!(
            runtime
                .pending_text_copy
                .iter()
                .map(|request| (request.text.as_str(), request.cut.is_some()))
                .collect::<Vec<_>>(),
            [("older", false), ("cut", true), ("newest", false)]
        );
    }

    #[test]
    fn newer_text_session_supersedes_older_pending_pastes_without_coalescing_its_own() {
        let mut runtime = runtime();
        runtime
            .text_paste
            .try_submit_with_spawner_for_test(
                paste_target(1),
                || TextPasteOutcome::Empty,
                |_job| Ok(()),
            )
            .expect("test transport starts");

        runtime.queue_text_paste(paste_target(1)).unwrap();
        runtime.queue_text_paste(paste_target(1)).unwrap();
        runtime.queue_text_paste(paste_target(2)).unwrap();
        runtime.queue_text_paste(paste_target(2)).unwrap();

        assert_eq!(
            runtime
                .pending_text_paste
                .iter()
                .map(|target| target.generation)
                .collect::<Vec<_>>(),
            [2, 2]
        );
    }

    #[test]
    fn active_text_paste_defers_pending_requests() {
        let mut runtime = runtime();
        runtime
            .text_paste
            .try_submit_with_spawner_for_test(
                paste_target(1),
                || TextPasteOutcome::Empty,
                |_job| Ok(()),
            )
            .expect("test transport starts");
        runtime.enqueue_text_paste(paste_target(1));

        assert_eq!(runtime.take_pending_text_paste_if_idle(), None);
        assert_eq!(runtime.pending_text_paste.len(), 1);
    }

    #[test]
    fn queued_paste_target_rebases_after_an_earlier_paste() {
        let mut runtime = runtime();
        runtime.pending_text_paste.push_back(TextPasteTarget {
            generation: 7,
            revision: 3,
            caret: 5,
            selection_anchor: Some(2),
        });

        runtime.rebase_pending_text_pastes(&TextPasteEdit {
            generation: 7,
            previous_revision: 3,
            revision: 4,
            replaced: 2..5,
            inserted_len: 4,
        });

        assert_eq!(
            runtime.pending_text_paste.front(),
            Some(&TextPasteTarget {
                generation: 7,
                revision: 4,
                caret: 6,
                selection_anchor: None,
            })
        );
    }

    #[test]
    fn typed_outcomes_expose_stable_user_messages() {
        assert_eq!(
            HexCopyOutcome::Failed.message(),
            "Failed to copy to clipboard"
        );
        assert_eq!(TextCopyOutcome::Copied.message(), "Copied to clipboard");
        assert_eq!(TextPasteOutcome::Empty.message(), "Clipboard empty");
    }
}
