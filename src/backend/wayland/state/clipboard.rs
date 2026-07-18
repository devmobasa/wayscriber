//! Wayland-state glue for clipboard publish and paste requests.

use super::WaylandState;
use crate::backend::wayland::clipboard::{
    self, ClipboardPasteCompletion, ClipboardPasteResult, ClipboardPoll,
    ClipboardPublishCompletion, FailedLocalSelectionProbe, PasteAction, TransferEffect,
    TransferPlan, TransferWarning, transfer,
};
use crate::input::state::{ClipboardPasteRequest, UiToastKind};
use std::time::{Duration, Instant};

mod session_paste;

use session_paste::{PastePersistenceDecision, SessionPasteWarning};

impl WaylandState {
    pub(in crate::backend::wayland) fn drain_clipboard_requests(&mut self) {
        if !self.clipboard_publish.is_active()
            && let Some(request) = self.input_state.take_pending_selection_clipboard_publish()
        {
            self.start_selection_clipboard_publish(request.generation, request.payload_json);
        }

        if let Some(request) = take_pending_clipboard_paste_if_idle(
            &mut self.input_state,
            self.clipboard_paste.is_active(),
        ) {
            self.start_clipboard_paste(request);
        }
    }

    pub(in crate::backend::wayland) fn poll_clipboard_publish_completion(&mut self) {
        match self.clipboard_publish.poll() {
            ClipboardPoll::Idle | ClipboardPoll::Pending { .. } => {}
            ClipboardPoll::Ready {
                id,
                context: generation,
                outcome,
            } => {
                if outcome.generation == generation {
                    self.apply_selection_clipboard_publish_completion(outcome);
                } else {
                    log::error!(
                        "Clipboard publish operation {id} returned generation {}, expected {generation}",
                        outcome.generation
                    );
                    self.apply_selection_clipboard_publish_completion(
                        failed_clipboard_publish_completion(generation),
                    );
                }
            }
            ClipboardPoll::ProducerFailed {
                id,
                context: generation,
                reason,
            } => {
                log::warn!("Clipboard publish operation {id} failed: {reason}");
                self.apply_selection_clipboard_publish_completion(
                    failed_clipboard_publish_completion(generation),
                );
            }
            ClipboardPoll::Disconnected {
                id,
                context: generation,
            } => {
                log::warn!("Clipboard publish operation {id} disconnected");
                self.apply_selection_clipboard_publish_completion(
                    failed_clipboard_publish_completion(generation),
                );
            }
        }
    }

    pub(in crate::backend::wayland) fn poll_clipboard_paste_completion(&mut self) {
        match self.clipboard_paste.poll() {
            ClipboardPoll::Idle | ClipboardPoll::Pending { .. } => {}
            ClipboardPoll::Ready {
                id,
                context: request,
                outcome,
            } => {
                if outcome.request.id == request.id {
                    self.apply_clipboard_paste_completion(ClipboardPasteCompletion {
                        request,
                        result: outcome.result,
                    });
                } else {
                    log::error!(
                        "Clipboard paste operation {id} returned request {}, expected {}",
                        outcome.request.id,
                        request.id
                    );
                    self.apply_clipboard_paste_completion(failed_clipboard_paste_completion(
                        request,
                        "clipboard producer returned a mismatched request",
                    ));
                }
            }
            ClipboardPoll::ProducerFailed {
                id,
                context: request,
                reason,
            } => {
                log::warn!("Clipboard paste operation {id} failed: {reason}");
                self.apply_clipboard_paste_completion(failed_clipboard_paste_completion(
                    request, &reason,
                ));
            }
            ClipboardPoll::Disconnected {
                id,
                context: request,
            } => {
                log::warn!("Clipboard paste operation {id} disconnected");
                self.apply_clipboard_paste_completion(failed_clipboard_paste_completion(
                    request,
                    "clipboard producer disconnected",
                ));
            }
        }
    }

    fn start_selection_clipboard_publish(&mut self, generation: u64, payload_json: String) {
        self.suppress_focus_exit_for(Duration::from_millis(1500));
        if let Err(failure) =
            self.clipboard_publish
                .try_submit(generation, "clipboard-publish", move || {
                    transfer::resolve_selection_clipboard_publish(generation, payload_json)
                })
        {
            let (error, generation) = failure.into_parts();
            log::warn!("Could not submit clipboard publish operation: {error}");
            self.apply_selection_clipboard_publish_completion(failed_clipboard_publish_completion(
                generation,
            ));
        }
    }

    fn apply_selection_clipboard_publish_completion(
        &mut self,
        completion: ClipboardPublishCompletion,
    ) {
        let applied = self.input_state.complete_selection_clipboard_publish(
            completion.generation,
            completion.fingerprint,
            completion.copied,
        );
        if applied && let Some(warning) = completion.warning {
            self.set_transfer_warning_toast(warning);
        }
    }

    fn start_clipboard_paste(&mut self, request: ClipboardPasteRequest) {
        log::info!(
            "Starting clipboard paste request {} for board '{}' page {} generation {}",
            request.id,
            request.target_board_id,
            request.target_page_index,
            request.target_page_generation
        );
        self.suppress_focus_exit_for(Duration::from_millis(1500));

        let pending_shapes = self.input_state.local_selection_shapes_for_pending_publish(
            request.local_selection_fallback_generation,
        );
        let failed_probe = self
            .input_state
            .failed_local_selection_probe_for_generation(
                request.local_selection_fallback_generation,
            )
            .map(|(generation, expected)| FailedLocalSelectionProbe {
                generation,
                expected,
            });
        let plan = transfer::plan_paste_start(request, pending_shapes, failed_probe);
        self.apply_paste_plan(plan);
    }

    fn apply_clipboard_paste_completion(&mut self, completion: ClipboardPasteCompletion) {
        let active_request_id = self.input_state.active_clipboard_paste_request_id();
        log::info!(
            "Applying clipboard paste completion {} with active_request={:?}: {}",
            completion.request.id,
            active_request_id,
            completion.result.summary()
        );
        let private_payload = match &completion.result {
            ClipboardPasteResult::PrivateSelection(payload)
                if active_request_id == Some(completion.request.id) =>
            {
                let payload_matches_local = self
                    .input_state
                    .private_payload_matches_request_selection(&completion.request, payload);
                let same_instance = self.input_state.private_payload_is_same_instance(payload);
                let shapes = self
                    .input_state
                    .private_payload_shapes_for_request(&completion.request, payload.clone());
                Some(transfer::PrivateSelectionResolution {
                    payload_matches_local,
                    same_instance,
                    shapes,
                })
            }
            _ => None,
        };
        let plan = transfer::plan_paste_completion(completion, active_request_id, private_payload);
        self.apply_paste_plan(plan);
    }

    fn apply_paste_plan(&mut self, plan: TransferPlan<PasteAction>) {
        for effect in plan.effects {
            self.apply_transfer_effect(effect);
        }

        match plan.action {
            PasteAction::UseLocalShapes {
                request,
                shapes,
                warning,
            } => {
                let pasted = self
                    .input_state
                    .paste_clipboard_shapes_from_request(&request, shapes);
                self.input_state.finish_clipboard_paste_request(request.id);
                if pasted == 0 {
                    self.set_transfer_warning_toast(
                        warning.unwrap_or(TransferWarning::NothingPasted),
                    );
                    self.input_state.trigger_blocked_feedback();
                } else if let Some(warning) = warning {
                    self.set_transfer_info_toast(warning);
                }
            }
            PasteAction::ProbeSystemFingerprint {
                request,
                generation,
                expected,
            } => {
                let current = clipboard::clipboard_fingerprint();
                let local_shapes = self
                    .input_state
                    .local_selection_shapes_for_fallback(generation);
                let plan = transfer::plan_after_fingerprint_probe(
                    request,
                    generation,
                    expected,
                    current,
                    local_shapes,
                );
                self.apply_paste_plan(plan);
            }
            PasteAction::ReadSystemClipboard { request } => {
                self.start_system_clipboard_read(request);
            }
            PasteAction::StaleCompletion { request_id } => {
                log::debug!("Ignoring stale clipboard paste completion {}", request_id);
            }
            PasteAction::ApplyPrivateSelection { request, shapes } => {
                let pasted = self
                    .input_state
                    .paste_clipboard_shapes_from_request(&request, shapes);
                self.input_state.finish_clipboard_paste_request(request.id);
                if pasted == 0 {
                    self.set_transfer_warning_toast(TransferWarning::NoShapesPasted);
                    self.input_state.trigger_blocked_feedback();
                }
            }
            PasteAction::ApplyExternalImage { request, image } => {
                let mime_type = image.mime_type.clone();
                let image_width = image.width;
                let image_height = image.height;
                let image_bytes = image.bytes.len();
                let persistence_warning = match self
                    .session_paste_preflight_message(&request, &image)
                {
                    Ok(PastePersistenceDecision::Allow { warning }) => warning,
                    Ok(PastePersistenceDecision::Block { warning }) => {
                        log::warn!(
                            "External image paste request {} rejected by session size preflight: {}",
                            request.id,
                            warning.log_detail
                        );
                        self.show_session_paste_warning(warning);
                        self.input_state.trigger_blocked_feedback();
                        self.input_state.finish_clipboard_paste_request(request.id);
                        return;
                    }
                    Err(err) => {
                        log::warn!(
                            "External image paste request {} could not be checked against session size limits: {}",
                            request.id,
                            err
                        );
                        Some(SessionPasteWarning::toast_only(
                            "Could not check session size; this image may not persist.",
                        ))
                    }
                };
                let pasted = self
                    .input_state
                    .paste_external_image_from_request(&request, image);
                log::info!(
                    "Applied external image paste request {} to board '{}' page {}: success={}, mime={}, dimensions={}x{}, bytes={}",
                    request.id,
                    request.target_board_id,
                    request.target_page_index,
                    pasted,
                    mime_type,
                    image_width,
                    image_height,
                    image_bytes
                );
                if !pasted {
                    self.input_state.trigger_blocked_feedback();
                } else if let Some(warning) = persistence_warning {
                    self.show_session_paste_warning(warning);
                }
                self.input_state.finish_clipboard_paste_request(request.id);
            }
            PasteAction::TryFreshLocalFallbackOrWarn {
                request,
                missing_warning,
                fallback_message,
            } => {
                self.paste_fresh_local_fallback_or_warn(
                    &request,
                    missing_warning,
                    fallback_message,
                );
                self.input_state.finish_clipboard_paste_request(request.id);
            }
            PasteAction::ShowWarning {
                request,
                warning,
                block_feedback,
            } => {
                self.set_transfer_warning_toast(warning);
                if block_feedback {
                    self.input_state.trigger_blocked_feedback();
                }
                self.input_state.finish_clipboard_paste_request(request.id);
            }
        }
    }

    fn start_system_clipboard_read(&mut self, request: ClipboardPasteRequest) {
        log::info!("Reading system clipboard for paste request {}", request.id);
        let context = request.clone();
        if let Err(failure) =
            self.clipboard_paste
                .try_submit(context, "clipboard-paste", move || {
                    let started = Instant::now();
                    let result = transfer::resolve_system_clipboard();
                    log::info!(
                        "System clipboard read for paste request {} completed in {:?}: {}",
                        request.id,
                        started.elapsed(),
                        result.summary()
                    );
                    ClipboardPasteCompletion { request, result }
                })
        {
            let (error, request) = failure.into_parts();
            log::warn!(
                "Could not submit clipboard paste operation for request {}: {error}",
                request.id
            );
            self.apply_clipboard_paste_completion(failed_clipboard_paste_completion(
                request,
                &error.to_string(),
            ));
        }
    }

    fn apply_transfer_effect(&mut self, effect: TransferEffect) {
        match effect {
            TransferEffect::SupersedeLocalGeneration { generation } => self
                .input_state
                .mark_selection_clipboard_superseded_for_generation(Some(generation)),
        }
    }

    fn paste_fresh_local_fallback_or_warn(
        &mut self,
        request: &ClipboardPasteRequest,
        missing_warning: TransferWarning,
        fallback_message: &'static str,
    ) {
        if let Some(generation) = request.local_selection_fallback_generation
            && let Some(shapes) = self
                .input_state
                .local_selection_shapes_for_fallback(generation)
        {
            let pasted = self
                .input_state
                .paste_clipboard_shapes_from_request(request, shapes);
            if pasted > 0 {
                self.input_state
                    .set_ui_toast(UiToastKind::Info, fallback_message);
                return;
            }
        }

        self.set_transfer_warning_toast(missing_warning);
        self.input_state.trigger_blocked_feedback();
    }

    fn set_transfer_info_toast(&mut self, warning: TransferWarning) {
        let message = match warning {
            TransferWarning::ReadTimedOut => Some("Clipboard read timed out; pasted local copy."),
            TransferWarning::ClipboardUnavailable => {
                Some("System clipboard unavailable; pasted local copy.")
            }
            TransferWarning::ClipboardError => Some("Failed to read clipboard; pasted local copy."),
            _ => None,
        };
        if let Some(message) = message {
            self.input_state.set_ui_toast(UiToastKind::Info, message);
        }
    }

    fn set_transfer_warning_toast(&mut self, warning: TransferWarning) {
        match warning {
            TransferWarning::PublishSelectionTooLarge => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Copied locally; selection is too large for system clipboard.",
            ),
            TransferWarning::PublishUnavailable => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Copied locally; system clipboard unavailable.",
            ),
            TransferWarning::NothingPasted => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "Nothing pasted."),
            TransferWarning::NoShapesPasted => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "No shapes pasted."),
            TransferWarning::ClipboardEmpty => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "System clipboard is empty."),
            TransferWarning::UnsupportedContent => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Clipboard content is not a supported image or Wayscriber selection.",
            ),
            TransferWarning::TooLarge { limit } => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                format!(
                    "Clipboard data is too large to paste (limit {} MB).",
                    limit / 1024 / 1024
                ),
            ),
            TransferWarning::TooManyPixels {
                width,
                height,
                limit,
            } => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                format!(
                    "Clipboard image is too large ({}x{}, limit {} pixels).",
                    width, height, limit
                ),
            ),
            TransferWarning::DecodeFailed => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Clipboard image could not be decoded.",
            ),
            TransferWarning::ReadTimedOut => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "Clipboard read timed out."),
            TransferWarning::ClipboardUnavailable => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "System clipboard unavailable."),
            TransferWarning::ClipboardError => self
                .input_state
                .set_ui_toast(UiToastKind::Warning, "Failed to read clipboard."),
            TransferWarning::MalformedPrivateSelection => self.input_state.set_ui_toast(
                UiToastKind::Warning,
                "Wayscriber clipboard selection could not be read.",
            ),
        }
    }
}

fn failed_clipboard_publish_completion(generation: u64) -> ClipboardPublishCompletion {
    ClipboardPublishCompletion {
        generation,
        fingerprint: None,
        copied: false,
        warning: Some(TransferWarning::PublishUnavailable),
    }
}

fn failed_clipboard_paste_completion(
    request: ClipboardPasteRequest,
    reason: &str,
) -> ClipboardPasteCompletion {
    ClipboardPasteCompletion {
        request,
        result: ClipboardPasteResult::ClipboardError(reason.to_string()),
    }
}

fn take_pending_clipboard_paste_if_idle(
    input_state: &mut crate::input::InputState,
    clipboard_paste_active: bool,
) -> Option<ClipboardPasteRequest> {
    (!clipboard_paste_active)
        .then(|| input_state.take_pending_clipboard_paste_request())
        .flatten()
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use crate::input::state::{PasteAnchor, test_support::make_test_input_state};
    use crate::util::Rect;

    fn request(id: u64) -> ClipboardPasteRequest {
        ClipboardPasteRequest {
            id,
            target_board_id: "board".to_string(),
            target_page_index: 2,
            target_page_generation: 3,
            anchor: PasteAnchor::VisibleCenter { x: 10, y: 20 },
            visible_canvas_rect: Rect::new(0, 0, 100, 100).unwrap(),
            screen_size: (100, 100),
            selection_clipboard_generation_at_request: 4,
            local_selection_fallback_generation: Some(5),
        }
    }

    #[test]
    fn publish_transport_failure_preserves_generation_without_sync_probe() {
        let completion = failed_clipboard_publish_completion(17);
        assert_eq!(completion.generation, 17);
        assert_eq!(completion.fingerprint, None);
        assert!(!completion.copied);
        assert_eq!(
            completion.warning,
            Some(TransferWarning::PublishUnavailable)
        );
    }

    #[test]
    fn paste_transport_failure_preserves_original_request_context() {
        let original = request(19);
        let completion = failed_clipboard_paste_completion(original.clone(), "disconnected");
        assert_eq!(completion.request, original);
        assert!(matches!(
            completion.result,
            ClipboardPasteResult::ClipboardError(ref reason) if reason == "disconnected"
        ));
    }

    #[test]
    fn active_paste_transport_defers_the_newest_pending_request() {
        let mut input_state = make_test_input_state();
        let superseded = input_state.request_clipboard_paste();

        assert!(
            take_pending_clipboard_paste_if_idle(&mut input_state, true).is_none(),
            "an active transport must not consume a newer paste request"
        );

        let newest = input_state.request_clipboard_paste();
        assert_ne!(newest.id, superseded.id);
        assert!(take_pending_clipboard_paste_if_idle(&mut input_state, true).is_none());
        assert_eq!(
            take_pending_clipboard_paste_if_idle(&mut input_state, false),
            Some(newest)
        );
    }
}
